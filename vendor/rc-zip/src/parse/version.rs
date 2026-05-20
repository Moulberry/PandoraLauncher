use ownable::{IntoOwned, ToOwned};
use std::fmt;
use winnow::{binary::le_u8, seq, PResult, Parser, Partial};

/// A zip version (either created by, or required when reading an archive).
///
/// Versions determine which features are supported by a tool, and
/// which features are required when reading a file.
///
/// For more information, see the [.ZIP Application Note](https://support.pkware.com/display/PKZIP/APPNOTE), section 4.4.2.
#[derive(Clone, Copy, ToOwned, IntoOwned, PartialEq, Eq, Hash)]
pub struct Version {
    /// The host system on which
    pub host_system: HostSystem,

    /// Integer version, e.g. 45 for Zip version 4.5
    /// See APPNOTE, section 4.4.2.1
    pub version: u8,
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} v{}", self.host_system, self.version)
    }
}

impl Version {
    /// Parse a version from a byte slice
    pub fn parser(i: &mut Partial<&'_ [u8]>) -> PResult<Self> {
        seq! {Self {
            version: le_u8,
            host_system: le_u8.map(HostSystem::from),
        }}
        .parse_next(i)
    }
}

/// System on which an archive was created, as encoded into a version u16.
///
/// See APPNOTE, section 4.4.2.2
#[derive(Debug, Clone, Copy, ToOwned, IntoOwned, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HostSystem {
    /// MS-DOS and OS/2 (FAT / VFAT / FAT32 file systems)
    MsDos = Self::MS_DOS,

    /// Amiga
    Amiga = Self::AMIGA,

    /// OpenVMS
    OpenVms = Self::OPEN_VMS,

    /// UNIX
    Unix = Self::UNIX,

    /// VM/CMS
    VmCms = Self::VM_CMS,

    /// Atari ST
    AtariSt = Self::ATARI_ST,

    /// OS/2 H.P.F.S
    Os2Hpfs = Self::OS2_HPFS,

    /// Macintosh (see `Osx`)
    Macintosh = Self::MACINTOSH,

    /// Z-System
    ZSystem = Self::Z_SYSTEM,

    /// CP/M
    CpM = Self::CP_M,

    /// Windows NTFS
    WindowsNtfs = Self::WINDOWS_NTFS,

    /// MVS (OS/390 - Z/OS)
    Mvs = Self::MVS,

    /// VSE
    Vse = Self::VSE,

    /// Acorn Risc
    AcornRisc = Self::ACORN_RISC,

    /// VFAT
    Vfat = Self::VFAT,

    /// alternate MVS
    AlternateMvs = Self::ALTERNATE_MVS,

    /// BeOS
    BeOs = Self::BE_OS,

    /// Tandem
    Tandem = Self::TANDEM,

    /// OS/400
    Os400 = Self::OS400,

    /// OS X (Darwin)
    Osx = Self::OSX,

    /// Unknown host system
    ///
    /// Values 20 through 255 are currently unused, as of
    /// APPNOTE.TXT 6.3.10
    Unknown(u8),
}

impl HostSystem {
    const MS_DOS: u8 = 0;
    const AMIGA: u8 = 1;
    const OPEN_VMS: u8 = 2;
    const UNIX: u8 = 3;
    const VM_CMS: u8 = 4;
    const ATARI_ST: u8 = 5;
    const OS2_HPFS: u8 = 6;
    const MACINTOSH: u8 = 7;
    const Z_SYSTEM: u8 = 8;
    const CP_M: u8 = 9;
    const WINDOWS_NTFS: u8 = 10;
    const MVS: u8 = 11;
    const VSE: u8 = 12;
    const ACORN_RISC: u8 = 13;
    const VFAT: u8 = 14;
    const ALTERNATE_MVS: u8 = 15;
    const BE_OS: u8 = 16;
    const TANDEM: u8 = 17;
    const OS400: u8 = 18;
    const OSX: u8 = 19;
}

impl From<u8> for HostSystem {
    fn from(u: u8) -> Self {
        match u {
            Self::MS_DOS => Self::MsDos,
            Self::AMIGA => Self::Amiga,
            Self::OPEN_VMS => Self::OpenVms,
            Self::UNIX => Self::Unix,
            Self::VM_CMS => Self::VmCms,
            Self::ATARI_ST => Self::AtariSt,
            Self::OS2_HPFS => Self::Os2Hpfs,
            Self::MACINTOSH => Self::Macintosh,
            Self::Z_SYSTEM => Self::ZSystem,
            Self::CP_M => Self::CpM,
            Self::WINDOWS_NTFS => Self::WindowsNtfs,
            Self::MVS => Self::Mvs,
            Self::VSE => Self::Vse,
            Self::ACORN_RISC => Self::AcornRisc,
            Self::VFAT => Self::Vfat,
            Self::ALTERNATE_MVS => Self::AlternateMvs,
            Self::BE_OS => Self::BeOs,
            Self::TANDEM => Self::Tandem,
            Self::OS400 => Self::Os400,
            Self::OSX => Self::Osx,
            u => Self::Unknown(u),
        }
    }
}

impl From<HostSystem> for u8 {
    fn from(host_system: HostSystem) -> Self {
        match host_system {
            HostSystem::MsDos => HostSystem::MS_DOS,
            HostSystem::Amiga => HostSystem::AMIGA,
            HostSystem::OpenVms => HostSystem::OPEN_VMS,
            HostSystem::Unix => HostSystem::UNIX,
            HostSystem::VmCms => HostSystem::VM_CMS,
            HostSystem::AtariSt => HostSystem::ATARI_ST,
            HostSystem::Os2Hpfs => HostSystem::OS2_HPFS,
            HostSystem::Macintosh => HostSystem::MACINTOSH,
            HostSystem::ZSystem => HostSystem::Z_SYSTEM,
            HostSystem::CpM => HostSystem::CP_M,
            HostSystem::WindowsNtfs => HostSystem::WINDOWS_NTFS,
            HostSystem::Mvs => HostSystem::MVS,
            HostSystem::Vse => HostSystem::VSE,
            HostSystem::AcornRisc => HostSystem::ACORN_RISC,
            HostSystem::Vfat => HostSystem::VFAT,
            HostSystem::AlternateMvs => HostSystem::ALTERNATE_MVS,
            HostSystem::BeOs => HostSystem::BE_OS,
            HostSystem::Tandem => HostSystem::TANDEM,
            HostSystem::Os400 => HostSystem::OS400,
            HostSystem::Osx => HostSystem::OSX,
            HostSystem::Unknown(u) => u,
        }
    }
}
