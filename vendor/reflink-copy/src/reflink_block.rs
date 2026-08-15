use crate::sys;
use std::fs::File;
use std::io;
use std::num::NonZeroU64;

/// Creates a reflink of a specified block from one file to another.
///
/// This functionality is designed to be highly performant and does not perform any extra API calls.
/// It is expected that the user takes care of necessary preliminary checks and preparations.
///
/// If you need to clone an entire file, consider using the [`reflink`] or [`reflink_or_copy`]
/// functions instead.
///
/// > Note: Currently the function works only for windows and linux platforms. It returns `Err` for
///   any other platform.
///
/// # General restrictions
///
/// - The source and destination regions must begin and end at a cluster boundary.
/// - If the source and destination regions are in the same file, they must not overlap. (The
///   application may able to proceed by splitting up the block clone operation into multiple block
///   clones that no longer overlap.)
/// - `src_length` equal to 0 is not supported.
///
/// # Linux specific restrictions and remarks
///
/// - If the file size is not aligned to the cluster size, the reflink operation must not exceed
///   the file length. For example, to reflink the whole file with size of 7000 bytes, `src_length`
///   should be 7000 bytes.
///
/// More information about block cloning on Linux can be found by the
/// [link](https://www.man7.org/linux/man-pages/man2/ioctl_ficlonerange.2.html).
///
/// # Windows specific restrictions and remarks
///
/// - The destination region must not extend past the end of file. If the application wishes to
///   extend the destination with cloned data, it must first call
///   [`File::set_len`](fn@std::fs::File::set_len).
/// - The source and destination files must be on the same ReFS volume.
/// - The source and destination files must have the same Integrity Streams setting (that is,
///   Integrity Streams must be enabled in both files, or disabled in both files).
/// - If the source file is sparse, the destination file must also be sparse.
/// - The block clone operation will break Shared Opportunistic Locks (also known as Level 2
///   Opportunistic Locks).
/// - The ReFS volume must have been formatted with Windows Server 2016, and if Windows Failover
///   Clustering is in use, the Clustering Functional Level must have been Windows Server 2016 or
///   later at format time.
/// - If the file size is not aligned to the cluster size, the reflink operation should still
///   be aligned by the cluster size. For example, to reflink the whole file with size of 7000 bytes
///   and a cluster size of 4096 bytes, `src_length` should be 8192 bytes.
///
/// > Note: In order to handle blocks larger than 4GB,
///   [`ReflinkBlockBuilder::reflink_block`] splits these big blocks into smaller ones.
///   Each smaller block is 4GB minus the cluster size. This means there might be more than one API
///   call needed for the larger blocks.
///
/// More information about block cloning on Windows can be found by the
/// [link](https://learn.microsoft.com/en-us/windows/win32/fileio/block-cloning).
///
/// # Examples
///
/// The example below demonstrates how to create a new file reusing blocks from another file.
/// ```no_run
/// use std::fs::File;
/// use std::num::NonZeroU64;
///
/// fn shuffle() -> std::io::Result<()> {
///     let from_file = File::open("source.bin")?;
///     let to_file = File::create("destination.bin")?;
///     let cluster_size = NonZeroU64::new(4096).unwrap();
///     let len = cluster_size.get() * 2;
///
///     to_file.set_len(len)?;
///
///     reflink_copy::ReflinkBlockBuilder::new(&from_file, &to_file, cluster_size)
///         .from_offset(0)
///         .to_offset(cluster_size.get())
///         .reflink_block()?;
///
///     reflink_copy::ReflinkBlockBuilder::new(&from_file, &to_file, cluster_size)
///         .from_offset(cluster_size.get())
///         .to_offset(0)
///         .reflink_block()?;
///
///     Ok(())
/// }
/// ```
/// [`reflink`]: crate::reflink
/// [`reflink_or_copy`]: crate::reflink_or_copy
#[derive(Debug)]
pub struct ReflinkBlockBuilder<'from, 'to> {
    from: &'from File,
    from_offset: u64,
    to: &'to File,
    to_offset: u64,
    src_length: u64,
    cluster_size: Option<NonZeroU64>,
}

impl<'from, 'to> ReflinkBlockBuilder<'from, 'to> {
    /// Creates a new instance of [`ReflinkBlockBuilder`].
    pub fn new(from: &'from File, to: &'to File, src_length: NonZeroU64) -> Self {
        Self {
            from,
            from_offset: 0,
            to,
            to_offset: 0,
            src_length: src_length.get(),
            cluster_size: None,
        }
    }

    /// Sets the offset within the source file.
    #[must_use]
    pub fn from_offset(mut self, from_offset: u64) -> Self {
        self.from_offset = from_offset;
        self
    }

    /// Sets the offset within the destination file.
    #[must_use]
    pub fn to_offset(mut self, to_offset: u64) -> Self {
        self.to_offset = to_offset;
        self
    }

    /// Sets the cluster size. It is used to calculate the max block size of a single reflink call
    /// on Windows.
    #[must_use]
    pub fn cluster_size(mut self, cluster_size: NonZeroU64) -> Self {
        self.cluster_size = Some(cluster_size);
        self
    }

    /// Performs reflink operation for the specified block of data.
    pub fn reflink_block(self) -> io::Result<()> {
        sys::reflink_block(
            self.from,
            self.from_offset,
            self.to,
            self.to_offset,
            self.src_length,
            self.cluster_size,
        )
    }
}
