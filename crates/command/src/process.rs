use crate::PandoraExitStatus;
#[cfg(unix)]
use crate::unix::unix_helpers::cvt_r;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ProcessTerminateState {
    Running,
    Closed,
    Killed,
}

#[derive(Debug)]
pub struct PandoraProcess {
    #[cfg(unix)]
    pub(crate) pid: libc::pid_t,
    #[cfg(windows)]
    pub(crate) process_handle: windows::Win32::Foundation::HANDLE,

    terminate_state: ProcessTerminateState,
    exit_status: Option<PandoraExitStatus>,
}

unsafe impl Send for PandoraProcess {}
unsafe impl Sync for PandoraProcess {}

#[cfg(windows)]
impl Drop for PandoraProcess {
    fn drop(&mut self) {
        use windows::core::Free;
        unsafe {
            std::mem::take(&mut self.process_handle).free();
        }
    }
}

impl PandoraProcess {
    #[cfg(unix)]
    pub(crate) fn new(pid: libc::pid_t) -> Self {
        Self {
            pid,
            terminate_state: ProcessTerminateState::Running,
            exit_status: None,
        }
    }

    #[cfg(windows)]
    pub(crate) fn new(
        process_handle: windows::Win32::Foundation::HANDLE,
    ) -> Self {
        Self {
            process_handle,
            terminate_state: ProcessTerminateState::Running,
            exit_status: None,
        }
    }

    // Try to kill the process nicely (SIGTERM / WM_CLOSE)
    pub fn close(&mut self) -> std::io::Result<()> {
        if self.terminate_state >= ProcessTerminateState::Closed {
            return Ok(());
        }
        self.terminate_state = ProcessTerminateState::Closed;

        #[cfg(unix)]
        unsafe { cvt_r(|| libc::kill(self.pid, libc::SIGTERM))? };

        #[cfg(windows)]
        unsafe {
            use windows::Win32::{Foundation::LPARAM, System::Threading::GetProcessId, UI::WindowsAndMessaging::EnumWindows};

            let process = GetProcessId(self.process_handle);

            if process == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "stop: unable to get process id for handle"));
            }

            let mut data = CloseWindowData {
                match_process: process,
                found: false,
            };

            EnumWindows(Some(close_window_matching), LPARAM(&mut data as *mut _ as isize))?;

            if !data.found {
                // Unable to find a window to send WM_CLOSE, send TerminateProcess instead
                self.terminate_state = ProcessTerminateState::Killed;
                windows::Win32::System::Threading::TerminateProcess(self.process_handle, 1)?;
            }
        }

        Ok(())
    }

    pub fn id(&self) -> u32 {
        #[cfg(unix)]
        return self.pid as u32;
        #[cfg(windows)]
        return unsafe {
            windows::Win32::System::Threading::GetProcessId(self.process_handle)
        };
    }

    // Kill the process forcefully (SIGKILL / TerminateProcess())
    pub fn kill(mut self) -> std::io::Result<()> {
        if self.terminate_state >= ProcessTerminateState::Killed {
            return Ok(());
        }
        self.terminate_state = ProcessTerminateState::Killed;

        #[cfg(unix)]
        unsafe { cvt_r(|| libc::kill(self.pid, libc::SIGKILL))? };

        #[cfg(windows)]
        unsafe { windows::Win32::System::Threading::TerminateProcess(self.process_handle, 1)?; }

        Ok(())
    }

    pub fn wait(self) -> std::io::Result<PandoraExitStatus> {
        // Need to remember the exit status due to waitpid at-most-once semantics
        if let Some(exit_status) = self.exit_status {
            return Ok(exit_status);
        }

        #[cfg(unix)]
        {
            let mut status = 0 as libc::c_int;
            cvt_r(|| unsafe { libc::waitpid(self.pid, &mut status, 0) })?;
            return Ok(PandoraExitStatus(status));
        }

        #[cfg(windows)]
        unsafe {
            let wait = windows::Win32::System::Threading::WaitForSingleObject(self.process_handle, windows::Win32::System::Threading::INFINITE);
            if wait == windows::Win32::Foundation::WAIT_FAILED {
                return Err(windows::core::Error::from_thread().into());
            }

            let mut code = 0;
            windows::Win32::System::Threading::GetExitCodeProcess(self.process_handle, &mut code)?;
            return Ok(PandoraExitStatus(code));
        }
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<PandoraExitStatus>> {
        // Need to remember the exit status due to waitpid at-most-once semantics
        if let Some(exit_status) = self.exit_status {
            return Ok(Some(exit_status));
        }

        #[cfg(unix)]
        {
            let mut status = 0 as libc::c_int;
            cvt_r(|| unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) })?;
            if status == 0 {
                return Ok(None);
            } else {
                self.exit_status = Some(PandoraExitStatus(status));
                return Ok(self.exit_status);
            }
        }

        #[cfg(windows)]
        unsafe {
            let wait = windows::Win32::System::Threading::WaitForSingleObject(self.process_handle, 0);
            if wait == windows::Win32::Foundation::WAIT_FAILED {
                return Err(windows::core::Error::from_thread().into());
            } else if wait == windows::Win32::Foundation::WAIT_TIMEOUT {
                return Ok(None);
            }

            let mut code = 0;
            windows::Win32::System::Threading::GetExitCodeProcess(self.process_handle, &mut code)?;
            self.exit_status = Some(PandoraExitStatus(code));
            return Ok(self.exit_status);
        }
    }
}

#[cfg(windows)]
struct CloseWindowData {
    match_process: u32,
    found: bool,
}

#[cfg(windows)]
extern "system" fn close_window_matching(hwnd: windows::Win32::Foundation::HWND, lparam: windows::Win32::Foundation::LPARAM) -> windows::core::BOOL {
    use windows::Win32::{Foundation::{LPARAM, TRUE, WPARAM}, UI::WindowsAndMessaging::{GetWindowThreadProcessId, PostMessageW, WM_CLOSE}};

    let data = unsafe { (lparam.0 as *mut CloseWindowData).as_mut().unwrap() };

    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)); }

    if process_id != 0 && process_id == data.match_process {
        _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM::default(), LPARAM::default()) };
        data.found = true;
    }

    TRUE
}
