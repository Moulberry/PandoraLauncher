#![deny(unused_must_use)]

use std::ffi::{OsStr, OsString};

mod backend;
pub use backend::*;

mod backend_filesystem;
mod backend_handler;
mod account;
mod arcfactory;
mod directories;
mod duplicate;
mod export;
mod fs;
mod install_content;
mod instance;
mod java_manifest;
mod launch;
mod launch_wrapper;
mod launcher_import;
mod lockfile;
mod log_reader;
mod metadata;
mod mod_metadata;
mod id_slab;
mod persistent;
mod server_list_pinger;
mod shortcut;
mod skin_manager;
mod syncing;
mod update;

pub const KNOWN_SHADER_MODS: &[&'static str] = &["iris", "oculus", "optifine"];

pub fn join_windows_shell(args: &[&str]) -> String {
    let mut string = String::new();

    let mut first = true;
    for arg in args {
        let mut backslashes = 0;

        if first {
            first = false;
        } else {
            string.push(' ');
        }

        if arg.is_empty() {
            string.push_str("\"\"");
            continue;
        }

        let quoted = arg.contains(&[' ', '\t']);
        if quoted {
            string.push('"');
        }

        for char in arg.chars() {
            if char == '\\' {
                backslashes += 1;
            } else if char == '"' {
                for _ in 0..backslashes {
                    string.push_str("\\\\");
                }
                string.push_str("\\\"");
                backslashes = 0;
            } else {
                for _ in 0..backslashes {
                    string.push('\\');
                }
                backslashes = 0;
                string.push(char);
            }
        }

        if quoted {
            for _ in 0..backslashes {
                string.push_str("\\\\");
            }
        } else {
            for _ in 0..backslashes {
                string.push('\\');
            }
        }

        if quoted {
            string.push('"');
        }
    }

    string
}

pub fn join_windows_shell_os(args: &[&OsStr]) -> OsString {
    let mut string = Vec::new();

    let mut first = true;
    for arg in args {
        let mut backslashes = 0;

        if first {
            first = false;
        } else {
            string.push(b' ');
        }

        if arg.is_empty() {
            string.extend(b"\"\"");
            continue;
        }

        let arg_raw = arg.as_encoded_bytes();
        let quoted = arg_raw.contains(&b' ') || arg_raw.contains(&b'\t');
        if quoted {
            string.push(b'"');
        }

        for byte in arg_raw {
            if *byte == b'\\' {
                backslashes += 1;
            } else if *byte == b'"' {
                for _ in 0..backslashes*2 {
                    string.push(b'\\');
                }
                string.push(b'\\');
                string.push(b'"');
                backslashes = 0;
            } else {
                for _ in 0..backslashes {
                    string.push(b'\\');
                }
                backslashes = 0;
                string.push(*byte);
            }
        }

        if quoted {
            for _ in 0..backslashes*2 {
                string.push(b'\\');
            }
        } else {
            for _ in 0..backslashes {
                string.push(b'\\');
            }
        }

        if quoted {
            string.push(b'"');
        }
    }

    unsafe {
        OsString::from_encoded_bytes_unchecked(string)
    }
}
