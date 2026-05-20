use std::mem::offset_of;

/// Reparse Data Buffer header size
pub const REPARSE_DATA_BUFFER_HEADER_SIZE: u16 = offset_of!(super::REPARSE_DATA_BUFFER, ReparseBuffer) as u16;
/// Reparse GUID Data Buffer header size
pub const REPARSE_GUID_DATA_BUFFER_HEADER_SIZE: u16 =
    offset_of!(super::REPARSE_GUID_DATA_BUFFER, GenericReparseBuffer) as u16;
/// MountPointReparseBuffer header size
pub const MOUNT_POINT_REPARSE_BUFFER_HEADER_SIZE: u16 = offset_of!(super::MountPointReparseBuffer, PathBuffer) as u16;
