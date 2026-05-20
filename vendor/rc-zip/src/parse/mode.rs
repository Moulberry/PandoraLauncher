use std::fmt::{self, Write};

/// Mode represents a file's mode and permission bits.
/// The bits have the same definition on all systems,
/// but not all bits apply to all systems.
///
/// It is modelled after Go's `os.FileMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mode(pub u32);

impl Mode {
    /// d: is a directory
    pub const DIR: Self = Self(1 << 31);
    /// a: append-only
    pub const APPEND: Self = Self(1 << 30);
    /// l: exclusive use
    pub const EXCLUSIVE: Self = Self(1 << 29);
    /// T: temporary file; Plan 9 only
    pub const TEMPORARY: Self = Self(1 << 28);
    /// L: symbolic link
    pub const SYMLINK: Self = Self(1 << 27);
    /// D: device file
    pub const DEVICE: Self = Self(1 << 26);
    /// p: named pipe (FIFO)
    pub const NAMED_PIPE: Self = Self(1 << 25);
    /// S: Unix domain socket
    pub const SOCKET: Self = Self(1 << 24);
    /// u: setuid
    pub const SETUID: Self = Self(1 << 23);
    /// g: setgid
    pub const SETGID: Self = Self(1 << 22);
    /// c: Unix character device, when DEVICE is set
    pub const CHAR_DEVICE: Self = Self(1 << 21);
    /// t: sticky
    pub const STICKY: Self = Self(1 << 20);
    /// ?: non-regular file; nothing else is known
    pub const IRREGULAR: Self = Self(1 << 19);
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut empty = true;
        let pairs = [
            (Self::DIR, 'd'),
            (Self::APPEND, 'a'),
            (Self::EXCLUSIVE, 'l'),
            (Self::TEMPORARY, 'T'),
            (Self::SYMLINK, 'L'),
            (Self::DEVICE, 'D'),
            (Self::NAMED_PIPE, 'p'),
            (Self::SOCKET, 'S'),
            (Self::SETUID, 'u'),
            (Self::SETGID, 'g'),
            (Self::CHAR_DEVICE, 'c'),
            (Self::STICKY, 't'),
            (Self::IRREGULAR, '?'),
        ];
        for (flag, flag_char) in pairs {
            if self.has(flag) {
                f.write_char(flag_char)?;
                empty = false;
            }
        }
        if empty {
            write!(f, "-")?;
        }

        let rwx = "rwxrwxrwx";
        for (i, c) in rwx.char_indices() {
            if self.has(Mode(1 << (9 - 1 - i))) {
                write!(f, "{}", c)?;
            } else {
                write!(f, "-")?;
            }
        }

        Ok(())
    }
}

impl From<UnixMode> for Mode {
    fn from(m: UnixMode) -> Self {
        let mut mode = Mode(m.0 & 0o777);

        match m & UnixMode::IFMT {
            UnixMode::IFBLK => mode |= Mode::DEVICE,
            UnixMode::IFCHR => mode |= Mode::DEVICE & Mode::CHAR_DEVICE,
            UnixMode::IFDIR => mode |= Mode::DIR,
            UnixMode::IFIFO => mode |= Mode::NAMED_PIPE,
            UnixMode::IFLNK => mode |= Mode::SYMLINK,
            UnixMode::IFREG => { /* nothing to do */ }
            UnixMode::IFSOCK => mode |= Mode::SOCKET,
            _ => {}
        }

        if m.has(UnixMode::ISGID) {
            mode |= Mode::SETGID
        }
        if m.has(UnixMode::ISUID) {
            mode |= Mode::SETUID
        }
        if m.has(UnixMode::ISVTX) {
            mode |= Mode::STICKY
        }

        mode
    }
}

impl From<MsdosMode> for Mode {
    fn from(m: MsdosMode) -> Self {
        let mut mode = if m.has(MsdosMode::DIR) {
            Mode::DIR | Mode(0o777)
        } else {
            Mode(0o666)
        };
        if m.has(MsdosMode::READ_ONLY) {
            mode &= Mode(0o222);
        }

        mode
    }
}

impl From<u32> for Mode {
    fn from(u: u32) -> Self {
        Mode(u)
    }
}

/// UnixMode represents the file mode and permission bits for Unix systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnixMode(pub u32);

impl UnixMode {
    /// bit mask for the file type bit fields
    pub const IFMT: Self = Self(0xf000);

    /// the file is a socket
    pub const IFSOCK: Self = Self(0xc000);

    /// the file is a symbolic link
    pub const IFLNK: Self = Self(0xa000);

    /// the file is a regular file
    pub const IFREG: Self = Self(0x8000);

    /// the file is a block device
    pub const IFBLK: Self = Self(0x6000);

    /// the file is a directory
    pub const IFDIR: Self = Self(0x4000);

    /// the file is a character device
    pub const IFCHR: Self = Self(0x2000);

    /// the file is a FIFO
    pub const IFIFO: Self = Self(0x1000);

    /// the file is set-user-ID
    pub const ISUID: Self = Self(0x800);

    /// the file is set-group-ID
    pub const ISGID: Self = Self(0x400);

    /// the file is sticky
    pub const ISVTX: Self = Self(0x200);
}

impl From<u32> for UnixMode {
    fn from(u: u32) -> Self {
        UnixMode(u)
    }
}

/// MsdosMode represents the file mode and permission bits for MS-DOS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MsdosMode(pub u32);

impl MsdosMode {
    /// the file is a directory
    pub const DIR: Self = Self(0x10);

    /// the file is read-only
    pub const READ_ONLY: Self = Self(0x01);
}

impl From<u32> for MsdosMode {
    fn from(u: u32) -> Self {
        MsdosMode(u)
    }
}

macro_rules! derive_bitops {
    ($T: ty) => {
        impl std::ops::BitOr for $T {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self {
                Self(self.0 | rhs.0)
            }
        }

        impl std::ops::BitOrAssign for $T {
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl std::ops::BitAnd for $T {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self {
                Self(self.0 & rhs.0)
            }
        }

        impl std::ops::BitAndAssign for $T {
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }

        impl $T {
            /// Check if the mode has the given bits set.
            pub fn has(&self, rhs: Self) -> bool {
                self.0 & rhs.0 != 0
            }
        }
    };
}

derive_bitops!(Mode);
derive_bitops!(UnixMode);
derive_bitops!(MsdosMode);
