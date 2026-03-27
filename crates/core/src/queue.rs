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

impl QueueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Extracting => "extracting",
            Self::Verifying => "verifying",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(Self::Queued),
            "downloading" => Some(Self::Downloading),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "extracting" => Some(Self::Extracting),
            "verifying" => Some(Self::Verifying),
            _ => None,
        }
    }
}

impl std::fmt::Display for QueueStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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
