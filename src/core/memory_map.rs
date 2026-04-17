use crate::Error::IOError;
use crate::Result;
use std::fs::File;
use std::mem::size_of;
use std::ops::{Deref, DerefMut, Range};
use std::os::fd::{AsRawFd, RawFd};
use std::ptr::NonNull;
use std::{io, ptr, slice};

const LOG_TAG: &str = "MMKV:MemoryMap";
const LEN_OFFSET: usize = size_of::<u64>();

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
        let flush_len = match self.write_offset() {
            Ok(flush_len) => flush_len,
            Err(e) => {
                error!(
                    LOG_TAG,
                    "skip flushing mmap on drop due to invalid header: {:?}", e
                );
                return;
            }
        };
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
    pub fn new(file: &File, len: u64) -> Result<Self> {
        if len < LEN_OFFSET as u64 {
            return Err(IOError(format!(
                "failed to create mmap with len {len}: mmap length is smaller than header {LEN_OFFSET}"
            )));
        }
        let mmap_len = usize::try_from(len).map_err(|_| {
            IOError(format!(
                "failed to create mmap with len {len}: exceeds platform usize"
            ))
        })?;
        let raw_mmap = RawMmap::new(file.as_raw_fd(), mmap_len)
            .map_err(|e| IOError(format!("failed to create mmap with len {len}: {e}")))?;
        Ok(MemoryMap(raw_mmap))
    }

    pub fn append(&mut self, value: &[u8]) -> Result<()> {
        let data_len = value.len();
        let start = self.write_offset()?;
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
        let new_content_len = u64::try_from(new_content_len)
            .map_err(|_| IOError("append overflowed stored content length".to_string()))?;
        self.write_content_len(new_content_len);
        self.0[start..end].copy_from_slice(value);
        Ok(())
    }

    pub fn reset(&mut self) {
        self.write_content_len(0);
    }

    pub fn content_start_offset(&self) -> usize {
        LEN_OFFSET
    }

    /// The write offset of current mmap
    pub fn write_offset(&self) -> Result<usize> {
        self.content_len()?
            .checked_add(LEN_OFFSET)
            .ok_or_else(|| IOError("invalid mmap write offset overflow".to_string()))
    }

    /// The max len of current mmap
    pub fn len(&self) -> usize {
        self.0.len
    }

    pub fn read(&self, range: Range<usize>) -> Result<&[u8]> {
        if range.start > range.end || range.end > self.len() {
            return Err(IOError(format!(
                "read out of bounds, range {}..{}, mmap len {}",
                range.start,
                range.end,
                self.len()
            )));
        }
        Ok(self.0[range].as_ref())
    }

    fn content_len(&self) -> Result<usize> {
        let content_len = self.read_content_len();
        let max_content_len = self.payload_capacity() as u64;
        if content_len > max_content_len {
            return Err(IOError(format!(
                "invalid mmap content length {content_len}, max {max_content_len}"
            )));
        }
        // Safe: content_len <= payload_capacity() which is usize, so cast never truncates
        Ok(content_len as usize)
    }

    fn payload_capacity(&self) -> usize {
        self.len() - LEN_OFFSET
    }

    fn read_content_len(&self) -> u64 {
        u64::from_be_bytes(
            self.0[0..LEN_OFFSET]
                .try_into()
                .expect("mmap header slice length must match u64"),
        )
    }

    fn write_content_len(&mut self, content_len: u64) {
        self.0[0..LEN_OFFSET].copy_from_slice(&content_len.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;

    use crate::Error::IOError;

    use super::{LEN_OFFSET, MemoryMap};

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
        assert_eq!(mm.write_offset().unwrap(), LEN_OFFSET);
        mm.append(&[1, 2, 3]).unwrap();
        mm.append(&[4]).unwrap();
        assert_eq!(mm.write_offset().unwrap(), 12);

        let read = mm.read(8..10).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0], 1);
        assert_eq!(read[1], 2);
        let write_offset = mm.write_offset().unwrap();
        let read = mm.read(write_offset - 1..write_offset).unwrap();
        assert_eq!(read[0], 4);

        mm.reset();
        mm.append(&[5, 4, 3, 2, 1]).unwrap();
        assert_eq!(mm.write_offset().unwrap(), 13);
        let read = mm.read(8..9).unwrap();
        assert_eq!(read[0], 5);

        let read = mm.read(9..10).unwrap();
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
        let mut mm = MemoryMap::new(&file, (LEN_OFFSET + 1) as u64).unwrap();

        let err = mm.append(&[1, 2]).unwrap_err();
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

    #[test]
    fn test_mmap_read_out_of_bounds() {
        let _ = fs::remove_file("test_mmap_read_out_of_bounds");
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .open("test_mmap_read_out_of_bounds")
            .unwrap();
        file.set_len((LEN_OFFSET + 1) as u64).unwrap();
        let mm = MemoryMap::new(&file, (LEN_OFFSET + 1) as u64).unwrap();

        let err = mm.read(LEN_OFFSET..LEN_OFFSET + 2).unwrap_err();
        assert_eq!(
            err,
            IOError(format!(
                "read out of bounds, range {}..{}, mmap len {}",
                LEN_OFFSET,
                LEN_OFFSET + 2,
                LEN_OFFSET + 1
            ))
        );

        let _ = fs::remove_file("test_mmap_read_out_of_bounds");
    }

    #[test]
    fn test_mmap_rejects_len_smaller_than_header() {
        let _ = fs::remove_file("test_mmap_small_len");
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .open("test_mmap_small_len")
            .unwrap();
        file.set_len((LEN_OFFSET - 1) as u64).unwrap();

        let err = MemoryMap::new(&file, (LEN_OFFSET - 1) as u64).unwrap_err();
        assert_eq!(
            err,
            IOError(format!(
                "failed to create mmap with len {}: mmap length is smaller than header {}",
                LEN_OFFSET - 1,
                LEN_OFFSET
            ))
        );

        let _ = fs::remove_file("test_mmap_small_len");
    }

    #[test]
    fn test_mmap_rejects_invalid_stored_length() {
        let _ = fs::remove_file("test_mmap_invalid_len");
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .open("test_mmap_invalid_len")
            .unwrap();
        file.set_len((LEN_OFFSET + 1) as u64).unwrap();
        let mut mm = MemoryMap::new(&file, (LEN_OFFSET + 1) as u64).unwrap();
        mm.0[0..LEN_OFFSET].copy_from_slice(&2u64.to_be_bytes());

        let err = mm.write_offset().unwrap_err();
        assert_eq!(
            err,
            IOError("invalid mmap content length 2, max 1".to_string())
        );

        let _ = fs::remove_file("test_mmap_invalid_len");
    }
}
