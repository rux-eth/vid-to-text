use std::path::{Path, PathBuf};

use vtt_core::ClientConfig;

use crate::api;

/// Compute the output JSON path for a given input mp4 file.
pub fn compute_output_path(input_path: &Path, output_dir: &Option<String>) -> PathBuf {
    let stem = input_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let json_name = format!("{stem}.json");

    match output_dir {
        Some(dir) => PathBuf::from(dir).join(&json_name),
        None => input_path.with_file_name(&json_name),
    }
}

/// Find all mp4 files in a directory, sorted alphabetically.
pub fn find_mp4_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read directory {}: {e}", dir.display()))?;

    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("mp4"))
        .collect();

    files.sort();
    Ok(files)
}

/// Process a single mp4 file: upload → poll → download result → write JSON.
pub async fn process_single_file(
    client: &reqwest::Client,
    config: &ClientConfig,
    input_path: &Path,
    force: bool,
) -> Result<(), String> {
    let server_url = config.server_url();
    let filename = input_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Upload
    eprintln!("Uploading {filename}...");
    let job = api::upload_file(client, &server_url, input_path, force).await?;
    eprintln!("Job {} created for {filename}", job.id);

    // Poll until done
    api::poll_until_done(client, &server_url, &job.id, &filename, config).await?;

    // Download result
    let timeline = api::download_result(client, &server_url, &job.id).await?;

    // Write output
    let output_path = compute_output_path(input_path, &config.output.dir);
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("failed to create output dir: {e}"))?;
    }

    let json = serde_json::to_string_pretty(&timeline)
        .map_err(|e| format!("failed to serialize timeline: {e}"))?;
    tokio::fs::write(&output_path, &json)
        .await
        .map_err(|e| format!("failed to write {}: {e}", output_path.display()))?;

    eprintln!(
        "Wrote {} ({} segments, {:.1}s duration)",
        output_path.display(),
        timeline.segments.len(),
        timeline.duration_seconds
    );

    Ok(())
}

/// Process a YouTube URL: submit → poll → download result → write JSON.
pub async fn process_url(
    client: &reqwest::Client,
    config: &ClientConfig,
    url: &str,
    force: bool,
    max_resolution: Option<String>,
    max_fps: Option<u32>,
) -> Result<(), String> {
    let server_url = config.server_url();

    eprintln!("Submitting URL: {url}");
    let job = api::submit_url_job(client, &server_url, url, force, max_resolution, max_fps).await?;
    eprintln!("Job {} created", job.id);

    // Poll until done
    api::poll_until_done(client, &server_url, &job.id, url, config).await?;

    // Download result
    let timeline = api::download_result(client, &server_url, &job.id).await?;

    // Derive output filename from source
    let source_stem = std::path::Path::new(&timeline.source)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let json_name = format!("{source_stem}.json");

    let output_path = match &config.output.dir {
        Some(dir) => std::path::PathBuf::from(dir).join(&json_name),
        None => std::path::PathBuf::from(&json_name),
    };

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("failed to create output dir: {e}"))?;
        }
    }

    let json = serde_json::to_string_pretty(&timeline)
        .map_err(|e| format!("failed to serialize timeline: {e}"))?;
    tokio::fs::write(&output_path, &json)
        .await
        .map_err(|e| format!("failed to write {}: {e}", output_path.display()))?;

    eprintln!(
        "Wrote {} ({} segments, {:.1}s duration)",
        output_path.display(),
        timeline.segments.len(),
        timeline.duration_seconds
    );

    Ok(())
}

/// Process all mp4 files in a directory sequentially.
pub async fn process_directory(
    client: &reqwest::Client,
    config: &ClientConfig,
    dir: &Path,
    force: bool,
) -> Result<(), String> {
    let files = find_mp4_files(dir)?;

    if files.is_empty() {
        return Err(format!("no mp4 files found in {}", dir.display()));
    }

    eprintln!("Found {} mp4 file(s) in {}", files.len(), dir.display());

    let mut success = 0;
    let total = files.len();

    for file in &files {
        match process_single_file(client, config, file, force).await {
            Ok(()) => success += 1,
            Err(e) => eprintln!("Error processing {}: {e}", file.display()),
        }
    }

    eprintln!("\nProcessed {success}/{total} files successfully.");
    if success < total {
        return Err(format!("{} file(s) failed", total - success));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_output_path_alongside_input() {
        let path = compute_output_path(Path::new("/videos/test.mp4"), &None);
        assert_eq!(path, PathBuf::from("/videos/test.json"));
    }

    #[test]
    fn test_compute_output_path_with_output_dir() {
        let path =
            compute_output_path(Path::new("/videos/test.mp4"), &Some("/output".to_string()));
        assert_eq!(path, PathBuf::from("/output/test.json"));
    }

    #[test]
    fn test_find_mp4_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let files = find_mp4_files(dir.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_find_mp4_files_mixed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.mp4"), "").unwrap();
        std::fs::write(dir.path().join("b.txt"), "").unwrap();
        std::fs::write(dir.path().join("c.mp4"), "").unwrap();
        let files = find_mp4_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].file_name().unwrap().to_str().unwrap() == "a.mp4");
        assert!(files[1].file_name().unwrap().to_str().unwrap() == "c.mp4");
    }

    #[test]
    fn test_find_mp4_files_sorted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("z.mp4"), "").unwrap();
        std::fs::write(dir.path().join("a.mp4"), "").unwrap();
        std::fs::write(dir.path().join("m.mp4"), "").unwrap();
        let files = find_mp4_files(dir.path()).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files[0] < files[1]);
        assert!(files[1] < files[2]);
    }
}
