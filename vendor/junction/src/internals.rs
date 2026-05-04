pub(crate) mod c;
pub(crate) mod cast;
pub(crate) mod helpers;

use std::ffi::OsString;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::ptr::{addr_of_mut, copy_nonoverlapping};
use std::{cmp, fs, io, slice};

use cast::BytesAsReparseDataBuffer;

/// This prefix indicates to NTFS that the path is to be treated as a non-interpreted
/// path in the virtual file system.
/// Ref: <https://learn.microsoft.com/windows-hardware/drivers/kernel/object-manager>
const NT_PREFIX: [u16; 4] = helpers::utf16s(br"\??\");
/// Disables normalization and bypasses MAX_PATH.
/// Ref: <https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation?tabs=registry>
const VERBATIM_PREFIX: [u16; 4] = helpers::utf16s(br"\\?\");

pub(crate) const WCHAR_SIZE: u16 = size_of::<u16>() as _;

pub fn create(target: &Path, junction: &Path) -> io::Result<()> {
    const UNICODE_NULL_SIZE: u16 = WCHAR_SIZE;
    const MAX_PATH_BUFFER: u16 = c::MAXIMUM_REPARSE_DATA_BUFFER_SIZE as u16
        - c::REPARSE_DATA_BUFFER_HEADER_SIZE
        - c::MOUNT_POINT_REPARSE_BUFFER_HEADER_SIZE;

    // We're using low-level APIs to create the junction, and these are more picky about paths.
    // For example, forward slashes cannot be used as a path separator, so we should try to
    // canonicalize the path first.
    let target = helpers::get_full_path(target)?;
    // Strip Win32 verbatim prefix (\\?\) if present - we add NT prefix (\??\) ourselves
    let target = target.strip_prefix(VERBATIM_PREFIX.as_slice()).unwrap_or(&target);
    fs::create_dir(junction)?;
    let file = helpers::open_reparse_point(junction, true)?;

    // SubstituteName = "\??\" + target (NT path)
    let substitute_len_in_bytes = {
        let len = NT_PREFIX.len().saturating_add(target.len());
        let min_len = cmp::min(len, u16::MAX as usize) as u16;
        min_len.saturating_mul(WCHAR_SIZE)
    };

    // PrintName = target (Win32 path, without the \??\ prefix)
    let print_name_len_in_bytes = {
        let min_len = cmp::min(target.len(), u16::MAX as usize) as u16;
        min_len.saturating_mul(WCHAR_SIZE)
    };

    // Check for buffer overflow: both names + their null terminators must fit
    let total_path_buffer = substitute_len_in_bytes
        .saturating_add(UNICODE_NULL_SIZE)
        .saturating_add(print_name_len_in_bytes)
        .saturating_add(UNICODE_NULL_SIZE);
    if total_path_buffer > MAX_PATH_BUFFER {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "`target` is too long"));
    }

    // Redefine the above char array into a ReparseDataBuffer we can work with
    let mut data = BytesAsReparseDataBuffer::new();
    let rdb = data.as_mut_ptr();
    let in_buffer_size: u16 = unsafe {
        // Set the type of reparse point we are creating
        addr_of_mut!((*rdb).ReparseTag).write(c::IO_REPARSE_TAG_MOUNT_POINT);
        addr_of_mut!((*rdb).Reserved).write(0);

        // SubstituteName starts at offset 0 in PathBuffer
        addr_of_mut!((*rdb).ReparseBuffer.SubstituteNameOffset).write(0);
        addr_of_mut!((*rdb).ReparseBuffer.SubstituteNameLength).write(substitute_len_in_bytes);

        // PrintName starts right after SubstituteName + its null terminator
        addr_of_mut!((*rdb).ReparseBuffer.PrintNameOffset).write(substitute_len_in_bytes + UNICODE_NULL_SIZE);
        addr_of_mut!((*rdb).ReparseBuffer.PrintNameLength).write(print_name_len_in_bytes);

        let mut path_buffer_ptr: *mut u16 = addr_of_mut!((*rdb).ReparseBuffer.PathBuffer).cast();

        // Write SubstituteName: "\??\" + target
        copy_nonoverlapping(NT_PREFIX.as_ptr(), path_buffer_ptr, NT_PREFIX.len());
        path_buffer_ptr = path_buffer_ptr.add(NT_PREFIX.len());
        copy_nonoverlapping(target.as_ptr(), path_buffer_ptr, target.len());
        path_buffer_ptr = path_buffer_ptr.add(target.len());

        // Null terminator after SubstituteName
        path_buffer_ptr.write(0);
        path_buffer_ptr = path_buffer_ptr.add(1);

        // Write PrintName: target (Win32 path without \??\ prefix)
        copy_nonoverlapping(target.as_ptr(), path_buffer_ptr, target.len());
        path_buffer_ptr = path_buffer_ptr.add(target.len());

        // Null terminator after PrintName
        path_buffer_ptr.write(0);

        // Set the total size of the data buffer
        let size = c::MOUNT_POINT_REPARSE_BUFFER_HEADER_SIZE
            + substitute_len_in_bytes
            + UNICODE_NULL_SIZE
            + print_name_len_in_bytes
            + UNICODE_NULL_SIZE;
        addr_of_mut!((*rdb).ReparseDataLength).write(size);
        size.wrapping_add(c::REPARSE_DATA_BUFFER_HEADER_SIZE)
    };

    helpers::set_reparse_point(file.as_raw_handle(), rdb, u32::from(in_buffer_size))
}

pub fn delete(junction: &Path) -> io::Result<()> {
    let file = helpers::open_reparse_point(junction, true)?;
    helpers::delete_reparse_point(file.as_raw_handle())
}

pub fn exists(junction: &Path) -> io::Result<bool> {
    if !junction.exists() {
        return Ok(false);
    }
    let file = helpers::open_reparse_point(junction, false)?;
    // Allocate enough space to fit the maximum sized reparse data buffer
    let mut data = BytesAsReparseDataBuffer::new();
    // XXX: Could also use FindFirstFile to read the reparse point type
    // Ref https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-point-tags
    helpers::get_reparse_data_point(file.as_raw_handle(), data.as_mut_ptr())?;
    // SATETY: rdb should be initialized now
    let rdb = unsafe { data.assume_init() };
    // The reparse tag indicates if this is a junction or not
    Ok(rdb.ReparseTag == c::IO_REPARSE_TAG_MOUNT_POINT)
}

pub fn get_target(junction: &Path) -> io::Result<PathBuf> {
    // MSRV(1.63): use Path::try_exists instead
    if !junction.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "`junction` does not exist"));
    }
    let file = helpers::open_reparse_point(junction, false)?;
    let mut data = BytesAsReparseDataBuffer::new();
    helpers::get_reparse_data_point(file.as_raw_handle(), data.as_mut_ptr())?;
    // SAFETY: rdb should be initialized now
    let rdb = unsafe { data.assume_init() };
    if rdb.ReparseTag == c::IO_REPARSE_TAG_MOUNT_POINT {
        let offset = rdb.ReparseBuffer.SubstituteNameOffset / WCHAR_SIZE;
        let len = rdb.ReparseBuffer.SubstituteNameLength / WCHAR_SIZE;
        let wide = unsafe {
            let buf = rdb.ReparseBuffer.PathBuffer.as_ptr().add(offset as usize);
            slice::from_raw_parts(buf, len as usize)
        };
        // In case of "\??\C:\foo\bar"
        let wide = wide.strip_prefix(&NT_PREFIX).unwrap_or(wide);
        Ok(PathBuf::from(OsString::from_wide(wide)))
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "not a reparse tag mount point"))
    }
}
