use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Segment, VttError};

const CHECKPOINT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct ChunkCheckpoint {
    version: u32,
    chunk_index: u32,
    segments: Vec<Segment>,
}

/// Returns the checkpoint directory path for a job.
pub fn checkpoint_dir(temp_dir: &str, job_id: &str) -> PathBuf {
    Path::new(temp_dir).join(job_id).join("checkpoints")
}

/// Save a checkpoint for a completed chunk using atomic write.
/// Writes to a .tmp file first, then renames to the final path.
pub async fn save_checkpoint(
    temp_dir: &str,
    job_id: &str,
    chunk_index: u32,
    segments: &[Segment],
) -> Result<(), VttError> {
    let dir = checkpoint_dir(temp_dir, job_id);
    tokio::fs::create_dir_all(&dir).await?;

    let checkpoint = ChunkCheckpoint {
        version: CHECKPOINT_VERSION,
        chunk_index,
        segments: segments.to_vec(),
    };

    let json = serde_json::to_string_pretty(&checkpoint)?;
    let final_path = dir.join(format!("chunk_{:03}.json", chunk_index));
    let tmp_path = dir.join(format!("chunk_{:03}.json.tmp", chunk_index));

    tokio::fs::write(&tmp_path, &json).await?;
    tokio::fs::rename(&tmp_path, &final_path).await?;

    Ok(())
}

/// Load all checkpoints for a job. Returns a map from chunk index to segments.
/// Ignores .tmp files (incomplete writes from crashes).
pub async fn load_checkpoints(
    temp_dir: &str,
    job_id: &str,
) -> Result<HashMap<u32, Vec<Segment>>, VttError> {
    let dir = checkpoint_dir(temp_dir, job_id);

    if !dir.exists() {
        return Ok(HashMap::new());
    }

    let mut checkpoints = HashMap::new();
    let mut entries = tokio::fs::read_dir(&dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Skip .tmp files and non-json files
        if !name.ends_with(".json") || name.ends_with(".tmp") {
            continue;
        }

        let contents = tokio::fs::read_to_string(&path).await?;
        let checkpoint: ChunkCheckpoint = serde_json::from_str(&contents).map_err(|e| {
            VttError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("corrupt checkpoint {}: {e}", path.display()),
            ))
        })?;

        if checkpoint.version != CHECKPOINT_VERSION {
            eprintln!(
                "Warning: skipping checkpoint {} (version {} != expected {})",
                path.display(),
                checkpoint.version,
                CHECKPOINT_VERSION
            );
            continue;
        }

        checkpoints.insert(checkpoint.chunk_index, checkpoint.segments);
    }

    Ok(checkpoints)
}

/// Remove all checkpoints for a job.
pub async fn clear_checkpoints(temp_dir: &str, job_id: &str) -> Result<(), VttError> {
    let dir = checkpoint_dir(temp_dir, job_id);
    if dir.exists() {
        tokio::fs::remove_dir_all(&dir).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SegmentType;

    fn make_segments() -> Vec<Segment> {
        vec![
            Segment {
                segment_type: SegmentType::Speech,
                start: "00:00:01.000".to_string(),
                end: "00:00:03.000".to_string(),
                content: "Hello world".to_string(),
            },
            Segment {
                segment_type: SegmentType::Visual,
                start: "00:00:00.000".to_string(),
                end: "00:00:05.000".to_string(),
                content: "A person waves".to_string(),
            },
        ]
    }

    #[tokio::test]
    async fn test_save_and_load_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().display().to_string();
        let segments = make_segments();

        save_checkpoint(&temp_dir, "test-job", 0, &segments)
            .await
            .unwrap();

        let loaded = load_checkpoints(&temp_dir, "test-job").await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[&0], segments);
    }

    #[tokio::test]
    async fn test_save_multiple_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().display().to_string();
        let segments = make_segments();

        save_checkpoint(&temp_dir, "test-job", 0, &segments)
            .await
            .unwrap();
        save_checkpoint(&temp_dir, "test-job", 1, &segments)
            .await
            .unwrap();
        save_checkpoint(&temp_dir, "test-job", 2, &segments)
            .await
            .unwrap();

        let loaded = load_checkpoints(&temp_dir, "test-job").await.unwrap();
        assert_eq!(loaded.len(), 3);
        assert!(loaded.contains_key(&0));
        assert!(loaded.contains_key(&1));
        assert!(loaded.contains_key(&2));
    }

    #[tokio::test]
    async fn test_load_checkpoints_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().display().to_string();

        let loaded = load_checkpoints(&temp_dir, "nonexistent").await.unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_load_checkpoints_ignores_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().display().to_string();
        let segments = make_segments();

        // Save a real checkpoint
        save_checkpoint(&temp_dir, "test-job", 0, &segments)
            .await
            .unwrap();

        // Write a .tmp file (simulating a crash mid-write)
        let cp_dir = checkpoint_dir(&temp_dir, "test-job");
        tokio::fs::write(cp_dir.join("chunk_001.json.tmp"), "incomplete")
            .await
            .unwrap();

        let loaded = load_checkpoints(&temp_dir, "test-job").await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key(&0));
        assert!(!loaded.contains_key(&1));
    }

    #[tokio::test]
    async fn test_clear_checkpoints() {
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().display().to_string();
        let segments = make_segments();

        save_checkpoint(&temp_dir, "test-job", 0, &segments)
            .await
            .unwrap();

        let cp_dir = checkpoint_dir(&temp_dir, "test-job");
        assert!(cp_dir.exists());

        clear_checkpoints(&temp_dir, "test-job").await.unwrap();
        assert!(!cp_dir.exists());
    }

    #[tokio::test]
    async fn test_clear_checkpoints_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().display().to_string();

        // Should not error on nonexistent dir
        clear_checkpoints(&temp_dir, "nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn test_atomic_write_no_final_without_rename() {
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().display().to_string();
        let segments = make_segments();

        save_checkpoint(&temp_dir, "test-job", 0, &segments)
            .await
            .unwrap();

        let cp_dir = checkpoint_dir(&temp_dir, "test-job");
        // Final file should exist, .tmp should not
        assert!(cp_dir.join("chunk_000.json").exists());
        assert!(!cp_dir.join("chunk_000.json.tmp").exists());
    }
}
