//! Chunk splitting, reassembly, and verification for parallel downloads.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPlan {
    pub chunks: Vec<Chunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub index: u32,
    pub start_byte: u64,
    pub end_byte: u64,
    pub bytes_downloaded: u64,
    pub status: ChunkStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
}

impl ChunkPlan {
    /// Split a file of `total_size` into `num_chunks` chunks.
    pub fn split(total_size: u64, num_chunks: u32) -> Self {
        let chunk_size = total_size / num_chunks as u64;
        let mut chunks = Vec::with_capacity(num_chunks as usize);

        for i in 0..num_chunks {
            let start = i as u64 * chunk_size;
            let end = if i == num_chunks - 1 {
                total_size - 1
            } else {
                start + chunk_size - 1
            };
            chunks.push(Chunk {
                index: i,
                start_byte: start,
                end_byte: end,
                bytes_downloaded: 0,
                status: ChunkStatus::Pending,
            });
        }

        Self { chunks }
    }
}
