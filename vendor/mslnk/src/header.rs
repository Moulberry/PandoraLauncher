use bitflags::bitflags;
use byteorder::{ByteOrder, LE};

const CLSID: u128 = 0x460000000000_00c0_0000_0000_00021401;

/// A ShellLinkHeader structure (section 2.1), which contains identification
/// information, timestamps, and flags that specify the presence of optional
/// structures.
#[derive(Clone, Copy, Debug)]
pub struct ShellLinkHeader {
    /// A LinkFlags structure (section 2.1.1) that specifies information about the shell link and
    /// the presence of optional portions of the structure.
    link_flags: LinkFlags,
    /// A FileAttributesFlags structure (section 2.1.2) that specifies information about the link
    /// target.
    file_attributes: FileAttributeFlags,
    /// A FILETIME structure ([MS-DTYP]section 2.3.3) that specifies the creation time of the link
    /// target in UTC (Coordinated Universal Time). If the value is zero, there is no creation time
    /// set on the link target.
    creation_time: u64,
    /// A FILETIME structure ([MS-DTYP] section2.3.3) that specifies the access time of the link
    /// target in UTC (Coordinated Universal Time). If the value is zero, there is no access time
    /// set on the link target.
    access_time: u64,
    /// A FILETIME structure ([MS-DTYP] section 2.3.3) that specifies the write time of the link
    /// target in UTC (Coordinated Universal Time). If the value is zero, there is no write time
    /// set on the link target.
    write_time: u64,
    /// A 32-bit unsigned integer that specifies the size, in bytes, of the link target. If the
    /// link target fileis larger than 0xFFFFFFFF, this value specifies the least significant 32
    /// bits of the link target file size.
    file_size: u32,
    /// A 32-bit signed integer that specifies the index of an icon within a given icon location.
    icon_index: i32,
    /// A 32-bit unsigned integer that specifies the expected window state of an application
    /// launched by the link.
    show_command: ShowCommand,
    /// A HotkeyFlags structure (section 2.1.3) that specifies the keystrokes used to launch the
    /// application referenced by the shortcut key. This value is assigned to the application after
    /// it is launched, so that pressing the key activates that application.
    hotkey: HotkeyFlags,
}

impl ShellLinkHeader {
    /// Get the link flags
    pub fn link_flags(&self) -> &LinkFlags {
        &self.link_flags
    }

    /// Set the link flags
    pub fn set_link_flags(&mut self, link_flags: LinkFlags) {
        self.link_flags = link_flags;
    }

    /// Set some link flags
    pub fn update_link_flags(&mut self, link_flags: LinkFlags, value: bool) {
        self.link_flags.set(link_flags, value);
    }

    /// Get the file attributes
    pub fn file_attributes(&self) -> &FileAttributeFlags {
        &self.file_attributes
    }

    /// Set the file attributes
    pub fn set_file_attributes(&mut self, file_attributes: FileAttributeFlags) {
        self.file_attributes = file_attributes;
    }

    /// Get the file creation time
    pub fn creation_time(&self) -> u64 {
        self.creation_time
    }

    /// Set the file creation time
    pub fn set_creation_time(&mut self, creation_time: u64) {
        self.creation_time = creation_time;
    }

    /// Get the file access time
    pub fn access_time(&self) -> u64 {
        self.access_time
    }

    /// Set the file access time
    pub fn set_access_time(&mut self, access_time: u64) {
        self.access_time = access_time;
    }

    /// Get the file write time
    pub fn write_time(&self) -> u64 {
        self.write_time
    }

    /// Set the file write time
    pub fn set_write_time(&mut self, write_time: u64) {
        self.write_time = write_time;
    }

    /// The file size, or at least the least significant 32-bits of the
    /// size
    pub fn file_size(&self) -> u32 {
        self.file_size
    }

    /// Set the file size, or if bigger then 32-bits, set the least
    /// significant 32-bits
    pub fn set_file_size(&mut self, file_size: u32) {
        self.file_size = file_size;
    }

    /// Get the icon index
    pub fn icon_index(&self) -> i32 {
        self.icon_index
    }

    /// Set the icon index
    pub fn set_icon_index(&mut self, icon_index: i32) {
        self.icon_index = icon_index;
    }

    /// Get the show command
    pub fn show_command(&self) -> &ShowCommand {
        &self.show_command
    }

    /// Set the shortcut show command
    pub fn set_show_command(&mut self, show_command: ShowCommand) {
        self.show_command = show_command;
    }

    /// Get the hotkey flags
    pub fn hotkey(&self) -> &HotkeyFlags {
        &self.hotkey
    }

    /// Get a mutable pointer to the hotkey flags
    pub fn hotkey_mut(&mut self) -> &mut HotkeyFlags {
        &mut self.hotkey
    }
}

impl Default for ShellLinkHeader {
    /// Create a new, blank, ShellLinkHeader
    fn default() -> Self {
        Self {
            link_flags: LinkFlags::IS_UNICODE,
            file_attributes: FileAttributeFlags::FILE_ATTRIBUTE_NORMAL,
            creation_time: 0,
            access_time: 0,
            write_time: 0,
            file_size: 0,
            icon_index: 0,
            show_command: ShowCommand::ShowNormal,
            hotkey: HotkeyFlags::new(HotkeyKey::NoKeyAssigned, HotkeyModifiers::NO_MODIFIER),
        }
    }
}

impl Into<[u8; 0x4c]> for ShellLinkHeader {
    /// Write the data in this header to a `[u8]` for writing to the output file.
    fn into(self) -> [u8; 0x4c] {
        let mut header_data = [0u8; 0x4c];
        LE::write_u32(&mut header_data[0..], 0x4c);
        LE::write_u128(&mut header_data[4..], CLSID);
        LE::write_u32(&mut header_data[20..], self.link_flags.bits);
        LE::write_u32(&mut header_data[24..], self.file_attributes.bits);
        LE::write_u64(&mut header_data[28..], self.creation_time);
        LE::write_u64(&mut header_data[36..], self.access_time);
        LE::write_u64(&mut header_data[44..], self.write_time);
        LE::write_u32(&mut header_data[52..], self.file_size);
        LE::write_i32(&mut header_data[56..], self.icon_index);
        LE::write_u32(&mut header_data[60..], self.show_command as u32);
        LE::write_u16(&mut header_data[64..], self.hotkey.to_flags_u16());
        LE::write_u16(&mut header_data[66..], 0);
        LE::write_u32(&mut header_data[68..], 0);
        LE::write_u32(&mut header_data[72..], 0);
        header_data
    }
}

bitflags! {
    /// The LinkFlags structure defines bits that specify which shell linkstructures are present in
    /// the file format after the ShellLinkHeaderstructure (section 2.1).
    pub struct LinkFlags: u32 {
        /// The shell link is saved with an item ID list (IDList). If this bit is set, a
        /// LinkTargetIDList structure (section 2.2) MUST follow the ShellLinkHeader. If this bit
        /// is not set, this structure MUST NOT be present.
        const HAS_LINK_TARGET_ID_LIST           = 0b0000_0000_0000_0000_0000_0000_0000_0001;
        /// The shell link is saved with link information. If this bit is set, a LinkInfo structure
        /// (section 2.3) MUST be present. If this bit is not set, this structure MUST NOT be
        /// present.
        const HAS_LINK_INFO                     = 0b0000_0000_0000_0000_0000_0000_0000_0010;
        /// The shell link is saved with a name string. If this bit is set, a NAME_STRING
        /// StringData structure (section 2.4) MUST be present. If this bit is not set, this
        /// structure MUST NOT be present.
        const HAS_NAME                          = 0b0000_0000_0000_0000_0000_0000_0000_0100;
        /// The shell link is saved with a relative path string. If this bit is set, a
        /// RELATIVE_PATH StringData structure (section 2.4) MUST be present. If this bit is not
        /// set, this structure MUST NOT be present.
        const HAS_RELATIVE_PATH                 = 0b0000_0000_0000_0000_0000_0000_0000_1000;
        /// The shell link is saved with a relative path string. If this bit is set, a
        /// WORKING_DIR StringData structure (section 2.4) MUST be present. If this bit is not
        /// set, this structure MUST NOT be present.
        const HAS_WORKING_DIR                   = 0b0000_0000_0000_0000_0000_0000_0001_0000;
        /// The shell link is saved with a relative path string. If this bit is set, a
        /// COMMAND_LINE_ARGUMENTS StringData structure (section 2.4) MUST be present. If this bit
        /// is not set, this structure MUST NOT be present.
        const HAS_ARGUMENTS                     = 0b0000_0000_0000_0000_0000_0000_0010_0000;
        /// The shell link is saved with a relative path string. If this bit is set, a
        /// ICON_LOCATION StringData structure (section 2.4) MUST be present. If this bit is not
        /// set, this structure MUST NOT be present.
        const HAS_ICON_LOCATION                 = 0b0000_0000_0000_0000_0000_0000_0100_0000;
        /// The shell link contains Unicode encoded strings. This bit SHOULD be set. If this bit is
        /// set, the StringData section contains Unicode-encoded strings; otherwise, it contains
        /// strings that are encoded using the system default code page
        const IS_UNICODE                        = 0b0000_0000_0000_0000_0000_0000_1000_0000;
        /// The LinkInfo structure (section 2.3) is ignored.
        const FORCE_NO_LINK_INFO                = 0b0000_0000_0000_0000_0000_0001_0000_0000;
        /// The shell link is saved with an EnvironmentVariableDataBlock (section 2.5.4).
        const HAS_EXP_STRING                    = 0b0000_0000_0000_0000_0000_0010_0000_0000;
        /// The target is run in a separate virtual machine when launching a link target that is a
        /// 16-bit application.
        const RUN_IN_SEPARATE_PROCESS           = 0b0000_0000_0000_0000_0000_0100_0000_0000;
        /// A bit that is undefined and MUST be ignored.
        const UNUSED1                           = 0b0000_0000_0000_0000_0000_1000_0000_0000;
        /// The shell link is saved with a DarwinDataBlock(section2.5.3).
        const HAS_DARWIN_ID                     = 0b0000_0000_0000_0000_0001_0000_0000_0000;
        /// The application is run as a different user when the target of the shell link is
        /// activated.
        const RUN_AS_USER                       = 0b0000_0000_0000_0000_0010_0000_0000_0000;
        /// The shell link is saved with an IconEnvironmentDataBlock (section 2.5.5).
        const HAS_EXP_ICON                      = 0b0000_0000_0000_0000_0100_0000_0000_0000;
        /// The file system location is represented in the shell namespace when the path to an item
        /// is parsed into an IDList.
        const NO_PIDL_ALIAS                     = 0b0000_0000_0000_0000_1000_0000_0000_0000;
        /// A bit that is undefined and MUST be ignored.
        const UNUSED2                           = 0b0000_0000_0000_0001_0000_0000_0000_0000;
        /// The shell link is saved with a ShimDataBlock(section2.5.8)
        const RUN_WITH_SHIM_LAYER               = 0b0000_0000_0000_0010_0000_0000_0000_0000;
        /// The TrackerDataBlock(section2.5.10)is ignored.
        const FORCE_NO_LINK_TRACK               = 0b0000_0000_0000_0100_0000_0000_0000_0000;
        /// The shell link attempts to collect target properties and store them in the
        /// PropertyStoreDataBlock(section2.5.7) when the link target is set.
        const ENABLE_TARGET_METADATA            = 0b0000_0000_0000_1000_0000_0000_0000_0000;
        /// The EnvironmentVariableDataBlock is ignored.
        const DISABLE_LINK_PATH_TRACKING        = 0b0000_0000_0001_0000_0000_0000_0000_0000;
        /// The SpecialFolderDataBlock(section2.5.9)and the KnownFolderDataBlock(section2.5.6)are
        /// ignored when loading the shell link. If this bit is set, these extra data blocks SHOULD
        /// NOT be saved when saving the shell link.
        const DISABLE_KNOWN_FOLDER_TRACKING     = 0b0000_0000_0010_0000_0000_0000_0000_0000;
        /// If the linkhas a KnownFolderDataBlock(section2.5.6), the unaliased form of the known
        /// folder IDList SHOULD be used when translating the target IDList at the time that the
        /// link is loaded.
        const DISABLE_KNOWN_FOLDER_ALIAS        = 0b0000_0000_0100_0000_0000_0000_0000_0000;
        /// Creating a link that references another link is enabled. Otherwise, specifying a link
        /// as the target IDList SHOULD NOT be allowed.
        const ALLOW_LINK_TO_LINK                = 0b0000_0000_1000_0000_0000_0000_0000_0000;
        /// When saving a link for which the target IDList is under a known folder, either the
        /// unaliased form of that known folder or the target IDList SHOULD be used.
        const UNALIAS_ON_SAVE                   = 0b0000_0001_0000_0000_0000_0000_0000_0000;
        /// The target IDList SHOULD NOT be stored; instead, the path specified in the
        /// EnvironmentVariableDataBlock(section2.5.4) SHOULD be used to refer to the target.
        const PREFER_ENVIRONMENT_PATH           = 0b0000_0010_0000_0000_0000_0000_0000_0000;
        /// When the target is a UNC name that refers to a location on a local machine, the local
        /// path IDList in the PropertyStoreDataBlock(section2.5.7) SHOULD be stored, so it can be
        /// used when the link is loaded on the local machine.
        const KEEP_LOCAL_ID_LIST_FOR_UNC_TARGET = 0b0000_0100_0000_0000_0000_0000_0000_0000;
    }
}

bitflags! {
    /// The FileAttributesFlags structure defines bits that specify the file attributes of the link
    /// target, if the target is a file system item. File attributes can be used if the link target
    /// is not available, or if accessing the target would be inefficient. It is possible for the
    /// target items attributes to be out of sync with this value.
    pub struct FileAttributeFlags: u32 {
        /// The file or directory is read-only. For a file, if this bit is set, applications can read the file but cannot write to it or delete it. For a directory, if this bit is set, applications cannot delete the directory
        const FILE_ATTRIBUTE_READONLY               = 0b0000_0000_0000_0000_0000_0000_0000_0001;
        /// The file or directory is hidden. If this bit is set, the file or folder is not included in an ordinary directory listing.
        const FILE_ATTRIBUTE_HIDDEN                 = 0b0000_0000_0000_0000_0000_0000_0000_0010;
        /// The file or directory is part of the operating system or is used exclusively by the operating system.
        const FILE_ATTRIBUTE_SYSTEM                 = 0b0000_0000_0000_0000_0000_0000_0000_0100;
        /// A bit that MUST be zero.
        const RESERVED1                             = 0b0000_0000_0000_0000_0000_0000_0000_1000;
        /// The link target is a directory instead of a file.
        const FILE_ATTRIBUTE_DIRECTORY              = 0b0000_0000_0000_0000_0000_0000_0001_0000;
        /// The file or directory is an archive file. Applications use this flag to mark files for
        /// backup or removal.
        const FILE_ATTRIBUTE_ARCHIVE                = 0b0000_0000_0000_0000_0000_0000_0010_0000;
        /// A bit that MUST be zero.
        const RESERVED2                             = 0b0000_0000_0000_0000_0000_0000_0100_0000;
        /// The file or directory has no other flags set. If this bit is 1, all other bits in this
        /// structure MUST be clear.
        const FILE_ATTRIBUTE_NORMAL                 = 0b0000_0000_0000_0000_0000_0000_1000_0000;
        /// The file is being used for temporary storage.
        const FILE_ATTRIBUTE_TEMPORARY              = 0b0000_0000_0000_0000_0000_0001_0000_0000;
        /// The file is a sparse file.
        const FILE_ATTRIBUTE_SPARSE_FILE            = 0b0000_0000_0000_0000_0000_0010_0000_0000;
        /// The file or directory has an associated reparse point.
        const FILE_ATTRIBUTE_REPARSE_POINT          = 0b0000_0000_0000_0000_0000_0100_0000_0000;
        /// The file or directory is compressed. For a file, this means that all data in the file
        /// is compressed. For a directory, this means that compression is the default for newly
        /// created files and subdirectories.
        const FILE_ATTRIBUTE_COMPRESSED             = 0b0000_0000_0000_0000_0000_1000_0000_0000;
        /// The data of the file is not immediately available.
        const FILE_ATTRIBUTE_OFFLINE                = 0b0000_0000_0000_0000_0001_0000_0000_0000;
        /// The contents of the file need to be indexed.
        const FILE_ATTRIBUTE_NOT_CONTENT_INDEXED    = 0b0000_0000_0000_0000_0010_0000_0000_0000;
        /// The file or directory is encrypted. For a file, this means that all data in the file is encrypted. For a directory, this means that encryption is the default for newly created files and subdirectories.
        const FILE_ATTRIBUTE_ENCRYPTED              = 0b0000_0000_0000_0000_0100_0000_0000_0000;
    }
}

/// The HotkeyFlags structure specifies input generated by a combination of keyboard keys being
/// pressed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct HotkeyFlags {
    low_byte: HotkeyKey,
    high_byte: HotkeyModifiers,
}

impl HotkeyFlags {
    /// Create a new HotkeyFlags instance.
    pub fn new(low_byte: HotkeyKey, high_byte: HotkeyModifiers) -> Self {
        Self {
            low_byte,
            high_byte,
        }
    }

    /// Convert these HotkeyFlags to the u16 representation for saving.
    fn to_flags_u16(self) -> u16 {
        self.low_byte as u16 + ((self.high_byte.bits as u16) << 8)
    }

    /// The primary key assigned to the hotkey
    pub fn key(&self) -> &HotkeyKey {
        &self.low_byte
    }

    /// Set the hotkey primary key
    pub fn set_key(&mut self, key: HotkeyKey) {
        self.low_byte = key;
    }

    /// The modifiers (Shift, Ctrl, Alt) for this hotkey
    pub fn modifiers(&self) -> &HotkeyModifiers {
        &self.high_byte
    }

    /// Set the hotkey modifiers (Shift, Ctrl, Alt)
    pub fn set_modifiers(&mut self, modifiers: HotkeyModifiers) {
        self.high_byte = modifiers;
    }
}

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// An 8-bit unsigned integer that specifies a virtual key code that corresponds to a key on the
/// keyboard.
pub enum HotkeyKey {
    NoKeyAssigned = 0x00,
    Key0 = 0x30,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    KeyA = 0x41,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    F1 = 0x70,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    NumLock = 0x90,
    ScrollLock,
}

bitflags! {
    /// An 8-bit unsigned integer that specifies bits that correspond to modifier keys on the
    /// keyboard.
    pub struct HotkeyModifiers: u8 {
        /// No modifier key is being used.
        const NO_MODIFIER       = 0x00;
        /// The "SHIFT" key on the keyboard.
        const HOTKEYF_SHIFT     = 0x01;
        /// The "CTRL" key on the keyboard.
        const HOTKEYF_CONTROL   = 0x02;
        /// The "ALT" key on the keyboard.
        const HOTKEYF_ALT       = 0x04;
    }
}

/// The expected window state of an application launched by the link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShowCommand {
    /// The application is open and its window is open in a normal fashion.
    ShowNormal = 0x01,
    /// The application is open, and keyboard focus is given to the application, but its window is
    /// not shown.
    ShowMaximized = 0x03,
    /// The application is open, but its window is not shown. It is not given the keyboard focus.
    ShowMinNoActive = 0x07,
}
