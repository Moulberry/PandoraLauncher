use std::{io::{Error, ErrorKind}, sync::mpsc};

use once_cell::sync::OnceCell;

use crate::{PandoraChild, PandoraCommand, PandoraSandbox};

pub enum SpawnType {
    Normal,
    Elevated,
    Sandboxed(PandoraSandbox),
}

struct SpawnInfo {
    command: PandoraCommand,
    spawn_type: SpawnType,
    sender: tokio::sync::oneshot::Sender<std::io::Result<PandoraChild>>,
}

// We use a special thread for spawning commands, this is for a few reasons:
// 1. It prevents unix children from being terminated early due to PR_SET_PDEATHSIG if the parent thread is killed
// 2. It prevents race conditions such as inheriting handles on windows
// 3. Some functions e.g. ShellExecuteW will block the current thread until the user interacts with the UAC dialog

#[derive(Default)]
pub struct SpawnContext {
    #[cfg(windows)]
    pub job_handle: Option<std::os::windows::io::OwnedHandle>,
    #[cfg(windows)]
    pub null_device: Option<std::os::windows::io::OwnedHandle>,
    #[cfg(unix)]
    pub dev_null_fd: Option<libc::c_int>,
    #[cfg(target_os = "linux")]
    pub dbus_proxy: Option<crate::unix::linux::bwrap::DbusProxy>,
}

#[cfg(windows)]
fn illegal_filename_char(b: u8) -> bool {
    b < 0x1f || matches!(b, b'/' | b'?' | b'<' | b'>' | b'\\' | b':' | b'*' | b'|' | b'"')
}

pub fn spawn(command: PandoraCommand, spawn_type: SpawnType) -> tokio::sync::oneshot::Receiver<std::io::Result<PandoraChild>> {
    let (sender, receiver) = tokio::sync::oneshot::channel();

    static SPAWNING_CHANNEL: OnceCell<mpsc::Sender<SpawnInfo>> = OnceCell::new();
    let channel = SPAWNING_CHANNEL.get_or_init(|| {
        let (send, recv) = mpsc::channel::<SpawnInfo>();

        std::thread::Builder::new()
            .name("Pandora Command Spawner".to_string())
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut context = SpawnContext::default();

                // Initialize COM on this thread. In my testing this wasn't needed, but it shouldn't hurt
                #[cfg(windows)]
                unsafe {
                    _ = windows::Win32::System::Com::CoInitializeEx(
                        None,
                        windows::Win32::System::Com::COINIT_APARTMENTTHREADED |  windows::Win32::System::Com::COINIT_DISABLE_OLE1DDE,
                    );
                }

                for info in recv {
                    _ = info.sender.send(handle_spawn(info.command, info.spawn_type, &mut context))
                }
            })
            .unwrap();

        send
    });
    channel.send(SpawnInfo { command, spawn_type, sender }).unwrap();

    receiver
}

fn handle_spawn(mut command: PandoraCommand, spawn_type: SpawnType, context: &mut SpawnContext) -> std::io::Result<PandoraChild> {
    match spawn_type {
        SpawnType::Normal => {
            #[cfg(unix)]
            return crate::unix::unix_spawn::spawn(command, context);
            #[cfg(windows)]
            return crate::windows::windows_spawn::spawn(command, context);
        },
        SpawnType::Elevated => {
            command.stdin = crate::PandoraStdioWriteMode::Null;
            command.stdout = crate::PandoraStdioReadMode::Null;
            command.stderr = crate::PandoraStdioReadMode::Null;

            if command.inherit_env.is_some() || !command.env.is_empty() {
                return Err(Error::new(ErrorKind::InvalidInput, "cannot set custom environment for elevated process"));
            }

            #[cfg(target_os = "linux")]
            return crate::unix::linux::pkexec::spawn(command, context);
            #[cfg(windows)]
            return crate::windows::runas::spawn(command, context);
            #[cfg(target_os = "macos")]
            return crate::unix::macos::elevated::spawn(command);
        },
        SpawnType::Sandboxed(sandbox) => {
            #[cfg(target_os = "linux")]
            return crate::unix::linux::bwrap::spawn(command, sandbox, context);

            #[cfg(windows)]
            {
                if sandbox.name.as_encoded_bytes().iter().any(|b| illegal_filename_char(*b)) {
                    return Err(Error::new(ErrorKind::InvalidInput, "name contained illegal character"));
                }
                return crate::windows::appcontainer::spawn(command, sandbox, context);
            }

            #[cfg(target_os = "macos")]
            return crate::unix::macos::sandbox::spawn(command, sandbox, context);
        },
    }
}
