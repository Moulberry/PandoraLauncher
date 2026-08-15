use super::utility::AutoRemovedFile;
use crate::ReflinkSupport;
use std::num::NonZeroU64;

use std::{
    convert::TryInto,
    ffi::c_void,
    fs::File,
    io,
    mem::{self, MaybeUninit},
    os::windows::{ffi::OsStrExt, fs::MetadataExt, io::AsRawHandle},
    path::Path,
};

use windows::core::PCWSTR;
use windows::Win32::{
    Foundation::{HANDLE, MAX_PATH},
    Storage::FileSystem::{
        GetVolumeInformationByHandleW, GetVolumeInformationW, GetVolumeNameForVolumeMountPointW,
        GetVolumePathNameW, FILE_ATTRIBUTE_SPARSE_FILE, FILE_FLAGS_AND_ATTRIBUTES,
    },
    System::{
        Ioctl::{
            DUPLICATE_EXTENTS_DATA, FSCTL_DUPLICATE_EXTENTS_TO_FILE,
            FSCTL_GET_INTEGRITY_INFORMATION, FSCTL_GET_INTEGRITY_INFORMATION_BUFFER,
            FSCTL_SET_INTEGRITY_INFORMATION, FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
            FSCTL_SET_SPARSE,
        },
        SystemServices::FILE_SUPPORTS_BLOCK_REFCOUNTING,
        IO::DeviceIoControl,
    },
};

pub fn reflink(from: &Path, to: &Path) -> io::Result<()> {
    // Inspired by https://github.com/0xbadfca11/reflink/blob/master/reflink.cpp
    let src = File::open(from)?;

    let src_metadata = src.metadata()?;
    let src_file_size = src_metadata.file_size();
    let src_is_sparse =
        (FILE_FLAGS_AND_ATTRIBUTES(src_metadata.file_attributes()) & FILE_ATTRIBUTE_SPARSE_FILE).0
            != 0;

    let dest = AutoRemovedFile::create_new(to)?;

    // Set the destination to be sparse while we clone.
    // Important to avoid allocating zero-backed real storage when cloning
    // below which will just be released when cloning file extents.
    dest.set_sparse()?;

    let src_integrity_info = src.get_integrity_information()?;
    let cluster_size: i64 = src_integrity_info.ClusterSizeInBytes.into();
    if cluster_size != 0 {
        if cluster_size != 4 * 1024 && cluster_size != 64 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cluster size of source must either be 4K or 64K (restricted by ReFS)",
            ));
        }
        // Copy over integrity information. Not sure if this is required.
        let mut dest_integrity_info = FSCTL_SET_INTEGRITY_INFORMATION_BUFFER {
            ChecksumAlgorithm: src_integrity_info.ChecksumAlgorithm,
            Reserved: src_integrity_info.Reserved,
            Flags: src_integrity_info.Flags,
        };

        // ignore the error if it fails, the clone will still work
        if let Err(_e) = dest.set_integrity_information(&mut dest_integrity_info) {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                ?_e,
                "Failed to set integrity information (probably on DevDriver), but the clone still works"
            );
        }
    }

    // file_size must be sufficient to hold the data.
    // TODO test if the current implementation works:
    // Later on, we round up the bytes to copy in order to end at a cluster boundary.
    // This might very well result in us cloning past the file end.
    // Let's hope windows api sanitizes this, because otherwise a clean implementation is not really possible.
    dest.as_inner_file().set_len(src_file_size)?;

    // We must end at a cluster boundary
    let total_copy_len: i64 = {
        if cluster_size == 0 {
            src_file_size.try_into().unwrap()
        } else {
            // Round to the next cluster size
            round_up(src_file_size.try_into().unwrap(), cluster_size)
        }
    };

    let cluster_size = if cluster_size != 0 {
        Some(NonZeroU64::new(cluster_size as u64).unwrap())
    } else {
        None
    };
    reflink_block(
        &src,
        0,
        dest.as_inner_file(),
        0,
        total_copy_len as u64,
        cluster_size,
    )?;
    if !src_is_sparse {
        dest.unset_sparse()?;
    }

    dest.persist(src_metadata.permissions())
}

/// Additional functionality for windows files, needed for reflink
trait FileExt {
    fn set_sparse(&self) -> io::Result<()>;
    fn unset_sparse(&self) -> io::Result<()>;
    fn get_integrity_information(&self) -> io::Result<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER>;
    fn set_integrity_information(
        &self,
        integrity_info: &mut FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
    ) -> io::Result<()>;
    fn is_block_cloning_supported(&self) -> io::Result<bool>;

    fn as_handle(&self) -> HANDLE;
}

impl FileExt for File {
    fn set_sparse(&self) -> io::Result<()> {
        let mut bytes_returned = 0u32;
        unsafe {
            DeviceIoControl(
                self.as_handle(),
                FSCTL_SET_SPARSE,
                None,
                0,
                None,
                0,
                Some(&mut bytes_returned as *mut _),
                None,
            )
        }?;

        Ok(())
    }

    fn unset_sparse(&self) -> io::Result<()> {
        let mut bytes_returned = 0u32;
        let mut sparse_flag: u32 = 0;

        unsafe {
            DeviceIoControl(
                self.as_handle(),
                FSCTL_SET_SPARSE,
                Some(&mut sparse_flag as *mut _ as *mut c_void),
                mem::size_of::<u32>() as u32,
                None,
                0,
                Some(&mut bytes_returned as *mut _),
                None,
            )
        }?;

        Ok(())
    }

    fn get_integrity_information(&self) -> io::Result<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER> {
        let mut bytes_returned = 0u32;
        let mut integrity_info: MaybeUninit<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER> =
            MaybeUninit::uninit();

        unsafe {
            DeviceIoControl(
                self.as_handle(),
                FSCTL_GET_INTEGRITY_INFORMATION,
                None,
                0,
                Some(integrity_info.as_mut_ptr() as *mut c_void),
                mem::size_of::<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER>()
                    .try_into()
                    .unwrap(),
                Some(&mut bytes_returned as *mut _),
                None,
            )?;

            Ok(integrity_info.assume_init())
        }
    }

    fn set_integrity_information(
        &self,
        integrity_info: &mut FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
    ) -> io::Result<()> {
        unsafe {
            DeviceIoControl(
                self.as_handle(),
                FSCTL_SET_INTEGRITY_INFORMATION,
                Some(integrity_info as *mut _ as *mut c_void),
                mem::size_of::<FSCTL_SET_INTEGRITY_INFORMATION_BUFFER>()
                    .try_into()
                    .unwrap(),
                None,
                0,
                None,
                None,
            )
        }?;
        Ok(())
    }

    fn is_block_cloning_supported(&self) -> io::Result<bool> {
        let mut flags = 0u32;
        unsafe {
            GetVolumeInformationByHandleW(
                self.as_handle(),
                None,
                None,
                None,
                Some(&mut flags as *mut _),
                None,
            )
        }?;
        Ok((flags & FILE_SUPPORTS_BLOCK_REFCOUNTING) != 0)
    }

    fn as_handle(&self) -> HANDLE {
        HANDLE(self.as_raw_handle())
    }
}

impl FileExt for AutoRemovedFile {
    fn set_sparse(&self) -> io::Result<()> {
        self.as_inner_file().set_sparse()
    }

    fn unset_sparse(&self) -> io::Result<()> {
        self.as_inner_file().unset_sparse()
    }

    fn get_integrity_information(&self) -> io::Result<FSCTL_GET_INTEGRITY_INFORMATION_BUFFER> {
        self.as_inner_file().get_integrity_information()
    }

    fn set_integrity_information(
        &self,
        integrity_info: &mut FSCTL_SET_INTEGRITY_INFORMATION_BUFFER,
    ) -> io::Result<()> {
        self.as_inner_file()
            .set_integrity_information(integrity_info)
    }

    fn is_block_cloning_supported(&self) -> io::Result<bool> {
        self.as_inner_file().is_block_cloning_supported()
    }

    fn as_handle(&self) -> HANDLE {
        self.as_inner_file().as_handle()
    }
}

/// Rounds `num_to_round` to the next multiple of `multiple`
///
/// # Precondition
///  - `multiple` > 0
///  - `mutliple` is a power of 2
fn round_up(num_to_round: i64, multiple: i64) -> i64 {
    debug_assert!(multiple > 0);
    debug_assert_eq!((multiple & (multiple - 1)), 0);
    (num_to_round + multiple - 1) & -multiple
}

/// Checks whether reflink is supported on the filesystem for the specified source and target paths.
///
/// This function verifies that both paths are on the same volume and that the filesystem supports
/// reflink.
pub fn check_reflink_support(
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
) -> io::Result<ReflinkSupport> {
    let from_volume = get_volume_path(from)?;
    let to_volume = get_volume_path(to)?;

    let from_guid = get_volume_guid_path(&from_volume)?;
    let to_guid = get_volume_guid_path(&to_volume)?;
    if from_guid != to_guid {
        // The source and destination files must be on the same volume
        return Ok(ReflinkSupport::NotSupported);
    }

    let volume_flags = get_volume_flags(&from_volume)?;
    if volume_flags & FILE_SUPPORTS_BLOCK_REFCOUNTING != 0 {
        Ok(ReflinkSupport::Supported)
    } else {
        Ok(ReflinkSupport::NotSupported)
    }
}

/// A wrapper function for
/// [GetVolumePathNameW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumepathnamew)
/// that retrieves the volume mount point where the specified path is mounted.
fn get_volume_path(path: impl AsRef<Path>) -> io::Result<Vec<u16>> {
    let path_wide: Vec<u16> = path
        .as_ref()
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut volume_name_buffer = vec![0u16; MAX_PATH as usize];

    unsafe { GetVolumePathNameW(PCWSTR(path_wide.as_ptr()), volume_name_buffer.as_mut()) }?;

    if let Some(pos) = volume_name_buffer.iter().position(|&c| c == 0) {
        volume_name_buffer.truncate(pos);
    }

    Ok(volume_name_buffer)
}

/// A wrapper function for
/// [GetVolumeNameForVolumeMountPointW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumenameforvolumemountpointw)
/// that retrieves a volume GUID path for the volume that is associated with the specified volume
/// mount point (drive letter, volume GUID path, or mounted folder).
fn get_volume_guid_path(volume_path_w: &[u16]) -> io::Result<Vec<u16>> {
    let mut volume_guid_path = vec![0u16; 50usize];
    unsafe {
        GetVolumeNameForVolumeMountPointW(PCWSTR(volume_path_w.as_ptr()), volume_guid_path.as_mut())
    }?;

    if let Some(pos) = volume_guid_path.iter().position(|&c| c == 0) {
        volume_guid_path.truncate(pos);
    }

    Ok(volume_guid_path)
}

/// A wrapper function for
/// [GetVolumeInformationW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationw)
/// that returns `FileSystemFlags`.
fn get_volume_flags(volume_path_w: &[u16]) -> io::Result<u32> {
    let mut file_system_flags = 0u32;

    unsafe {
        GetVolumeInformationW(
            PCWSTR(volume_path_w.as_ptr()),
            None,
            None,
            None,
            Some(&mut file_system_flags as *mut _),
            None,
        )
    }?;

    Ok(file_system_flags)
}

pub(crate) fn reflink_block(
    from: &File,
    from_offset: u64,
    to: &File,
    to_offset: u64,
    src_length: u64,
    cluster_size: Option<NonZeroU64>,
) -> io::Result<()> {
    const GB: u64 = 1024u64 * 1024 * 1024;
    const MAX_REFS_CLUSTER_SIZE: u64 = 64 * 1024;

    // Must be smaller than 4GB; This is always a multiple of ClusterSize
    let max_io_size = 4u64 * GB
        - cluster_size
            .map(NonZeroU64::get)
            .unwrap_or(MAX_REFS_CLUSTER_SIZE);

    let mut bytes_copied = 0;
    while bytes_copied < src_length {
        let bytes_to_copy = max_io_size.min(src_length - bytes_copied);
        if let Some(cluster_size) = cluster_size {
            debug_assert_eq!(bytes_to_copy % cluster_size, 0);
            debug_assert_eq!(bytes_copied % cluster_size, 0);
        }

        duplicate_extent_to_file(
            from,
            from_offset + bytes_copied,
            to,
            to_offset + bytes_copied,
            bytes_to_copy,
        )?;

        bytes_copied += bytes_to_copy;
    }

    Ok(())
}

fn duplicate_extent_to_file(
    from: &File,
    from_offset: u64,
    to: &File,
    to_offset: u64,
    src_length: u64,
) -> io::Result<()> {
    let mut dup_extent = DUPLICATE_EXTENTS_DATA {
        FileHandle: from.as_handle(),
        SourceFileOffset: from_offset as i64,
        TargetFileOffset: to_offset as i64,
        ByteCount: src_length as i64,
    };

    let mut bytes_returned = 0u32;
    unsafe {
        DeviceIoControl(
            to.as_handle(),
            FSCTL_DUPLICATE_EXTENTS_TO_FILE,
            Some(&mut dup_extent as *mut _ as *mut c_void),
            size_of::<DUPLICATE_EXTENTS_DATA>().try_into().unwrap(),
            None,
            0,
            Some(&mut bytes_returned as *mut _),
            None,
        )
    }?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_round_up() {
        assert_eq!(round_up(0, 2), 0);
        assert_eq!(round_up(1, 2), 2);
        assert_eq!(round_up(2, 2), 2);

        assert_eq!(round_up(15, 8), 16);
        assert_eq!(round_up(17, 8), 24);

        assert_eq!(round_up(100000, 4096), 102400);
        assert_eq!(round_up(100000, 65536), 131072);
    }

    #[test]
    #[should_panic]
    fn test_invalid_multiple_zero() {
        round_up(10, 0);
    }
    #[test]
    #[should_panic]
    fn test_invalid_multiple_non_power_of_two() {
        round_up(10, 3);
    }

    #[test]
    fn test_get_volume_path_is_same() -> io::Result<()> {
        let src_volume_path = get_volume_path("./src")?;
        let tests_volume_path = get_volume_path("./tests")?;
        assert_eq!(src_volume_path, tests_volume_path);

        Ok(())
    }

    #[test]
    fn test_get_volume_guid() -> io::Result<()> {
        let volume_path = get_volume_path(".")?;

        let re = regex::Regex::new(r"\\\\\?\\Volume\{.{8}-.{4}-.{4}-.{4}-.{12}\}\\").unwrap();
        let volume_guid = get_volume_guid_path(&volume_path)?;
        let volume_guid = String::from_utf16(&volume_guid).unwrap();
        assert!(re.is_match(&volume_guid));

        Ok(())
    }

    #[test]
    fn test_get_volume_flags() -> io::Result<()> {
        let volume_path = get_volume_path(".")?;
        let volume_flags = get_volume_flags(&volume_path)?;
        assert!(volume_flags > 0);
        Ok(())
    }
}
