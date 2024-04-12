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
    pub fn new(path: &Path, page_size: u64) -> Self {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .unwrap();
        let mut file_len = file.metadata().unwrap().len();
        if file_len == 0 {
            file_len += page_size;
            file.set_len(file_len).unwrap();
        }
        file.sync_all().unwrap();
        Config {
            page_size,
            path: path.to_path_buf(),
            file,
        }
    }

    pub fn expand(&mut self) {
        let expand_start = Instant::now();
        let file_size = self.file_size();
        info!(LOG_TAG, "start expand, file size: {}", file_size);
        // expand the file size with page_size
        self.file.sync_all().unwrap();
        self.file.set_len(file_size + self.page_size).unwrap();
        info!(
            LOG_TAG,
            "expanded, file size: {}, cost {:?}",
            self.file_size(),
            expand_start.elapsed()
        );
    }

    pub fn file_size(&self) -> u64 {
        self.file.metadata().unwrap().len()
    }

    pub fn remove_file(&self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl Clone for Config {
    fn clone(&self) -> Self {
        Config {
            page_size: self.page_size,
            path: self.path.clone(),
            file: self.file.try_clone().unwrap(),
        }
    }
}
