use crate::Error::IOError;
use crate::Result;
use std::fs::File;
use std::mem::size_of;
use std::ops::Range;
use std::os::fd::{AsRawFd, RawFd};
use std::ptr::NonNull;
use std::sync::Arc;
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

impl std::ops::Deref for RawMmap {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr() as *const u8, self.len) }
    }
}

unsafe impl Send for RawMmap {}
unsafe impl Sync for RawMmap {}

/// A shared read-only view into a memory-mapped region.
/// Multiple `MmapHandle` instances can coexist with the [MemoryMap] writer.
#[derive(Clone, Debug)]
pub struct MmapHandle {
    raw: Arc<RawMmap>,
}

impl MmapHandle {
    pub fn read(&self, range: Range<usize>) -> &[u8] {
        &self.raw[range]
    }

    /// The length of the mapping this handle points at. Test-only: production readers
    /// never need it, but tests assert that an expand publishes a bigger mapping.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.raw.len
    }
}

#[derive(Debug)]
pub struct MemoryMap {
    raw: Arc<RawMmap>,
}

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
        if let Err(e) = self.raw.flush(flush_len) {
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
        Ok(MemoryMap {
            raw: Arc::new(raw_mmap),
        })
    }

    pub fn to_handle(&self) -> MmapHandle {
        MmapHandle {
            raw: Arc::clone(&self.raw),
        }
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
        // SAFETY: `&mut self` ensures no aliased mutable access. Readers hold MmapHandle
        // which provides only &[u8]. The write target [start, end) was validated to be within
        // bounds and starts at write_offset — readers never access bytes past write_offset.
        unsafe {
            let dst = (self.raw.ptr.as_ptr() as *mut u8).add(start);
            ptr::copy_nonoverlapping(value.as_ptr(), dst, data_len);
        }
        Ok(())
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
        self.raw.len
    }

    pub fn base_ptr(&self) -> usize {
        self.raw.ptr.as_ptr() as usize
    }

    pub fn flush(&self) -> Result<()> {
        let len = self.write_offset()?;
        self.raw
            .flush(len)
            .map_err(|e| IOError(format!("failed to flush mmap: {e}")))
    }

    /// Move the write offset back to `offset`, discarding everything after it.
    /// Used at open to drop an undecodable tail so that later appends overwrite it.
    pub fn truncate_content(&mut self, offset: usize) -> Result<()> {
        if offset < LEN_OFFSET || offset > self.len() {
            return Err(IOError(format!(
                "truncate offset {offset} out of bounds, header {LEN_OFFSET}, mmap len {}",
                self.len()
            )));
        }
        let content_len = u64::try_from(offset - LEN_OFFSET)
            .map_err(|_| IOError("truncate overflowed stored content length".to_string()))?;
        self.write_content_len(content_len);
        Ok(())
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
        Ok(&self.raw[range])
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
            self.raw[0..LEN_OFFSET]
                .try_into()
                .expect("mmap header slice length must match u64"),
        )
    }

    fn write_content_len(&mut self, content_len: u64) {
        let bytes = content_len.to_be_bytes();
        // SAFETY: `&mut self` ensures exclusive mutation; readers only access bytes via &[u8] through MmapHandle.
        unsafe {
            let dst = self.raw.ptr.as_ptr() as *mut u8;
            ptr::copy_nonoverlapping(bytes.as_ptr(), dst, LEN_OFFSET);
        }
    }
}

/// Unit tests for the mapping itself: the 8-byte content-length header, append and read
/// bounds, truncation, flushing and the shared read-only `MmapHandle`.
#[cfg(test)]
mod tests {
    use std::fs::File;

    use crate::Error::IOError;

    use super::{LEN_OFFSET, MemoryMap};

    /// An anonymous temp file of `len` bytes; it disappears when the handle drops.
    fn temp_file(len: u64) -> File {
        let file = tempfile::tempfile().unwrap();
        file.set_len(len).unwrap();
        file
    }

    fn write_raw(mm: &mut MemoryMap, offset: usize, data: &[u8]) {
        unsafe {
            let dst = (mm.raw.ptr.as_ptr() as *mut u8).add(offset);
            std::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        }
    }

    #[test]
    fn append_and_read_track_the_content_length_header() {
        let file = temp_file(1024);
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

        mm.write_content_len(0);
        mm.append(&[5, 4, 3, 2, 1]).unwrap();
        assert_eq!(mm.write_offset().unwrap(), 13);
        let read = mm.read(8..9).unwrap();
        assert_eq!(read[0], 5);

        let read = mm.read(9..10).unwrap();
        assert_eq!(read[0], 4);
    }

    #[test]
    fn truncate_content_moves_the_write_offset_and_rejects_bad_offsets() {
        let file = temp_file(64);
        let mut mm = MemoryMap::new(&file, 64).unwrap();
        mm.append(&[1, 2, 3, 4]).unwrap();
        assert_eq!(mm.write_offset().unwrap(), LEN_OFFSET + 4);

        mm.truncate_content(LEN_OFFSET + 2).unwrap();
        assert_eq!(mm.write_offset().unwrap(), LEN_OFFSET + 2);
        mm.append(&[9]).unwrap();
        assert_eq!(mm.read(LEN_OFFSET..LEN_OFFSET + 3).unwrap(), &[1, 2, 9]);

        assert!(mm.truncate_content(LEN_OFFSET - 1).is_err());
        assert!(mm.truncate_content(65).is_err());
        assert_eq!(mm.write_offset().unwrap(), LEN_OFFSET + 3);
    }

    #[test]
    fn append_past_the_mapping_is_rejected() {
        let file = temp_file((LEN_OFFSET + 1) as u64);
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
    }

    #[test]
    fn read_past_the_mapping_is_rejected() {
        let file = temp_file((LEN_OFFSET + 1) as u64);
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
    }

    #[test]
    fn new_rejects_a_length_smaller_than_the_header() {
        let file = temp_file((LEN_OFFSET - 1) as u64);

        let err = MemoryMap::new(&file, (LEN_OFFSET - 1) as u64).unwrap_err();
        assert_eq!(
            err,
            IOError(format!(
                "failed to create mmap with len {}: mmap length is smaller than header {}",
                LEN_OFFSET - 1,
                LEN_OFFSET
            ))
        );
    }

    #[test]
    fn a_stored_length_past_the_mapping_is_rejected() {
        let file = temp_file((LEN_OFFSET + 1) as u64);
        let mut mm = MemoryMap::new(&file, (LEN_OFFSET + 1) as u64).unwrap();
        write_raw(&mut mm, 0, &2u64.to_be_bytes());

        let err = mm.write_offset().unwrap_err();
        assert_eq!(
            err,
            IOError("invalid mmap content length 2, max 1".to_string())
        );
    }

    #[test]
    fn a_handle_taken_before_an_append_observes_the_new_bytes() {
        let file = temp_file(64);
        let mut mm = MemoryMap::new(&file, 64).unwrap();
        let handle = mm.to_handle();

        mm.append(&[1, 2, 3]).unwrap();

        assert_eq!(
            handle.read(LEN_OFFSET..LEN_OFFSET + 3),
            &[1, 2, 3],
            "MmapHandle shares the mapping, it is not a snapshot"
        );
    }

    #[test]
    fn flush_after_an_append_succeeds() {
        let file = temp_file(64);
        let mut mm = MemoryMap::new(&file, 64).unwrap();
        mm.append(&[1, 2, 3, 4]).unwrap();

        assert_eq!(mm.flush(), Ok(()));
    }

    #[test]
    fn content_start_offset_and_len_describe_the_mapping() {
        let file = temp_file(128);
        let mm = MemoryMap::new(&file, 128).unwrap();

        assert_eq!(mm.content_start_offset(), LEN_OFFSET);
        assert_eq!(mm.len(), 128);
        assert_eq!(mm.to_handle().len(), 128);
        assert_eq!(mm.write_offset().unwrap(), mm.content_start_offset());
    }
}
