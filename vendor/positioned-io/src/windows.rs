extern crate winapi;

use std::{
    cmp::min,
    fs::File,
    io,
    io::{Seek, SeekFrom, Write},
    mem,
    os::windows::{fs::FileExt, io::AsRawHandle},
    ptr,
};

use self::winapi::{
    shared::{
        basetsd::SIZE_T,
        minwindef::{BOOL, DWORD},
    },
    um::{
        handleapi::CloseHandle,
        memoryapi::{CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ},
        sysinfoapi::GetSystemInfo,
        winnt::{HANDLE, PAGE_READONLY},
    },
};
use super::{ReadAt, WriteAt};

fn result(e: BOOL) -> io::Result<()> {
    if e == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn allocation_granularity() -> usize {
    unsafe {
        let mut info = mem::zeroed();
        GetSystemInfo(&mut info);
        info.dwAllocationGranularity as usize
    }
}

impl ReadAt for File {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<usize> {
        let file_len = self.metadata()?.len();

        if buf.is_empty() || pos >= file_len {
            return Ok(0);
        }

        let len = min(file_len - pos, buf.len() as u64) as usize;

        unsafe {
            let alignment = pos % allocation_granularity() as u64;
            let aligned_pos = pos - alignment;
            let aligned_len = len + alignment as usize;

            let mapping = CreateFileMappingW(
                self.as_raw_handle() as HANDLE,
                ptr::null_mut(),
                PAGE_READONLY,
                0,
                0,
                ptr::null(),
            );

            if mapping.is_null() {
                return Err(io::Error::last_os_error());
            }

            let aligned_ptr = MapViewOfFile(
                mapping,
                FILE_MAP_READ,
                (aligned_pos >> 32) as DWORD,
                (aligned_pos & 0xffff_ffff) as DWORD,
                aligned_len as SIZE_T,
            );

            CloseHandle(mapping);

            if aligned_ptr.is_null() {
                return Err(io::Error::last_os_error());
            }

            let ptr = (aligned_ptr as *const u8).offset(alignment as isize);
            ptr::copy_nonoverlapping(ptr, buf.as_mut_ptr(), len);

            result(UnmapViewOfFile(aligned_ptr))?;
        }

        Ok(len)
    }
}

impl WriteAt for File {
    fn write_at(&mut self, pos: u64, buf: &[u8]) -> io::Result<usize> {
        let cursor = self.seek(SeekFrom::Current(0))?;
        let result = self.seek_write(buf, pos)?;
        self.seek(SeekFrom::Start(cursor))?;
        Ok(result)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(self)
    }
}
