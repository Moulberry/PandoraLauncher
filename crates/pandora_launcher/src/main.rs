#![deny(unused_must_use)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, RwLock};
use std::fmt::Write;

pub mod panic;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

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

                eprintln!("{}", message);
                *deadlock_message.write().unwrap() = Some(message);
                frontend_handle.send(bridge::message::MessageToFrontend::Refresh);
                return;
            }
        }
    });

    backend::start(frontend_handle, backend_handle.clone(), backend_recv);
    frontend::start(panic_message, deadlock_message, backend_handle, frontend_recv);
}
