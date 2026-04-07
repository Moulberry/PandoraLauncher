use std::{ffi::{OsStr, OsString}, io::{Error, ErrorKind}, os::fd::{AsRawFd, FromRawFd, RawFd}, path::{Path, PathBuf}};

use libseccomp::{ScmpAction, ScmpArgCompare, ScmpCompareOp, ScmpFilterContext, ScmpSyscall};
use once_cell::sync::Lazy;
use rustc_hash::FxHashSet;

use crate::{PandoraArg, PandoraChild, PandoraCommand, PandoraSandbox, spawner::SpawnContext, unix::unix_helpers::cvt};

const DEV_BINDS: &[&str] = &[
    // Graphics
    "/dev/dri",
    // Graphics (mali)
    "/dev/mali",
    "/dev/mali0",
    "/dev/umplock",
    // Graphics (nvidia)
    "/dev/nvidiactl",
    "/dev/nvidia0",
    "/dev/nvidia",
    // Graphics (adreno)
    "/dev/kgsl-3d0",
    "/dev/ion",
    // System info
    "/dev/disk/by-uuid",
    "/dev/dm",
    "/dev/loop",
    "/dev/mapper",
    "/dev/ram",
    // Raw ALSA
    "/dev/snd"
];

const SYSTEM_FILES_RO: &[&str] = &[
    "/bin",
    "/sbin",
    "/usr",
    "/lib",
    "/lib32",
    "/lib64",
    "/etc/alternatives",
    "/etc/resolv.conf",
    "/run/systemd/resolve",
    "/usr/share/ca-certificates",
    "/etc/ca-certificates",
    "/etc/ssl",
    "/etc/pki",
    "/etc/pkcs11",
    "/etc/hosts",
    "/etc/ld.so.cache",
    "/etc/ld.so.conf.d",
    "/etc/localtime",
    "/etc/os-release",
    "/etc/machine-id",
    "/etc/timezone",
    "/etc/fonts",
    "/sys/dev/char",
    "/sys/bus/pci/devices",
    "/sys/devices/system/cpu",
    "/sys/devices/virtual/dmi/id",
    "/sys/class/net",
    "/sys/firmware/devicetree/base/model",
    "/sys/class/power_supply",
    "/sys/class/hwmon",
    "/sys/class/thermal",
    "/sys/class/drm",
];

static ALLOWED_ENV_VARS: Lazy<FxHashSet<&'static OsStr>> = Lazy::new(|| {
    [
        "GDMSESSION",
        "DESKTOP_SESSION",
        "PATH",
        "HOME",
        "LANG",
        "LC_ALL",
        "TERM",
        "USER",
        "USERNAME",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "PULSE_SERVER"
    ].iter().map(OsStr::new).collect()
});

#[derive(Debug)]
enum BindType {
    Device,
    ReadOnly,
    ReadWrite,
}

#[derive(Default)]
struct BwrapBuilder {
    command: Vec<PandoraArg>,
    bound_paths: Vec<PathBuf>,
}

impl BwrapBuilder {
    fn set_env_var(&mut self, key: impl Into<PandoraArg>, value: impl Into<PandoraArg>) {
        self.push_str("--setenv");
        self.command.push(key.into());
        self.command.push(value.into());
    }

    fn push_str(&mut self, string: &'static str) {
        self.command.push(string.into())
    }

    fn push_os_string(&mut self, string: OsString) {
        self.command.push(string.into())
    }

    fn bind_if_exists(&mut self, bind_type: BindType, path: &Path) {
        let mut path = path.to_path_buf();
        loop {
            let Ok(resolved) = path.canonicalize() else {
                return;
            };

            if resolved == path {
                break;
            } else {
                for already_bound in &self.bound_paths {
                    if path.starts_with(already_bound) {
                        return;
                    }
                }
                self.bound_paths.push(path.clone());

                if !path.starts_with(&resolved) {
                    self.push_str("--symlink");
                    self.command.push(resolved.clone().into_os_string().into());
                    self.command.push(path.into_os_string().into());
                }

                path = resolved;
            }
        }

        for already_bound in &self.bound_paths {
            if path.starts_with(already_bound) {
                return;
            }
        }
        self.bound_paths.push(path.clone());

        match bind_type {
            BindType::Device => self.push_str("--dev-bind-try"),
            BindType::ReadOnly => self.push_str("--ro-bind-try"),
            BindType::ReadWrite => self.push_str("--bind-try"),
        }

        self.command.push(path.clone().into_os_string().into());
        self.command.push(path.into_os_string().into());
    }

    fn create_dir(&mut self, path: &Path) {
        self.push_str("--dir");
        self.command.push(path.as_os_str().to_os_string().into());
    }
}

pub fn should_pass_env_var(var: &OsStr) -> bool {
    return var.as_encoded_bytes().starts_with(b"XDG_") || ALLOWED_ENV_VARS.contains(var)
}

fn get_card_names() -> Vec<OsString> {
    let mut card_names = Vec::new();
    let Ok(read_dir) = std::fs::read_dir("/dev/dri") else {
        return card_names;
    };

    for entry in read_dir {
        if let Ok(entry) = entry {
            card_names.push(entry.file_name());
        }
    }

    card_names
}

pub fn spawn(mut command: PandoraCommand, sandbox: PandoraSandbox, context: &mut SpawnContext) -> std::io::Result<PandoraChild> {
    let resolved_executable = if command.executable.0.as_encoded_bytes().contains(&b'/') {
        let path = Path::new(&command.executable.0);
        let Ok(path) = path.canonicalize() else {
            return Err(Error::new(ErrorKind::NotFound, "executable file doesn't exist"));
        };
        path
    } else if let Some(path) = crate::path_cache::get_command_path(&command.executable.0) {
        path.to_path_buf()
    } else {
        return Err(Error::new(ErrorKind::NotFound, "unable to resolve executable"));
    };

    dbg!(&resolved_executable);

    let Some(bwrap) = crate::path_cache::get_command_path(OsStr::new("bwrap")) else {
        return Err(Error::new(ErrorKind::NotFound, "unable to find 'bwrap'"));
    };

    command.executable = bwrap.as_os_str().to_os_string().into();
    command.inherit_env = Some(should_pass_env_var);
    command.args.insert(0, resolved_executable.clone().into_os_string().into());

    let Some(directories) = directories::BaseDirs::new() else {
        return Err(Error::new(ErrorKind::NotFound, "unable to determine base directories"));
    };

    let mut builder = BwrapBuilder::default();

    builder.push_str("--die-with-parent");
    builder.push_str("--unshare-all");
    if sandbox.grant_network_access {
        builder.push_str("--share-net");
    }

    builder.push_str("--proc");
    builder.push_str("/proc");

    builder.push_str("--dev");
    builder.push_str("/dev");

    builder.push_str("--tmpfs");
    builder.push_str("/tmp");

    for dev_bind in DEV_BINDS {
        builder.bind_if_exists(BindType::Device, Path::new(*dev_bind));
    }

    for file_ro in SYSTEM_FILES_RO {
        builder.bind_if_exists(BindType::ReadOnly, Path::new(*file_ro));
    }

    // Bind graphics card devices
    let card_names = get_card_names();
    if let Ok(devices) = std::fs::read_dir("/sys/dev/char") {
        for entry in devices {
            let Ok(entry) = entry else {
                continue;
            };
            let Ok(path) = entry.path().canonicalize() else {
                continue;
            };
            let Some(filename) = path.file_name() else {
                continue;
            };
            if card_names.contains(&filename.to_os_string()) {
                if let Ok(device) = path.join("device").canonicalize() {
                    builder.bind_if_exists(BindType::ReadOnly, &device);
                    if let Ok(driver) = device.join("driver").canonicalize() {
                        builder.bind_if_exists(BindType::ReadOnly, &driver);
                    }
                }
            }
        }
    }

    // Bind everything in $PATH
    if let Some(path) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&path) {
            builder.bind_if_exists(BindType::ReadOnly, &path);
        }
    }

    // Bind files in xdg runtime dir
    if let Some(xdg_runtime_dir) = directories.runtime_dir() {
        builder.create_dir(xdg_runtime_dir);

        let display_path = xdg_runtime_dir.join(
            std::env::var_os("WAYLAND_DISPLAY").as_deref().unwrap_or(OsStr::new("wayland-0"))
        );
        builder.bind_if_exists(BindType::ReadOnly, &display_path);

        let pipewire_path = xdg_runtime_dir.join("pipewire-0");
        builder.bind_if_exists(BindType::ReadOnly, &pipewire_path);

        if let Some(pulse_server) = std::env::var_os("PULSE_SERVER") {
            if let Some(pulse_server_path) = pulse_server.as_encoded_bytes().strip_prefix(b"unix:") {
                let pulse_server_path = unsafe { OsStr::from_encoded_bytes_unchecked(pulse_server_path) };
                builder.bind_if_exists(BindType::ReadOnly, Path::new(pulse_server_path));
            }
        } else {
            let pulse_path = xdg_runtime_dir.join("pulse");
            builder.bind_if_exists(BindType::ReadOnly, &pulse_path);
        }
    }

    // Bind a bunch of pulse audio stuff
    let pulse_config_dir = directories.config_dir().join("pulse");
    builder.bind_if_exists(BindType::ReadOnly, &pulse_config_dir);
    let pulse_home_config_dir = directories.home_dir().join(".pulse");
    builder.bind_if_exists(BindType::ReadOnly, &pulse_home_config_dir);
    let asound_home_config_dir = directories.home_dir().join(".asoundrc");
    builder.bind_if_exists(BindType::ReadOnly, &asound_home_config_dir);

    builder.bind_if_exists(BindType::ReadWrite, Path::new("/run/pulse"));

    if let Some(pulse_clientconfig) = std::env::var_os("PULSE_CLIENTCONFIG") {
        builder.bind_if_exists(BindType::ReadOnly, Path::new(&pulse_clientconfig));
    }

    // Bind X11 sockets/xauthority
    let display_index = std::env::var_os("DISPLAY").and_then(|display| {
        let display_bytes = display.as_encoded_bytes();
        if display_bytes.len() == 2 && display_bytes[0] == b':' && display_bytes[1] >= b'0' && display_bytes[1] <= b'9' {
            Some(display_bytes[1] - b'0')
        } else {
            None
        }
    }).unwrap_or(0);

    builder.bind_if_exists(BindType::ReadOnly, Path::new(&format!("/tmp/.X11-unix/X{display_index}")));
    if let Some(xauthority) = std::env::var_os("XAUTHORITY") {
        builder.bind_if_exists(BindType::ReadOnly, Path::new(&xauthority));
    } else {
        builder.bind_if_exists(BindType::ReadOnly, &directories.home_dir().join(".Xauthority"));
    }

    // Bind java
    if sandbox.is_jvm && let Some(java_parent) = resolved_executable.parent() && java_parent.file_name() == Some(OsStr::new("bin")) {
        if let Some(java_parent_parent) = java_parent.parent() {
            let lib = java_parent_parent.join("lib");
            if lib.is_dir() {
                builder.bind_if_exists(BindType::ReadOnly, &lib);
            }

            let conf = java_parent_parent.join("conf");
            if conf.is_dir() {
                builder.bind_if_exists(BindType::ReadOnly, &conf);
            }
        }

        builder.bind_if_exists(BindType::ReadOnly, &java_parent);
    } else {
        builder.bind_if_exists(BindType::ReadOnly, &resolved_executable);
    }

    for path in sandbox.allow_read {
        builder.bind_if_exists(BindType::ReadOnly, &path);
    }
    for path in sandbox.allow_write {
        builder.bind_if_exists(BindType::ReadWrite, &path);
    }

    // Create sandboxed xdg home directories
    let sandbox_cache = sandbox.sandbox_dir.join("cache");
    let sandbox_config = sandbox.sandbox_dir.join("config");
    let sandbox_data = sandbox.sandbox_dir.join("data");
    _ = std::fs::create_dir_all(&sandbox_cache);
    _ = std::fs::create_dir_all(&sandbox_config);
    _ = std::fs::create_dir_all(&sandbox_data);
    builder.bind_if_exists(BindType::ReadWrite, &sandbox_cache);
    builder.bind_if_exists(BindType::ReadWrite, &sandbox_config);
    builder.bind_if_exists(BindType::ReadWrite, &sandbox_data);
    builder.set_env_var(OsStr::new("XDG_CACHE_HOME"), sandbox_cache.into_os_string());
    builder.set_env_var(OsStr::new("XDG_CONFIG_HOME"), sandbox_config.into_os_string());
    builder.set_env_var(OsStr::new("XDG_DATA_HOME"), sandbox_data.into_os_string());

    // Setup seccomp filtering
    let seccomp_fd = create_seccomp_filter()?;
    builder.push_str("--seccomp");
    builder.push_os_string(format!("{}", seccomp_fd.as_raw_fd()).into());
    command.pass_fds.push(seccomp_fd);

    let dbus_proxy = if let Some(dbus_proxy) = &context.dbus_proxy {
        dbus_proxy
    } else {
        let dbus_proxy = start_dbus_proxy(&sandbox.sandbox_dir, context)?;
        context.dbus_proxy = Some(dbus_proxy);
        context.dbus_proxy.as_ref().unwrap()
    };

    if let Some(session_bus_proxy) = &dbus_proxy.session_bus_proxy {
        builder.push_str("--bind");
        builder.push_os_string(session_bus_proxy.clone().into_os_string());
        let runtime_dir = directories.runtime_dir().unwrap_or(Path::new("/run/user/1000"));
        let mapped_bus_dir = runtime_dir.join("bus").into_os_string();
        builder.push_os_string(mapped_bus_dir.clone());

        let mut dbus_session_bus_address = OsString::new();
        dbus_session_bus_address.push("unix:path=");
        dbus_session_bus_address.push(mapped_bus_dir);

        builder.set_env_var(OsStr::new("DBUS_SESSION_BUS_ADDRESS"), dbus_session_bus_address);
        if directories.runtime_dir().is_none() {
            builder.set_env_var(OsStr::new("XDG_RUNTIME_DIR"), OsStr::new("/run/user/1000"));
        }
        builder.set_env_var(OsStr::new("GTK_USE_PORTAL"), OsStr::new("1"));
    }

    command.args.splice(0..0, builder.command);
    crate::unix::unix_spawn::spawn(command, context)
}

pub struct DbusProxy {
    session_bus_proxy: Option<PathBuf>,
}

fn start_dbus_proxy(sandbox_dir: &Path, context: &mut SpawnContext) -> std::io::Result<DbusProxy> {
    let Some(session_bus_address) = std::env::var_os("DBUS_SESSION_BUS_ADDRESS") else {
        return Ok(DbusProxy {
            session_bus_proxy: None,
        });
    };
    let Some(proxy_executable) = crate::path_cache::get_command_path(OsStr::new("xdg-dbus-proxy")) else {
        return Err(Error::new(ErrorKind::NotFound, "unable to find 'xdg-dbus-proxy'"))
    };

    let proxy_path = sandbox_dir.join("dbus-proxy");
    _ = std::fs::create_dir_all(&proxy_path);

    let session_bus_proxy = proxy_path.join("session.sock");

    let mut command = PandoraCommand::new(proxy_executable.as_os_str().to_os_string());
    command.arg(session_bus_address);
    command.arg(session_bus_proxy.clone().into_os_string());
    command.arg("--filter");
    command.arg("--talk=com.feralinteractive.GameMode");
    command.arg("--call=com.feralinteractive.GameMode=/com/feralinteractive/GameMode");
    command.arg("--talk=org.kde.StatusNotifierWatcher");
    command.arg("--call=org.kde.StatusNotifierWatcher=/StatusNotifierWatcher");
    command.arg("--talk=org.freedesktop.Notifications");
    command.arg("--call=org.freedesktop.Notifications=/org/freedesktop/Notifications");
    command.arg("--talk=org.freedesktop.portal.*");
    command.arg("--talk=org.mpris.MediaPlayer2.*");

    _ = crate::unix::unix_spawn::spawn(command, context)?;

    Ok(DbusProxy {
        session_bus_proxy: Some(session_bus_proxy),
    })
}

// Syscall allowlist copied from https://github.com/moby/profiles/blob/fa50b7287199d1c781284d1a34d1395a62e57f1e/seccomp/default.json
// Licensed as Apache 2.0 (https://github.com/moby/profiles/blob/fa50b7287199d1c781284d1a34d1395a62e57f1e/LICENSE)
const ALLOWED_SYSCALLS: &[&'static str] = &[
    "accept",
    "accept4",
    "access",
    "adjtimex",
    "alarm",
    "bind",
    "brk",
    "cachestat",
    "capget",
    "capset",
    "chdir",
    "chmod",
    "chown",
    "chown32",
    "clock_adjtime",
    "clock_adjtime64",
    "clock_getres",
    "clock_getres_time64",
    "clock_gettime",
    "clock_gettime64",
    "clock_nanosleep",
    "clock_nanosleep_time64",
    "close",
    "close_range",
    "connect",
    "copy_file_range",
    "creat",
    "dup",
    "dup2",
    "dup3",
    "epoll_create",
    "epoll_create1",
    "epoll_ctl",
    "epoll_ctl_old",
    "epoll_pwait",
    "epoll_pwait2",
    "epoll_wait",
    "epoll_wait_old",
    "eventfd",
    "eventfd2",
    "execve",
    "execveat",
    "exit",
    "exit_group",
    "faccessat",
    "faccessat2",
    "fadvise64",
    "fadvise64_64",
    "fallocate",
    "fanotify_mark",
    "fchdir",
    "fchmod",
    "fchmodat",
    "fchmodat2",
    "fchown",
    "fchown32",
    "fchownat",
    "fcntl",
    "fcntl64",
    "fdatasync",
    "fgetxattr",
    "flistxattr",
    "flock",
    "fork",
    "fremovexattr",
    "fsetxattr",
    "fstat",
    "fstat64",
    "fstatat64",
    "fstatfs",
    "fstatfs64",
    "fsync",
    "ftruncate",
    "ftruncate64",
    "futex",
    "futex_requeue",
    "futex_time64",
    "futex_wait",
    "futex_waitv",
    "futex_wake",
    "futimesat",
    "getcpu",
    "getcwd",
    "getdents",
    "getdents64",
    "getegid",
    "getegid32",
    "geteuid",
    "geteuid32",
    "getgid",
    "getgid32",
    "getgroups",
    "getgroups32",
    "getitimer",
    "getpeername",
    "getpgid",
    "getpgrp",
    "getpid",
    "getppid",
    "getpriority",
    "getrandom",
    "getresgid",
    "getresgid32",
    "getresuid",
    "getresuid32",
    "getrlimit",
    "get_robust_list",
    "getrusage",
    "getsid",
    "getsockname",
    "getsockopt",
    "get_thread_area",
    "gettid",
    "gettimeofday",
    "getuid",
    "getuid32",
    "getxattr",
    "getxattrat",
    "inotify_add_watch",
    "inotify_init",
    "inotify_init1",
    "inotify_rm_watch",
    "io_cancel",
    "ioctl",
    "io_destroy",
    "io_getevents",
    "io_pgetevents",
    "io_pgetevents_time64",
    "ioprio_get",
    "ioprio_set",
    "io_setup",
    "io_submit",
    "ipc",
    "kill",
    "landlock_add_rule",
    "landlock_create_ruleset",
    "landlock_restrict_self",
    "lchown",
    "lchown32",
    "lgetxattr",
    "link",
    "linkat",
    "listen",
    "listmount",
    "listxattr",
    "listxattrat",
    "llistxattr",
    "_llseek",
    "lremovexattr",
    "lseek",
    "lsetxattr",
    "lstat",
    "lstat64",
    "madvise",
    "map_shadow_stack",
    "membarrier",
    "memfd_create",
    "memfd_secret",
    "mincore",
    "mkdir",
    "mkdirat",
    "mknod",
    "mknodat",
    "mlock",
    "mlock2",
    "mlockall",
    "mmap",
    "mmap2",
    "mprotect",
    "mq_getsetattr",
    "mq_notify",
    "mq_open",
    "mq_timedreceive",
    "mq_timedreceive_time64",
    "mq_timedsend",
    "mq_timedsend_time64",
    "mq_unlink",
    "mremap",
    "mseal",
    "msgctl",
    "msgget",
    "msgrcv",
    "msgsnd",
    "msync",
    "munlock",
    "munlockall",
    "munmap",
    "name_to_handle_at",
    "nanosleep",
    "newfstatat",
    "_newselect",
    "open",
    "openat",
    "openat2",
    "pause",
    "pidfd_open",
    "pidfd_send_signal",
    "pipe",
    "pipe2",
    "pkey_alloc",
    "pkey_free",
    "pkey_mprotect",
    "poll",
    "ppoll",
    "ppoll_time64",
    "prctl",
    "pread64",
    "preadv",
    "preadv2",
    "prlimit64",
    "process_mrelease",
    "pselect6",
    "pselect6_time64",
    "pwrite64",
    "pwritev",
    "pwritev2",
    "read",
    "readahead",
    "readlink",
    "readlinkat",
    "readv",
    "recv",
    "recvfrom",
    "recvmmsg",
    "recvmmsg_time64",
    "recvmsg",
    "remap_file_pages",
    "removexattr",
    "removexattrat",
    "rename",
    "renameat",
    "renameat2",
    "restart_syscall",
    "riscv_hwprobe",
    "rmdir",
    "rseq",
    "rt_sigaction",
    "rt_sigpending",
    "rt_sigprocmask",
    "rt_sigqueueinfo",
    "rt_sigreturn",
    "rt_sigsuspend",
    "rt_sigtimedwait",
    "rt_sigtimedwait_time64",
    "rt_tgsigqueueinfo",
    "sched_getaffinity",
    "sched_getattr",
    "sched_getparam",
    "sched_get_priority_max",
    "sched_get_priority_min",
    "sched_getscheduler",
    "sched_rr_get_interval",
    "sched_rr_get_interval_time64",
    "sched_setaffinity",
    "sched_setattr",
    "sched_setparam",
    "sched_setscheduler",
    "sched_yield",
    "seccomp",
    "select",
    "semctl",
    "semget",
    "semop",
    "semtimedop",
    "semtimedop_time64",
    "send",
    "sendfile",
    "sendfile64",
    "sendmmsg",
    "sendmsg",
    "sendto",
    "setfsgid",
    "setfsgid32",
    "setfsuid",
    "setfsuid32",
    "setgid",
    "setgid32",
    "setgroups",
    "setgroups32",
    "setitimer",
    "setpgid",
    "setpriority",
    "setregid",
    "setregid32",
    "setresgid",
    "setresgid32",
    "setresuid",
    "setresuid32",
    "setreuid",
    "setreuid32",
    "setrlimit",
    "set_robust_list",
    "setsid",
    "setsockopt",
    "set_thread_area",
    "set_tid_address",
    "setuid",
    "setuid32",
    "setxattr",
    "setxattrat",
    "shmat",
    "shmctl",
    "shmdt",
    "shmget",
    "shutdown",
    "sigaltstack",
    "signalfd",
    "signalfd4",
    "sigprocmask",
    "sigreturn",
    "socketcall",
    "socketpair",
    "splice",
    "stat",
    "stat64",
    "statfs",
    "statfs64",
    "statmount",
    "statx",
    "symlink",
    "symlinkat",
    "sync",
    "sync_file_range",
    "syncfs",
    "sysinfo",
    "tee",
    "tgkill",
    "time",
    "timer_create",
    "timer_delete",
    "timer_getoverrun",
    "timer_gettime",
    "timer_gettime64",
    "timer_settime",
    "timer_settime64",
    "timerfd_create",
    "timerfd_gettime",
    "timerfd_gettime64",
    "timerfd_settime",
    "timerfd_settime64",
    "times",
    "tkill",
    "truncate",
    "truncate64",
    "ugetrlimit",
    "umask",
    "uname",
    "unlink",
    "unlinkat",
    "uretprobe",
    "utime",
    "utimensat",
    "utimensat_time64",
    "utimes",
    "vfork",
    "vmsplice",
    "wait4",
    "waitid",
    "waitpid",
    "write",
    "writev",
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    "arch_prctl",
];

fn create_seccomp_filter() -> std::io::Result<std::os::fd::OwnedFd> {
    // We could cache the fd and dup, but I'm worried that this might allow the child
    // to modify the memfd even when it is sealed, so we just recreate the bpf memfd from scratch

    let Ok(mut filter) = ScmpFilterContext::new(ScmpAction::Errno(libc::EPERM)) else {
        log::error!("Unable to init seccomp filter context");
        return Err(Error::new(ErrorKind::Other, "unable to init seccomp filter context"));
    };

    for syscall in ALLOWED_SYSCALLS {
        let Ok(syscall) = ScmpSyscall::from_name(*syscall) else {
            continue;
        };
        _ = filter.add_rule(ScmpAction::Allow, syscall);
    }
    if let Ok(syscall) = ScmpSyscall::from_name("clone") {
        let disallowed_clones = libc::CLONE_NEWNS | libc::CLONE_NEWUTS | libc::CLONE_NEWIPC | libc::CLONE_NEWUSER
            | libc::CLONE_NEWPID | libc::CLONE_NEWNET | libc::CLONE_NEWCGROUP;
        _ = filter.add_rule_conditional(ScmpAction::Allow, syscall, &[
           ScmpArgCompare::new(0, ScmpCompareOp::MaskedEqual(disallowed_clones as u64), 0)
        ]);
    }
    if let Ok(syscall) = ScmpSyscall::from_name("clone3") {
        _ = filter.add_rule(ScmpAction::Errno(libc::ENOSYS), syscall);
    }
    if let Ok(syscall) = ScmpSyscall::from_name("socket") {
        _ = filter.add_rule_conditional(ScmpAction::Allow, syscall, &[
           ScmpArgCompare::new(0, ScmpCompareOp::NotEqual, libc::AF_VSOCK as u64)
        ]);
    }
    if let Ok(syscall) = ScmpSyscall::from_name("personality") {
        _ = filter.add_rule_conditional(ScmpAction::Allow, syscall, &[
           ScmpArgCompare::new(0, ScmpCompareOp::Equal, 0x0)
        ]);
        _ = filter.add_rule_conditional(ScmpAction::Allow, syscall, &[
           ScmpArgCompare::new(0, ScmpCompareOp::Equal, 0x8)
        ]);
        _ = filter.add_rule_conditional(ScmpAction::Allow, syscall, &[
           ScmpArgCompare::new(0, ScmpCompareOp::Equal, 0x20000)
        ]);
        _ = filter.add_rule_conditional(ScmpAction::Allow, syscall, &[
           ScmpArgCompare::new(0, ScmpCompareOp::Equal, 0x20008)
        ]);
        _ = filter.add_rule_conditional(ScmpAction::Allow, syscall, &[
           ScmpArgCompare::new(0, ScmpCompareOp::Equal, 0xffffffff)
        ]);
    }
    if let Ok(syscall) = ScmpSyscall::from_name("ioctl") {
        _ = filter.add_rule_conditional(ScmpAction::Errno(libc::EPERM), syscall, &[
           ScmpArgCompare::new(1, ScmpCompareOp::MaskedEqual(0xFFFFFFFF), libc::TIOCSTI)
        ]);
    }
    if let Ok(syscall) = ScmpSyscall::from_name("ioctl") {
        _ = filter.add_rule_conditional(ScmpAction::Errno(libc::EPERM), syscall, &[
           ScmpArgCompare::new(1, ScmpCompareOp::MaskedEqual(0xFFFFFFFF), libc::TIOCLINUX)
        ]);
    }

    unsafe {
        let fd = cvt(libc::memfd_create(c"default-seccomp-bpf".as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING))? as RawFd;
        if filter.export_bpf(std::os::fd::BorrowedFd::borrow_raw(fd)).is_err() {
            log::error!("Unable to export bpf");
            return Err(Error::new(ErrorKind::Other, "unable to export bpf"));
        }
        libc::lseek(fd, 0, libc::SEEK_SET);
        cvt(libc::fcntl(fd, libc::F_ADD_SEALS, libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE))?;
        Ok(std::os::fd::OwnedFd::from_raw_fd(fd))
    }
}
