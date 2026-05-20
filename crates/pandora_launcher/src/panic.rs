use std::{
    io::Write,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use bridge::handle::FrontendHandle;
use parking_lot::RwLock;

pub fn install_logging_hook() {
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unknown>");

        let payload = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };

        let backtrace = backtrace::Backtrace::new();

        let message = match info.location() {
            Some(location) => {
                format!(
                    "Thread {} panicked at {}:{}:{}\n{}\n{:?}",
                    thread_name,
                    location.file(),
                    location.line(),
                    location.column(),
                    payload,
                    PrettyBacktrace(backtrace)
                )
            },
            None => format!("Thread {} panicked\n{}\n{:?}", thread_name, payload, PrettyBacktrace(backtrace)),
        };
        persist_panic_report(&message);
        log::error!("{}", message);
    }));
}

pub fn install_hook(panic_message: Arc<RwLock<Option<String>>>, frontend_handle: FrontendHandle) {
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        if thread.name() == Some("tokio-runtime-worker") {
            let payload = match info.payload().downcast_ref::<&'static str>() {
                Some(s) => *s,
                None => match info.payload().downcast_ref::<String>() {
                    Some(s) => &**s,
                    None => "Box<Any>",
                },
            };

            let backtrace = backtrace::Backtrace::new();

            let message = match info.location() {
                Some(location) => {
                    format!(
                        "Backend panicked at {}:{}:{}\n{}\n{:?}",
                        location.file(),
                        location.line(),
                        location.column(),
                        payload,
                        PrettyBacktrace(backtrace)
                    )
                },
                None => format!("Backend panicked\n{}\n{:?}", payload, PrettyBacktrace(backtrace)),
            };

            persist_panic_report(&message);
            log::error!("{}", message);
            *panic_message.write() = Some(message);
            frontend_handle.send(bridge::message::MessageToFrontend::Refresh);
        } else {
            (old_hook)(info);
        }
    }));
}

fn persist_panic_report(message: &str) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let pid = std::process::id();
    let report_dir: PathBuf = PathBuf::from("panic-reports");
    let _ = std::fs::create_dir_all(&report_dir);
    let file_name = format!("panic-{}-{}-{}.log", now.as_secs(), now.subsec_millis(), pid);
    let path = report_dir.join(file_name);

    if let Ok(mut file) = std::fs::OpenOptions::new().create_new(true).write(true).open(path) {
        let _ = writeln!(file, "{}", message);
        let _ = file.flush();
    }
}

struct PrettyBacktrace(backtrace::Backtrace);

impl std::fmt::Debug for PrettyBacktrace {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cwd = std::env::current_dir();
        let mut print_path = move |fmt: &mut std::fmt::Formatter<'_>, path: backtrace::BytesOrWideString<'_>| {
            let path = path.into_path_buf();
            if let Ok(cwd) = &cwd
                && let Ok(suffix) = path.strip_prefix(cwd)
            {
                return std::fmt::Display::fmt(&suffix.display(), fmt);
            }
            std::fmt::Display::fmt(&path.display(), fmt)
        };

        let mut f = backtrace::BacktraceFmt::new(fmt, backtrace::PrintFmt::Short, &mut print_path);
        f.add_context()?;
        let frames = self.0.frames();
        let ignore_start = &[
            "backtrace::backtrace::trace",
            "backtrace::capture::Backtrace::create",
            "backtrace::capture::Backtrace::new",
            "pandora_launcher::panic::install_hook::{{closure}}",
            "__rustc::rust_begin_unwind",
        ];
        let mut start = 0;
        for (index, frame) in frames.iter().enumerate() {
            for symbol in frame.symbols() {
                if let Some(name) = symbol.name() {
                    let name_str = format!("{name:#}");
                    if ignore_start.contains(&name_str.as_str()) {
                        start = index;
                    }
                }
            }
        }
        for frame in &frames[start..] {
            f.frame().backtrace_frame(frame)?;
        }
        f.finish()?;
        Ok(())
    }
}
