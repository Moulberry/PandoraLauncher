#![deny(unused_must_use)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::fmt::Write;
use std::time::SystemTime;

use bridge::handle::{BackendHandle, FrontendHandle};
use bridge::message::{MessageToBackend, MessageToFrontend};
use bridge::quit::QuitCoordinator;
use clap::Parser;
use fern::colors::ColoredLevelConfig;
use native_dialog::DialogBuilder;
use parking_lot::RwLock;

#[derive(Parser, Debug)]
#[command()]
struct Cli {
    /// Instance to launch, instead of opening the launcher
    #[arg(long)]
    run_instance: Option<String>,
    /// Internal function to set traversable ACLs in an elevated context
    #[cfg(windows)]
    #[arg(long, hide = false, num_args = 2..)]
    internal_set_traverse_acls: Option<Vec<std::ffi::OsString>>,
}

pub mod panic;

fn main() {
    let cli = Cli::parse();

    #[cfg(windows)]
    if let Some(internal_set_traverse_acls) = cli.internal_set_traverse_acls {
        if let Err(err) = command::set_traverse_acls(internal_set_traverse_acls) {
            eprintln!("Unable to set traverse ACLs: {err}");
            std::process::exit(1);
        } else {
            std::process::exit(0);
        }
    }

    let data_dir = if let Some(portable_dir) = get_portable_dir() {
        portable_dir
    } else {
        let base_dirs = directories::BaseDirs::new().unwrap();
        base_dirs.data_dir().into()
    };

    let launcher_dir = data_dir.join("PandoraLauncher");
    _ = std::fs::create_dir_all(&launcher_dir);
    _ = std::env::set_current_dir(&launcher_dir);

    let socket = launcher_dir.join("launcher.sock");

    let lockfile_path = launcher_dir.join("launcher.lock");
    let lockfile = match OpenOptions::new().read(true).write(true).create(true).open(&lockfile_path) {
        Ok(lockfile) => lockfile,
        Err(err) => {
            show_error_eprintln(format!("Unable open launcher.lock file: {err}"));
            return;
        },
    };

    if lockfile.try_lock().is_ok() {
        setup_launcher_logging(&launcher_dir);

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("Failed to initialize Tokio runtime");

        _ = std::fs::remove_file(&socket);

        log::info!("Starting local socket: {socket:?}");
        let enter_guard = runtime.enter();

        #[cfg(unix)]
        let bind = PlatformListener::bind(&socket);
        #[cfg(windows)]
        let bind = PlatformListener::bind(std::ffi::OsStr::new("pandora-launcher-socket"));

        let mut listener = match bind {
            Ok(listener) => listener,
            Err(err) => {
                show_error(format!("Unable to start listener: {err}"));
                return;
            },
        };
        drop(enter_guard);

        let panic_message = Arc::new(RwLock::new(None));
        let deadlock_message = Arc::new(RwLock::new(None));

        let (backend_recv, backend_handle, frontend_recv, frontend_handle) = bridge::handle::create_pair();

        crate::panic::install_hook(panic_message.clone(), frontend_handle.clone());
        start_deadlock_detection(&deadlock_message, &frontend_handle);

        let listen_cancel = tokio_util::sync::CancellationToken::new();

        // note: there are many possible race conditions with the whole single-process architecture
        // it's possible for a command to be sent to the main process while it is shutting down
        // it's possible for the socket to be dropped while the file lock is still present
        // it's possible for the file lock to be locked and the socket hasn't started yet
        // most of these can be fixed by implementing some sort of retry logic on the calling process
        // we might also need a semaphore between the listening logic and the shutdown logic, and to
        // potentially cancel the shutdown if we receive a command that results in the shutdown no longer
        // being necessary

        runtime.spawn({
            let frontend_handle = frontend_handle.clone();
            let backend_handle = backend_handle.clone();
            let listen_cancel = listen_cancel.clone();
            let mut args = Vec::new();

            async move {
                'listen: loop {
                    tokio::select! {
                        conn = listener.accept() => {
                            let conn = match conn {
                                Ok(conn) => conn,
                                Err(err) => {
                                    log::error!("An error occurred trying to handle an incoming connection: {err}");
                                    continue;
                                },
                            };
                            let mut conn = tokio::io::BufReader::new(conn);

                            use tokio::io::AsyncReadExt;
                            use tokio::io::AsyncBufReadExt;

                            let mut argc = [0; 1];
                            if let Err(err) = conn.read_exact(&mut argc).await {
                                log::error!("Error reading data from listener: {err}");
                                continue;
                            }

                            args.clear();
                            for _ in 0..argc[0] {
                                let mut buf = Vec::new();
                                if let Err(err) = conn.read_until(b'\0', &mut buf).await {
                                    log::error!("Error reading data from listener: {err}");
                                    continue 'listen;
                                }

                                if buf.last().copied() != Some(0) {
                                    log::error!("Error reading data from listener: expected last byte to be NUL byte");
                                    continue 'listen;
                                }

                                buf.truncate(buf.len() - 1);
                                args.push(unsafe { OsString::from_encoded_bytes_unchecked(buf) });
                            }

                            match Cli::try_parse_from(&args) {
                                Ok(cli) => run_cli(cli, &frontend_handle, &backend_handle),
                                Err(err) => {
                                    log::error!("Error while parsing received arguments: {err}");
                                    continue 'listen;
                                },
                            }
                        },
                        _ = listen_cancel.cancelled() => {
                            break;
                        }
                    }
                }

                drop(listener);
                _ = std::fs::remove_file(&socket);
                drop(lockfile);
            }
        });

        let quit_handler = {
            let backend_handle = backend_handle.clone();
            QuitCoordinator::new(Box::new(move || {
                listen_cancel.cancel();
                backend_handle.send(MessageToBackend::Quit);
                // backend will send Quit to frontend when done
                // when frontend is done, frontend::start will be unblocked and program will exit
            }))
        };

        run_cli(cli, &frontend_handle, &backend_handle);

        backend::start(runtime, launcher_dir.clone(), frontend_handle, backend_handle.clone(), backend_recv, quit_handler.fork());
        frontend::start(launcher_dir.clone(), panic_message, deadlock_message, backend_handle, frontend_recv, quit_handler);
        log::info!("Quiting...");
    } else {
        eprintln!("Connecting to existing local socket: {socket:?}");

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("Failed to initialize Tokio runtime");

        runtime.block_on(async {
            #[cfg(unix)]
            let connect = PlatformClientStream::connect(&socket).await;
            #[cfg(windows)]
            let connect = PlatformClientStream::connect(std::ffi::OsStr::new("pandora-launcher-socket")).await;

            let mut conn = match connect {
                Ok(conn) => conn,
                Err(err) => {
                    show_error_eprintln(format!("Error connecting to local socket: {err}"));
                    return;
                },
            };

            let argc = std::env::args_os().len();
            if argc >= u8::MAX as usize {
                show_error_eprintln(format!("Too many arguments"));
                return;
            }

            let mut bytes = Vec::new();
            bytes.push(argc as u8);
            for arg in std::env::args_os() {
                bytes.extend(arg.as_encoded_bytes());
                bytes.push(0);
            }

            use tokio::io::AsyncWriteExt;
            if let Err(err) = conn.write_all(&bytes).await {
                show_error_eprintln(format!("Error sending request to local socket: {err}"));
                return;
            }
        });
    }
}

struct PlatformListener {
    #[cfg(unix)]
    listener: tokio::net::UnixListener,
    #[cfg(windows)]
    pipe_name: std::ffi::OsString,
    #[cfg(windows)]
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
}

struct PlatformServerStream {
    #[cfg(unix)]
    stream: tokio::net::UnixStream,
    #[cfg(windows)]
    server: tokio::net::windows::named_pipe::NamedPipeServer,
}

struct PlatformClientStream {
    #[cfg(unix)]
    stream: tokio::net::UnixStream,
    #[cfg(windows)]
    client: tokio::net::windows::named_pipe::NamedPipeClient,
}

#[cfg(unix)]
impl PlatformListener {
    fn bind(local_path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            listener: tokio::net::UnixListener::bind(local_path)?
        })
    }

    async fn accept(&mut self) -> std::io::Result<PlatformServerStream> {
        let (stream, _) = self.listener.accept().await?;
        Ok(PlatformServerStream { stream })
    }
}

#[cfg(windows)]
impl PlatformListener {
    fn bind(global_name: &std::ffi::OsStr) -> std::io::Result<Self> {
        let mut pipe_name = std::ffi::OsString::new();
        pipe_name.push(r"\\.\pipe\");
        pipe_name.push(global_name);

        let pipe = tokio::net::windows::named_pipe::ServerOptions::new()
            .access_outbound(false)
            .first_pipe_instance(true)
            .create(&pipe_name)?;

        Ok(Self { pipe_name, pipe, })
    }

    async fn accept(&mut self) -> std::io::Result<PlatformServerStream> {
        self.pipe.connect().await?;
        let old_pipe = std::mem::replace(&mut self.pipe, tokio::net::windows::named_pipe::ServerOptions::new()
            .access_outbound(false)
            .create(&self.pipe_name)?);
        Ok(PlatformServerStream {
            server: old_pipe
        })
    }
}

impl PlatformServerStream {
    #[cfg(unix)]
    fn project(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut tokio::net::UnixStream> {
        unsafe { self.map_unchecked_mut(|s| { &mut s.stream }) }
    }

    #[cfg(windows)]
    fn project(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut tokio::net::windows::named_pipe::NamedPipeServer> {
        unsafe { self.map_unchecked_mut(|s| { &mut s.server }) }
    }
}

impl tokio::io::AsyncRead for PlatformServerStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        tokio::io::AsyncRead::poll_read(self.project(), cx, buf)
    }
}

#[cfg(unix)]
impl PlatformClientStream {
    async fn connect(local_path: &Path) -> std::io::Result<Self> {
        Ok(Self { stream: tokio::net::UnixStream::connect(local_path).await? })
    }

    fn project(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut tokio::net::UnixStream> {
        unsafe { self.map_unchecked_mut(|s| { &mut s.stream }) }
    }
}

#[cfg(windows)]
impl PlatformClientStream {
    async fn connect(global_name: &std::ffi::OsStr) -> std::io::Result<Self> {
        let mut pipe_name = std::ffi::OsString::new();
        pipe_name.push(r"\\.\pipe\");
        pipe_name.push(global_name);

        loop {
            match tokio::net::windows::named_pipe::ClientOptions::new().read(false).open(&pipe_name) {
                Ok(client) => return Ok(Self { client }),
                Err(e) if e.raw_os_error() == Some(231) => (), // ERROR_PIPE_BUSY
                Err(e) => return Err(e),
            }

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    fn project(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut tokio::net::windows::named_pipe::NamedPipeClient> {
        unsafe { self.map_unchecked_mut(|s| { &mut s.client }) }
    }
}

impl tokio::io::AsyncWrite for PlatformClientStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        tokio::io::AsyncWrite::poll_write(self.project(), cx, buf)
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        tokio::io::AsyncWrite::poll_flush(self.project(), cx)
    }

    fn poll_shutdown(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        tokio::io::AsyncWrite::poll_shutdown(self.project(), cx)
    }
}

fn run_cli(cli: Cli, frontend: &FrontendHandle, backend: &BackendHandle) {
    frontend.send(MessageToFrontend::OpenOrFocusMainWindow);

    if let Some(run_instance) = cli.run_instance {
        backend.send(bridge::message::MessageToBackend::StartInstanceByName {
            name: run_instance,
            quick_play: None,
        });
    }
}

fn setup_launcher_logging(launcher_dir: &Path) {
    let log_file = launcher_dir.join("launcher.log");
    if log_file.exists() {
        let old_log_file = launcher_dir.join("launcher.log.old");
        _ = std::fs::rename(&log_file, old_log_file);
    }

    if let Err(error) = init_logging(log::LevelFilter::Debug, &log_file) {
        eprintln!("Unable to enable logging: {error:?}");
    }

    log::debug!("DEBUG logging enabled");
    log::trace!("TRACE logging enabled");

    panic::install_logging_hook();
}

fn show_error(error: String) {
    log::error!("{}", error);
    _ = DialogBuilder::message()
        .set_level(native_dialog::MessageLevel::Error)
        .set_title("An error occurred")
        .set_text(error)
        .alert()
        .show();
}

fn show_error_eprintln(error: String) {
    eprintln!("{}", error);
    _ = DialogBuilder::message()
        .set_level(native_dialog::MessageLevel::Error)
        .set_title("An error occurred")
        .set_text(error)
        .alert()
        .show();
}

fn start_deadlock_detection(deadlock_message: &Arc<parking_lot::lock_api::RwLock<parking_lot::RawRwLock, Option<String>>>, frontend_handle: &bridge::handle::FrontendHandle) {
    std::thread::spawn({
        let deadlock_message = deadlock_message.clone();
        let frontend_handle = frontend_handle.clone();
        move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                let deadlocks = parking_lot::deadlock::check_deadlock();
                if deadlocks.is_empty() {
                    continue;
                }

                let mut message = String::new();
                _ = writeln!(&mut message, "{} deadlock(s) detected", deadlocks.len());
                for (i, threads) in deadlocks.iter().enumerate() {
                    _ = writeln!(&mut message, "==== Deadlock #{} ({} threads) ====", i, threads.len());
                    for (thread_index, t) in threads.iter().enumerate() {
                        _ = writeln!(&mut message, "== Thread #{} ({:?}) ==", thread_index, t.thread_id());
                        _ = writeln!(&mut message, "{:#?}", t.backtrace());
                    }
                }

                log::error!("{}", message);
                *deadlock_message.write() = Some(message);
                frontend_handle.send(MessageToFrontend::Refresh);
                return;
            }
        }
    });
}

fn init_logging(level: log::LevelFilter, log_file: &Path) -> Result<(), fern::InitError> {
    let base_config = fern::Dispatch::new()
        .level_for("pandora_launcher", level)
        .level_for("auth", level)
        .level_for("backend", level)
        .level_for("frontend", level)
        .level_for("bridge", level)
        .level_for("command", level)
        .level_for("gpui_component::text", log::LevelFilter::Off)
        .level(log::LevelFilter::Warn);

    let colors_line = ColoredLevelConfig::new().info(fern::colors::Color::BrightWhite);

    let file_config = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{time} {level} {target}] {message}",
                time = humantime::format_rfc3339_seconds(SystemTime::now()),
                level = record.level(),
                target = record.target(),
                message = message
            ))
        })
        .chain(fern::log_file(log_file)?);

    let stdout_config = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{time} {level} {target}{color_line}] {message}\x1B[0m",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                time = humantime::format_rfc3339_seconds(SystemTime::now()),
                level = record.level(),
                target = record.target(),
                message = message
            ))
        })
        .chain(std::io::stdout());

    base_config
        .chain(file_config)
        .chain(stdout_config)
        .apply()?;

    Ok(())
}

fn get_portable_dir() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let file_name = current_exe.file_name()?;
    let file_name = file_name.to_string_lossy();
    if file_name.to_lowercase().contains("portable") {
        Some(current_exe.parent()?.into())
    } else {
        None
    }
}
