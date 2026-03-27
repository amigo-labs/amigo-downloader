//! Download Coordinator — orchestrates downloads across all protocols.

use crate::config::Config;
use crate::protocol::DownloadRequest;
use crate::queue::QueueManager;
use crate::storage::Storage;

pub struct Coordinator {
    config: Config,
    queue: QueueManager,
    storage: Storage,
}

impl Coordinator {
    pub fn new(config: Config, storage: Storage) -> Self {
        Self {
            queue: QueueManager::new(config.max_concurrent_downloads),
            config,
            storage,
        }
    }

    pub async fn add_download(&self, _request: DownloadRequest) -> Result<String, crate::Error> {
        todo!("Add download to queue and start if slots available")
    }

    pub async fn pause(&self, _id: &str) -> Result<(), crate::Error> {
        todo!()
    }

    pub async fn resume(&self, _id: &str) -> Result<(), crate::Error> {
        todo!()
    }

    pub async fn cancel(&self, _id: &str) -> Result<(), crate::Error> {
        todo!()
    }
}
