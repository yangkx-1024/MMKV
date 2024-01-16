use std::fs::File;
use std::ops::{Deref, DerefMut, Range};
use std::os::fd::{AsRawFd, RawFd};
use std::ptr::NonNull;
use std::{io, ptr, slice};

pub const LEN_OFFSET: usize = 8;

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
                    ptr: NonNull::new(ptr).unwrap(),
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
pub struct MemoryMap(RawMmap);

impl MemoryMap {
    pub fn new(file: &File, len: usize) -> Self {
        let raw_mmap = RawMmap::new(file.as_raw_fd(), len).unwrap();
        MemoryMap(raw_mmap)
    }

    pub fn append(&mut self, value: Vec<u8>) -> io::Result<()> {
        let data_len = value.len();
        let start = self.len();
        let content_len = start - LEN_OFFSET;
        let end = data_len + start;
        let new_content_len = data_len + content_len;
        self.0[0..LEN_OFFSET].copy_from_slice(new_content_len.to_be_bytes().as_slice());
        self.0[start..end].copy_from_slice(value.as_slice());
        self.0.flush(end)
    }

    pub fn reset(&mut self) -> io::Result<()> {
        let len = 0usize;
        self.0[0..LEN_OFFSET].copy_from_slice(len.to_be_bytes().as_slice());
        self.0.flush(LEN_OFFSET)
    }

    pub fn len(&self) -> usize {
        usize::from_be_bytes(self.0[0..LEN_OFFSET].try_into().unwrap()) + LEN_OFFSET
    }

    pub fn read(&self, range: Range<usize>) -> &[u8] {
        self.0[range].as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;

    use super::MemoryMap;

    #[test]
    fn test_mmap() {
        let _ = fs::remove_file("test_mmap");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open("test_mmap")
            .unwrap();
        file.set_len(1024).unwrap();
        let mut mm = MemoryMap::new(&file, 1024);
        assert_eq!(mm.len(), 8);
        mm.append(vec![1, 2, 3]).unwrap();
        mm.append(vec![4]).unwrap();
        assert_eq!(mm.len(), 12);

        let read = mm.read(8..10);
        assert_eq!(read.len(), 2);
        assert_eq!(read[0], 1);
        assert_eq!(read[1], 2);
        let read = mm.read(mm.len() - 1..mm.len());
        assert_eq!(read[0], 4);

        mm.reset().unwrap();
        mm.append(vec![5, 4, 3, 2, 1]).unwrap();
        assert_eq!(mm.len(), 13);
        let read = mm.read(8..9);
        assert_eq!(read[0], 5);

        let read = mm.read(9..10);
        assert_eq!(read[0], 4);
        let _ = fs::remove_file("test_mmap");
    }
}
