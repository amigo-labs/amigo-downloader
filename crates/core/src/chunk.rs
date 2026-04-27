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
    /// Split a file of `total_size` into at most `num_chunks` chunks.
    ///
    /// When `total_size < num_chunks`, the previous implementation computed
    /// `chunk_size = total_size / num_chunks = 0`, then derived
    /// `end = start + 0 - 1` which wrapped to `u64::MAX`. The resulting
    /// HTTP `Range` headers requested overlapping or absurd byte ranges
    /// and the reassembly stage either looped or wrote garbage.
    ///
    /// Behaviour now:
    ///  * `num_chunks == 0` or `total_size == 0` → empty plan.
    ///  * The actual chunk count is capped at `total_size` so every chunk
    ///    holds at least one byte.
    ///  * `chunk_size = ceil(total_size / n)`, computed via `div_ceil` to
    ///    avoid the underflow that bit the original code.
    ///  * Each chunk's `end_byte` is clamped to `total_size - 1`.
    ///  * For any `(total_size, num_chunks)`, the produced ranges cover
    ///    `[0, total_size)` exactly once and remain disjoint.
    pub fn split(total_size: u64, num_chunks: u32) -> Self {
        if num_chunks == 0 || total_size == 0 {
            return Self { chunks: vec![] };
        }
        // Cap requested chunks at total_size so chunk_size is always ≥ 1.
        // After ceil-dividing, the *actual* number of populated chunks may
        // still be smaller (e.g. total=100, requested=16 → chunk_size=7,
        // only 15 chunks are needed — the 16th would start past EOF).
        let requested = (num_chunks as u64).min(total_size);
        let chunk_size = total_size.div_ceil(requested);
        let mut chunks = Vec::with_capacity(requested as usize);

        let mut start = 0u64;
        let mut index: u32 = 0;
        while start < total_size {
            let end = start
                .saturating_add(chunk_size)
                .saturating_sub(1)
                .min(total_size - 1);
            chunks.push(Chunk {
                index,
                start_byte: start,
                end_byte: end,
                bytes_downloaded: 0,
                status: ChunkStatus::Pending,
            });
            // The next start cannot overflow because end < total_size and
            // total_size <= u64::MAX.
            start = end + 1;
            index += 1;
        }

        Self { chunks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_covers_exactly(plan: &ChunkPlan, total_size: u64) {
        if total_size == 0 {
            assert!(plan.chunks.is_empty());
            return;
        }
        let mut expected = 0u64;
        for c in &plan.chunks {
            assert!(c.start_byte <= c.end_byte, "empty chunk: {c:?}");
            assert_eq!(c.start_byte, expected, "gap or overlap at {c:?}");
            expected = c
                .end_byte
                .checked_add(1)
                .expect("end_byte must not be u64::MAX");
        }
        assert_eq!(expected, total_size, "plan does not cover the whole file");
    }

    #[test]
    fn split_handles_total_smaller_than_num_chunks() {
        // Regression: previously chunk_size = 10/100 = 0 and the non-last
        // chunk's end wrapped to u64::MAX.
        let plan = ChunkPlan::split(10, 100);
        assert_eq!(plan.chunks.len(), 10, "chunks capped at total_size");
        assert_covers_exactly(&plan, 10);
        for c in &plan.chunks {
            assert_eq!(c.end_byte - c.start_byte, 0, "each chunk is 1 byte");
        }
    }

    #[test]
    fn split_handles_perfect_division() {
        let plan = ChunkPlan::split(1024, 4);
        assert_eq!(plan.chunks.len(), 4);
        assert_covers_exactly(&plan, 1024);
        // 4 × 256 byte chunks
        for c in &plan.chunks {
            assert_eq!(c.end_byte - c.start_byte + 1, 256);
        }
    }

    #[test]
    fn split_handles_remainder() {
        // 1000 / 7 → 143 with remainder; ceil(1000/7) = 143, last chunk is shorter.
        let plan = ChunkPlan::split(1000, 7);
        assert_eq!(plan.chunks.len(), 7);
        assert_covers_exactly(&plan, 1000);
    }

    #[test]
    fn split_handles_one_chunk() {
        let plan = ChunkPlan::split(42, 1);
        assert_eq!(plan.chunks.len(), 1);
        assert_eq!(plan.chunks[0].start_byte, 0);
        assert_eq!(plan.chunks[0].end_byte, 41);
    }

    #[test]
    fn split_handles_zero_inputs() {
        assert!(ChunkPlan::split(0, 4).chunks.is_empty());
        assert!(ChunkPlan::split(100, 0).chunks.is_empty());
    }

    #[test]
    fn split_property_random_inputs() {
        // Ad-hoc property check (no proptest dependency): cover a spread
        // of (total, n) combos and verify the invariant for each.
        for total in [1u64, 2, 7, 100, 1_000, 1_234_567, u64::MAX / 2] {
            for n in [1u32, 2, 3, 8, 16, 64, 1024] {
                let plan = ChunkPlan::split(total, n);
                assert_covers_exactly(&plan, total);
                assert!(plan.chunks.len() <= n as usize);
                assert!(plan.chunks.len() as u64 <= total);
            }
        }
    }
}
