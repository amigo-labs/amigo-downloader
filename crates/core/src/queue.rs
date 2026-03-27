//! Priority queue and download scheduling.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub id: String,
    pub priority: i32,
    pub status: QueueStatus,
    pub package_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Extracting,
    Verifying,
}

pub struct QueueManager {
    max_concurrent: u32,
}

impl QueueManager {
    pub fn new(max_concurrent: u32) -> Self {
        Self { max_concurrent }
    }

    pub fn max_concurrent(&self) -> u32 {
        self.max_concurrent
    }
}
