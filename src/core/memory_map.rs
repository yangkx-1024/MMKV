use crate::Error::IOError;
use crate::Result;
use std::fs::File;
use std::ops::{Deref, DerefMut, Range};
use std::os::fd::{AsRawFd, RawFd};
use std::ptr::NonNull;
use std::{io, ptr, slice};

const LOG_TAG: &str = "MMKV:MemoryMap";
const LEN_OFFSET: usize = 8;

#[cfg(any(target_os = "linux", target_os = "android"))]
const MAP_POPULATE: libc::c_int = libc::MAP_POPULATE;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
const MAP_POPULATE: libc::c_int = 0;

#[derive(Debug)]
struct RawMmap {
    ptr: NonNull<libc::c_void>,
    len: usize,
}

impl RawMmap {
    fn new(fd: RawFd, len: usize) -> io::Result<RawMmap> {
        unsafe {
            let ptr = libc::mmap(
                ptr::null_mut(),
                len as libc::size_t,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED | MAP_POPULATE,
                fd,
                0,
            );
            if ptr == libc::MAP_FAILED {
                Err(io::Error::last_os_error())
            } else {
                libc::madvise(ptr, len, libc::MADV_WILLNEED);
                Ok(RawMmap {
                    ptr: NonNull::new(ptr)
                        .ok_or_else(|| io::Error::other("mmap returned null pointer"))?,
                    len,
                })
            }
        }
    }

    fn flush(&self, len: usize) -> io::Result<()> {
        let result = unsafe { libc::msync(self.ptr.as_ptr(), len as libc::size_t, libc::MS_SYNC) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl Drop for RawMmap {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr.as_ptr(), self.len as libc::size_t);
        }
    }
}

impl Deref for RawMmap {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr() as *const u8, self.len) }
    }
}

impl DerefMut for RawMmap {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr() as *mut u8, self.len) }
    }
}

unsafe impl Send for RawMmap {}

unsafe impl Sync for RawMmap {}

#[derive(Debug)]
#[repr(transparent)]
pub struct MemoryMap(RawMmap);

impl Drop for MemoryMap {
    fn drop(&mut self) {
        let flush_len = self.write_offset();
        if flush_len > self.len() {
            error!(
                LOG_TAG,
                "skip flushing invalid mmap range, flush len {}, max len {}",
                flush_len,
                self.len()
            );
            return;
        }
        if let Err(e) = self.0.flush(flush_len) {
            error!(LOG_TAG, "failed to flush mmap on drop: {e}");
        }
    }
}

impl MemoryMap {
    pub fn new(file: &File, len: usize) -> Result<Self> {
        let raw_mmap = RawMmap::new(file.as_raw_fd(), len)
            .map_err(|e| IOError(format!("failed to create mmap with len {len}: {e}")))?;
        Ok(MemoryMap(raw_mmap))
    }

    pub fn append(&mut self, value: Vec<u8>) -> Result<()> {
        let data_len = value.len();
        let start = self.write_offset();
        let content_len = start - LEN_OFFSET;
        let end = start
            .checked_add(data_len)
            .ok_or_else(|| IOError("append overflowed target offset".to_string()))?;
        if end > self.len() {
            return Err(IOError(format!(
                "append out of bounds, start {}, data len {}, end {}, mmap len {}",
                start,
                data_len,
                end,
                self.len()
            )));
        }
        let new_content_len = content_len
            .checked_add(data_len)
            .ok_or_else(|| IOError("append overflowed content length".to_string()))?;
        self.0[0..LEN_OFFSET].copy_from_slice(new_content_len.to_be_bytes().as_slice());
        self.0[start..end].copy_from_slice(value.as_slice());
        Ok(())
    }

    pub fn reset(&mut self) {
        let len = 0usize;
        self.0[0..LEN_OFFSET].copy_from_slice(len.to_be_bytes().as_slice());
    }

    pub fn content_start_offset(&self) -> usize {
        LEN_OFFSET
    }

    /// The write offset of current mmap
    pub fn write_offset(&self) -> usize {
        usize::from_be_bytes(self.0[0..LEN_OFFSET].try_into().unwrap()) + LEN_OFFSET
    }

    /// The max len of current mmap
    pub fn len(&self) -> usize {
        self.0.len
    }

    pub fn read(&self, range: Range<usize>) -> &[u8] {
        self.0[range].as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;

    use crate::Error::IOError;

    use super::{MemoryMap, LEN_OFFSET};

    #[test]
    fn test_mmap() {
        let _ = fs::remove_file("test_mmap");
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .open("test_mmap")
            .unwrap();
        file.set_len(1024).unwrap();
        let mut mm = MemoryMap::new(&file, 1024).unwrap();
        assert_eq!(mm.write_offset(), LEN_OFFSET);
        mm.append(vec![1, 2, 3]).unwrap();
        mm.append(vec![4]).unwrap();
        assert_eq!(mm.write_offset(), 12);

        let read = mm.read(8..10);
        assert_eq!(read.len(), 2);
        assert_eq!(read[0], 1);
        assert_eq!(read[1], 2);
        let read = mm.read(mm.write_offset() - 1..mm.write_offset());
        assert_eq!(read[0], 4);

        mm.reset();
        mm.append(vec![5, 4, 3, 2, 1]).unwrap();
        assert_eq!(mm.write_offset(), 13);
        let read = mm.read(8..9);
        assert_eq!(read[0], 5);

        let read = mm.read(9..10);
        assert_eq!(read[0], 4);
        let _ = fs::remove_file("test_mmap");
    }

    #[test]
    fn test_mmap_append_out_of_bounds() {
        let _ = fs::remove_file("test_mmap_append_out_of_bounds");
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .open("test_mmap_append_out_of_bounds")
            .unwrap();
        file.set_len((LEN_OFFSET + 1) as u64).unwrap();
        let mut mm = MemoryMap::new(&file, LEN_OFFSET + 1).unwrap();

        let err = mm.append(vec![1, 2]).unwrap_err();
        assert_eq!(
            err,
            IOError(format!(
                "append out of bounds, start {}, data len {}, end {}, mmap len {}",
                LEN_OFFSET,
                2,
                LEN_OFFSET + 2,
                LEN_OFFSET + 1
            ))
        );

        let _ = fs::remove_file("test_mmap_append_out_of_bounds");
    }
}
