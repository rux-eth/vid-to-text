use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vtt_core::Timeline;

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheMeta {
    pub hash: String,
    pub url: Option<String>,
    pub title: String,
    pub source: String,
    pub duration_seconds: f64,
    pub segment_count: usize,
    pub processed_at: String,
}

/// Returns the cache directory, creating it if needed.
pub fn cache_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("could not determine home directory")?;
    let dir = home.join(".vid-to-text").join("cache");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create cache dir: {e}"))?;
    Ok(dir)
}

/// SHA256 hash of source identifier, truncated to 8 hex chars.
pub fn hash_source(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)[..8].to_string()
}

/// Check if a source has been cached. Returns path to timeline.json if found.
pub fn cache_lookup(source: &str) -> Option<PathBuf> {
    let dir = cache_dir().ok()?;
    let hash = hash_source(source);
    let timeline_path = dir.join(&hash).join("timeline.json");
    if timeline_path.exists() {
        Some(timeline_path)
    } else {
        None
    }
}

/// Load a cached Timeline by source identifier.
pub fn cache_load(source: &str) -> Result<Timeline, String> {
    let path = cache_lookup(source)
        .ok_or_else(|| format!("no cache found for: {source}"))?;
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read cached timeline: {e}"))?;
    serde_json::from_str(&json)
        .map_err(|e| format!("failed to parse cached timeline: {e}"))
}

/// Store a Timeline in the cache with metadata.
pub fn cache_store(
    source: &str,
    url: Option<&str>,
    timeline: &Timeline,
) -> Result<PathBuf, String> {
    let dir = cache_dir()?;
    let hash = hash_source(source);
    let entry_dir = dir.join(&hash);
    std::fs::create_dir_all(&entry_dir)
        .map_err(|e| format!("failed to create cache entry dir: {e}"))?;

    // Write timeline
    let timeline_path = entry_dir.join("timeline.json");
    let json = serde_json::to_string_pretty(timeline)
        .map_err(|e| format!("failed to serialize timeline: {e}"))?;
    std::fs::write(&timeline_path, &json)
        .map_err(|e| format!("failed to write cached timeline: {e}"))?;

    // Write metadata
    let meta = CacheMeta {
        hash: hash.clone(),
        url: url.map(|u| u.to_string()),
        title: timeline
            .source
            .trim_end_matches(".mp4")
            .to_string(),
        source: timeline.source.clone(),
        duration_seconds: timeline.duration_seconds,
        segment_count: timeline.segments.len(),
        processed_at: chrono::Utc::now().to_rfc3339(),
    };
    let meta_json = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("failed to serialize meta: {e}"))?;
    std::fs::write(entry_dir.join("meta.json"), &meta_json)
        .map_err(|e| format!("failed to write cache meta: {e}"))?;

    eprintln!("Cached as {hash} in {}", entry_dir.display());
    Ok(timeline_path)
}

/// List all cached videos.
pub fn cache_list() -> Result<Vec<CacheMeta>, String> {
    let dir = cache_dir()?;
    let mut entries = Vec::new();

    let read_dir = std::fs::read_dir(&dir)
        .map_err(|e| format!("failed to read cache dir: {e}"))?;

    for entry in read_dir.flatten() {
        let meta_path = entry.path().join("meta.json");
        if meta_path.exists() {
            if let Ok(json) = std::fs::read_to_string(&meta_path) {
                if let Ok(meta) = serde_json::from_str::<CacheMeta>(&json) {
                    entries.push(meta);
                }
            }
        }
    }

    entries.sort_by(|a, b| b.processed_at.cmp(&a.processed_at));
    Ok(entries)
}

/// Resolve a hash or file path to a timeline.json path.
/// If input looks like a hash (8 hex chars, no extension), look up in cache.
/// Otherwise treat as a file path.
pub fn resolve_input(input: &str) -> Result<PathBuf, String> {
    // Check if it's a hash (8 hex chars)
    if input.len() == 8 && input.chars().all(|c| c.is_ascii_hexdigit()) {
        let dir = cache_dir()?;
        let timeline_path = dir.join(input).join("timeline.json");
        if timeline_path.exists() {
            return Ok(timeline_path);
        }
        return Err(format!("no cached video found with hash: {input}"));
    }

    // Otherwise treat as file path
    let path = PathBuf::from(input);
    if path.exists() {
        Ok(path)
    } else {
        Err(format!("file not found: {input}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_source_consistent() {
        let h1 = hash_source("https://youtube.com/watch?v=abc");
        let h2 = hash_source("https://youtube.com/watch?v=abc");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 8);
    }

    #[test]
    fn test_hash_source_different() {
        let h1 = hash_source("https://youtube.com/watch?v=abc");
        let h2 = hash_source("https://youtube.com/watch?v=def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_resolve_input_hash_not_found() {
        let result = resolve_input("deadbeef");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_input_file_not_found() {
        let result = resolve_input("/nonexistent/file.json");
        assert!(result.is_err());
    }
}
