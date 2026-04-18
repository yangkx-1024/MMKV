use crate::Error::IOError;
use crate::Result;
use std::fs;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Instant;

const LOG_TAG: &str = "MMKV:Config";

pub struct Config {
    pub(crate) page_size: u64,
    pub path: PathBuf,
    pub file: File,
}

impl Config {
    pub fn new(path: &Path, page_size: u64) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| IOError(format!("failed to open {}: {e}", path.display())))?;
        let mut file_len = file
            .metadata()
            .map_err(|e| {
                IOError(format!(
                    "failed to get metadata for {}: {e}",
                    path.display()
                ))
            })?
            .len();
        if file_len == 0 {
            file_len += page_size;
            file.set_len(file_len).map_err(|e| {
                IOError(format!(
                    "failed to initialize file size for {} to {}: {e}",
                    path.display(),
                    file_len
                ))
            })?;
        }
        file.sync_all()
            .map_err(|e| IOError(format!("failed to sync {}: {e}", path.display())))?;
        // Sweep orphaned .tmp.* files left by a previous crashed trim.
        if let (Some(dir), Some(stem)) = (path.parent(), path.file_name()) {
            let prefix = format!("{}.tmp.", stem.to_string_lossy());
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().starts_with(&prefix) {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
        Ok(Config {
            page_size,
            path: path.to_path_buf(),
            file,
        })
    }

    pub(crate) fn ensure_file_len(&mut self, min_len: u64) -> Result<u64> {
        let file_size = self.file_size()?;
        if min_len <= file_size {
            return Ok(file_size);
        }
        let expand_start = Instant::now();
        let expanded_size = self.ensure_size(file_size, min_len, usize::MAX as u64)?;
        info!(
            LOG_TAG,
            "start expand, file size: {}, required: {}, target: {}",
            file_size,
            min_len,
            expanded_size,
        );
        self.file.sync_all().map_err(|e| {
            IOError(format!(
                "failed to sync file before expand {}: {e}",
                self.path.display()
            ))
        })?;
        self.file.set_len(expanded_size).map_err(|e| {
            IOError(format!(
                "failed to expand file {} to {}: {e}",
                self.path.display(),
                expanded_size
            ))
        })?;
        info!(
            LOG_TAG,
            "expanded, file size: {}, cost {:?}",
            expanded_size,
            expand_start.elapsed()
        );
        Ok(expanded_size)
    }

    pub fn file_size(&self) -> Result<u64> {
        self.file
            .metadata()
            .map_err(|e| {
                IOError(format!(
                    "failed to get metadata for {}: {e}",
                    self.path.display()
                ))
            })
            .map(|metadata| metadata.len())
    }

    pub fn remove_file(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(IOError(format!(
                "failed to remove {}: {e}",
                self.path.display()
            ))),
        }
    }

    #[cfg(test)]
    pub fn try_clone(&self) -> Result<Self> {
        Ok(Config {
            page_size: self.page_size,
            path: self.path.clone(),
            file: self.file.try_clone().map_err(|e| {
                IOError(format!(
                    "failed to clone config file {}: {e}",
                    self.path.display()
                ))
            })?,
        })
    }

    fn ensure_size(&self, file_size: u64, min_len: u64, max_mappable_len: u64) -> Result<u64> {
        if min_len > max_mappable_len {
            return Err(IOError(format!(
                "failed to expand file {} to {}: exceeds platform usize",
                self.path.display(),
                min_len
            )));
        }

        let growth_target_len = self.growth_target_len(file_size, min_len, max_mappable_len);
        let expanded_size = self
            .aligned_len_with_limit(growth_target_len, max_mappable_len)
            .or_else(|| self.aligned_len_with_limit(min_len, max_mappable_len))
            .unwrap_or(min_len);
        Ok(expanded_size)
    }

    fn aligned_len_with_limit(&self, min_len: u64, max_mappable_len: u64) -> Option<u64> {
        let aligned = min_len.next_multiple_of(self.page_size);
        (aligned <= max_mappable_len).then_some(aligned)
    }

    fn growth_target_len(&self, file_size: u64, min_len: u64, max_mappable_len: u64) -> u64 {
        match file_size.checked_mul(2) {
            Some(doubled_len) if doubled_len <= max_mappable_len => doubled_len.max(min_len),
            _ => min_len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::fs;
    use std::path::Path;

    #[test]
    fn ensure_file_len_doubles_small_overflow_and_aligns_large_jumps() {
        let file_name = "test_config_ensure_file_len";
        let _ = fs::remove_file(file_name);
        let mut config = Config::new(Path::new(file_name), 64).unwrap();

        assert_eq!(config.file_size().unwrap(), 64);
        // Small overflow doubles from 64 to 128.
        assert_eq!(config.ensure_file_len(65).unwrap(), 128);
        assert_eq!(config.file_size().unwrap(), 128);
        // Large jump bypasses doubling and grows to the exact aligned requirement.
        assert_eq!(config.ensure_file_len(281).unwrap(), 320);
        assert_eq!(config.file_size().unwrap(), 320);
        assert_eq!(config.ensure_file_len(280).unwrap(), 320);
        assert_eq!(config.file_size().unwrap(), 320);
        // Small overflow after growth doubles from 320 to 640.
        assert_eq!(config.ensure_file_len(321).unwrap(), 640);
        assert_eq!(config.file_size().unwrap(), 640);

        config.remove_file().unwrap();
    }

    #[test]
    fn ensure_size_falls_back_before_exceeding_mappable_limit() {
        let file_name = "test_config_growth_plan_limit";
        let _ = fs::remove_file(file_name);
        let config = Config::new(Path::new(file_name), 64).unwrap();

        // Simulate a constrained 32-bit-like mmap limit on a 64-bit host.
        assert_eq!(config.ensure_size(200, 201, 255).unwrap(), 201);
        assert_eq!(config.ensure_size(128, 191, 255).unwrap(), 192);

        config.remove_file().unwrap();
    }
}
