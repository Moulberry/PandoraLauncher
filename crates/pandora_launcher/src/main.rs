#![deny(unused_must_use)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::fmt::Write;
use std::time::SystemTime;

use bridge::message::MessageToFrontend;
use bridge::modal_action::{ModalAction, ProgressTrackerFinishType};
use clap::Parser;
use fern::colors::ColoredLevelConfig;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use native_dialog::DialogBuilder;
use parking_lot::RwLock;

#[derive(Parser, Debug)]
#[command()]
struct Args {
    /// Instance to launch, instead of opening the launcher
    #[arg(long)]
    run_instance: Option<String>,
}

pub mod panic;

fn main() {
    let args = Args::parse();

    let data_dir = if let Some(portable_dir) = get_portable_dir() {
        portable_dir
    } else {
        let base_dirs = directories::BaseDirs::new().unwrap();
        base_dirs.data_dir().into()
    };

    let launcher_dir = data_dir.join("PandoraLauncher");
    _ = std::env::set_current_dir(&launcher_dir);

    let log_path = launcher_dir.join("launcher.log");
    if log_path.exists() {
        let old_log_path = launcher_dir.join("launcher.log.old");
        _ = std::fs::rename(log_path, old_log_path);
    }

    if let Err(error) = setup_logging(log::LevelFilter::Debug) {
        eprintln!("Unable to enable logging: {error:?}");
    }

    log::debug!("DEBUG logging enabled");
    log::trace!("TRACE logging enabled");

    panic::install_logging_hook();

    if let Some(run_instance) = args.run_instance {
        let (backend_recv, backend_handle, mut frontend_recv, frontend_handle) = bridge::handle::create_pair();

        backend::start(launcher_dir.clone(), frontend_handle, backend_handle.clone(), backend_recv);

        while let Some(message) = frontend_recv.try_recv() {
            if let MessageToFrontend::InstanceAdded { id, name, .. } = message {
                if name.as_str() == run_instance.as_str() {
                    println!("Starting instance {}", run_instance);
                    let modal_action = ModalAction::default();
                    backend_handle.send(bridge::message::MessageToBackend::StartInstance {
                        id,
                        quick_play: None,
                        modal_action: modal_action.clone()
                    });
                    run_modal_action(modal_action);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    return;
                }
            }
        }

        show_error(format!("Unable to find instance {}", run_instance));
        std::process::exit(1);
    } else {
        run_gui(launcher_dir);
    }
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

fn run_modal_action(modal_action: ModalAction) {
    let m = MultiProgress::new();
    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {msg}",
    )
    .unwrap()
    .progress_chars("##-");

    let mut opened = HashSet::new();
    let mut progress_bars = HashMap::new();

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));

        if let Some(error) = &*modal_action.error.read().unwrap() {
            show_error(error.to_string());
            return;
        }

        if modal_action.refcnt() <= 1 {
            modal_action.set_finished();
        }

        if modal_action.get_finished_at().is_some() {
            return;
        }

        if let Some(visit_url) = &*modal_action.visit_url.write().unwrap() {
            if opened.insert(visit_url.url.clone()) {
                _ = m.println(format!("Open this URL in your browser to continue: {}", visit_url.url));
                let open = DialogBuilder::message()
                    .set_title("Open URL")
                    .set_text(&visit_url.message)
                    .confirm()
                    .show()
                    .unwrap_or(true);
                if open {
                    _ = open::that_detached(&*visit_url.url);
                } else {
                    return;
                }
            }
        }

        let trackers = modal_action.trackers.trackers.read().unwrap();
        for tracker in &*trackers {
            let id = tracker.id();

            let pb = progress_bars.entry(id).or_insert_with(|| {
                let pb = m.add(ProgressBar::new(200));
                pb.set_style(sty.clone());
                pb
            });

            if pb.is_finished() && tracker.get_finished_at().is_some() {
                continue;
            }

            let (count, total) = tracker.get();
            pb.set_length(total as u64);
            pb.set_position(count as u64);
            pb.set_message(tracker.get_title().to_string());

            if tracker.get_finished_at().is_some() {
                pb.finish();
            }
        }
        drop(trackers);
    }
}

fn run_gui(launcher_dir: PathBuf) {
    let panic_message = Arc::new(RwLock::new(None));
    let deadlock_message = Arc::new(RwLock::new(None));

    let (backend_recv, backend_handle, frontend_recv, frontend_handle) = bridge::handle::create_pair();

    crate::panic::install_hook(panic_message.clone(), frontend_handle.clone());

    // Start deadlock detection
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
                frontend_handle.send(bridge::message::MessageToFrontend::Refresh);
                return;
            }
        }
    });

    backend::start(launcher_dir.clone(), frontend_handle, backend_handle.clone(), backend_recv);
    frontend::start(launcher_dir.clone(), panic_message, deadlock_message, backend_handle, frontend_recv);
}

fn setup_logging(level: log::LevelFilter) -> Result<(), fern::InitError> {
    let base_config = fern::Dispatch::new()
        .level_for("pandora_launcher", level)
        .level_for("auth", level)
        .level_for("backend", level)
        .level_for("frontend", level)
        .level_for("bridge", level)
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
        .chain(fern::log_file("launcher.log")?);

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
