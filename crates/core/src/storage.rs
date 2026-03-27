//! SQLite + filesystem storage abstraction.

use std::path::PathBuf;

pub struct Storage {
    pub db_path: PathBuf,
    pub download_dir: PathBuf,
    pub temp_dir: PathBuf,
}

impl Storage {
    pub fn new(db_path: PathBuf, download_dir: PathBuf, temp_dir: PathBuf) -> Self {
        Self {
            db_path,
            download_dir,
            temp_dir,
        }
    }

    pub async fn init_db(&self) -> Result<(), crate::Error> {
        todo!("Create SQLite tables if not exists")
    }
}
