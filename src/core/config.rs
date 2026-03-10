use crate::Error::IOError;
use crate::Result;
use std::fs;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Instant;

const LOG_TAG: &str = "MMKV:Config";

pub struct Config {
    page_size: u64,
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
        Ok(Config {
            page_size,
            path: path.to_path_buf(),
            file,
        })
    }

    pub fn expand(&mut self) -> Result<()> {
        let expand_start = Instant::now();
        let file_size = self.file_size()?;
        info!(LOG_TAG, "start expand, file size: {}", file_size);
        // expand the file size with page_size
        self.file.sync_all().map_err(|e| {
            IOError(format!(
                "failed to sync file before expand {}: {e}",
                self.path.display()
            ))
        })?;
        let expanded_size = file_size + self.page_size;
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
            self.file_size()?,
            expand_start.elapsed()
        );
        Ok(())
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
}
