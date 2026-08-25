use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::{Chunk, FfmpegConfig, ServerConfig, VisionConfig, VttError};

/// Result of ffmpeg/ffprobe availability check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfmpegInfo {
    pub ffmpeg_version: String,
    pub ffprobe_version: String,
}

/// Artifacts produced for a single chunk.
#[derive(Debug, Clone)]
pub struct ChunkArtifacts {
    pub chunk: Chunk,
    pub audio_path: PathBuf,
    pub frame_paths: Vec<PathBuf>,
}

/// Describes all prepared artifacts for a job.
#[derive(Debug)]
pub struct ChunkManifest {
    pub job_dir: PathBuf,
    pub duration_seconds: f64,
    pub chunks: Vec<ChunkArtifacts>,
}

/// Compute chunk boundaries given total duration and chunk size.
/// Pure function, no I/O.
pub fn compute_chunks(duration_seconds: f64, chunk_duration_secs: u32) -> Vec<Chunk> {
    if duration_seconds <= 0.0 || chunk_duration_secs == 0 {
        return Vec::new();
    }

    let chunk_dur = chunk_duration_secs as f64;
    let num_full = (duration_seconds / chunk_dur).floor() as u32;
    let remainder = duration_seconds - (num_full as f64 * chunk_dur);

    let mut chunks = Vec::new();
    for i in 0..num_full {
        chunks.push(Chunk {
            index: i,
            start_seconds: i as f64 * chunk_dur,
            end_seconds: (i + 1) as f64 * chunk_dur,
        });
    }

    if remainder > 0.01 {
        chunks.push(Chunk {
            index: num_full,
            start_seconds: num_full as f64 * chunk_dur,
            end_seconds: duration_seconds,
        });
    }

    chunks
}

/// Check that ffmpeg and ffprobe are available and return their versions.
pub async fn check_ffmpeg(config: &FfmpegConfig) -> Result<FfmpegInfo, VttError> {
    let ffmpeg_version = get_version(&config.path).await?;
    let ffprobe_version = get_version(&config.ffprobe_path()).await?;

    Ok(FfmpegInfo {
        ffmpeg_version,
        ffprobe_version,
    })
}

async fn get_version(binary: &str) -> Result<String, VttError> {
    let output = Command::new(binary)
        .arg("-version")
        .output()
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to run {binary}: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VttError::Ffmpeg(format!(
            "{binary} exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let version_line = stdout.lines().next().unwrap_or("unknown").to_string();
    Ok(version_line)
}

/// Probe a video file and return its duration in seconds.
pub async fn probe_duration(config: &FfmpegConfig, video_path: &Path) -> Result<f64, VttError> {
    let output = Command::new(&config.ffprobe_path())
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
        ])
        .arg(video_path)
        .output()
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to run ffprobe: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VttError::Ffmpeg(format!(
            "ffprobe exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| VttError::Ffmpeg(format!("failed to parse ffprobe output: {e}")))?;

    let duration_str = json["format"]["duration"]
        .as_str()
        .ok_or_else(|| VttError::Ffmpeg("ffprobe output missing format.duration".into()))?;

    duration_str
        .parse::<f64>()
        .map_err(|e| VttError::Ffmpeg(format!("failed to parse duration '{duration_str}': {e}")))
}

/// Extract audio for a single chunk as a WAV file (16kHz mono for Whisper).
/// Returns the path to the created WAV file.
pub async fn extract_audio(
    config: &FfmpegConfig,
    video_path: &Path,
    chunk: &Chunk,
    output_dir: &Path,
) -> Result<PathBuf, VttError> {
    let audio_path = output_dir.join("audio.wav");

    let output = build_audio_command(config, video_path, chunk, &audio_path)
        .output()
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to run ffmpeg audio extraction: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VttError::Ffmpeg(format!(
            "ffmpeg audio extraction exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    Ok(audio_path)
}

/// Extract frames for a single chunk as image files at the configured FPS.
/// Returns paths to all created image files, sorted by name.
pub async fn extract_frames(
    config: &FfmpegConfig,
    vision_config: &VisionConfig,
    video_path: &Path,
    chunk: &Chunk,
    output_dir: &Path,
) -> Result<Vec<PathBuf>, VttError> {
    let frames_dir = output_dir.join("frames");
    tokio::fs::create_dir_all(&frames_dir)
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to create frames dir: {e}")))?;

    let output = build_frames_command(config, vision_config, video_path, chunk, &frames_dir)
        .output()
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to run ffmpeg frame extraction: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VttError::Ffmpeg(format!(
            "ffmpeg frame extraction exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    let mut frame_paths = Vec::new();
    let mut entries = tokio::fs::read_dir(&frames_dir)
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to read frames dir: {e}")))?;

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        VttError::Ffmpeg(format!("failed to read frames dir entry: {e}"))
    })? {
        let path = entry.path();
        if path.is_file() {
            frame_paths.push(path);
        }
    }

    frame_paths.sort();
    Ok(frame_paths)
}

/// Prepare all artifacts for all chunks of a video.
/// Creates directory structure, extracts audio and frames for each chunk.
pub async fn prepare_chunks(
    config: &ServerConfig,
    video_path: &Path,
    job_id: &str,
) -> Result<ChunkManifest, VttError> {
    let job_dir = Path::new(&config.processing.temp_dir)
        .join(job_id)
        .join("chunks");

    tokio::fs::create_dir_all(&job_dir)
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to create job dir: {e}")))?;

    let duration_seconds = probe_duration(&config.ffmpeg, video_path).await?;
    let chunks = compute_chunks(duration_seconds, config.ffmpeg.chunk_duration_secs);

    let mut artifacts = Vec::new();
    for chunk in &chunks {
        let chunk_dir = job_dir.join(format!("chunk_{:03}", chunk.index));
        tokio::fs::create_dir_all(&chunk_dir)
            .await
            .map_err(|e| VttError::Ffmpeg(format!("failed to create chunk dir: {e}")))?;

        let audio_path =
            extract_audio(&config.ffmpeg, video_path, chunk, &chunk_dir).await?;
        let frame_paths =
            extract_frames(&config.ffmpeg, &config.vision, video_path, chunk, &chunk_dir).await?;

        artifacts.push(ChunkArtifacts {
            chunk: chunk.clone(),
            audio_path,
            frame_paths,
        });
    }

    Ok(ChunkManifest {
        job_dir,
        duration_seconds,
        chunks: artifacts,
    })
}

// --- Command builders (internal, tested via argument inspection) ---

fn build_audio_command(
    config: &FfmpegConfig,
    video_path: &Path,
    chunk: &Chunk,
    output_path: &Path,
) -> Command {
    let mut cmd = Command::new(&config.path);
    cmd.args([
        "-y",
        "-ss",
        &chunk.start_seconds.to_string(),
        "-to",
        &chunk.end_seconds.to_string(),
        "-i",
    ]);
    cmd.arg(video_path);
    cmd.args(["-vn", "-acodec", "pcm_s16le", "-ar", "16000", "-ac", "1"]);
    cmd.arg(output_path);
    cmd
}

fn build_frames_command(
    config: &FfmpegConfig,
    vision_config: &VisionConfig,
    video_path: &Path,
    chunk: &Chunk,
    frames_dir: &Path,
) -> Command {
    let pattern = frames_dir.join(format!("frame_%06d.{}", config.frame_format));

    let mut cmd = Command::new(&config.path);
    cmd.args([
        "-y",
        "-ss",
        &chunk.start_seconds.to_string(),
        "-to",
        &chunk.end_seconds.to_string(),
        "-i",
    ]);
    cmd.arg(video_path);
    cmd.args([
        "-vf",
        &format!("fps={}", vision_config.fps),
        "-q:v",
        &config.frame_quality.to_string(),
    ]);
    cmd.arg(&pattern);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- compute_chunks tests ---

    #[test]
    fn test_compute_chunks_short_video() {
        let chunks = compute_chunks(10.0, 180);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].index, 0);
        assert!((chunks[0].start_seconds - 0.0).abs() < 0.001);
        assert!((chunks[0].end_seconds - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_chunks_exact_multiple() {
        let chunks = compute_chunks(360.0, 180);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].index, 0);
        assert!((chunks[0].end_seconds - 180.0).abs() < 0.001);
        assert_eq!(chunks[1].index, 1);
        assert!((chunks[1].start_seconds - 180.0).abs() < 0.001);
        assert!((chunks[1].end_seconds - 360.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_chunks_with_remainder() {
        let chunks = compute_chunks(400.0, 180);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[2].index, 2);
        assert!((chunks[2].start_seconds - 360.0).abs() < 0.001);
        assert!((chunks[2].end_seconds - 400.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_chunks_zero_duration() {
        let chunks = compute_chunks(0.0, 180);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_compute_chunks_negative_duration() {
        let chunks = compute_chunks(-5.0, 180);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_compute_chunks_zero_chunk_size() {
        let chunks = compute_chunks(100.0, 0);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_compute_chunks_very_short_video() {
        let chunks = compute_chunks(0.5, 180);
        assert_eq!(chunks.len(), 1);
        assert!((chunks[0].end_seconds - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_compute_chunks_long_video() {
        // 10 hours at 3 min chunks = 200 chunks
        let chunks = compute_chunks(36000.0, 180);
        assert_eq!(chunks.len(), 200);
        assert_eq!(chunks[199].index, 199);
        assert!((chunks[199].end_seconds - 36000.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_chunks_duration_equals_chunk() {
        let chunks = compute_chunks(180.0, 180);
        assert_eq!(chunks.len(), 1);
        assert!((chunks[0].end_seconds - 180.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_chunks_contiguous() {
        let chunks = compute_chunks(500.0, 180);
        for i in 1..chunks.len() {
            assert!(
                (chunks[i].start_seconds - chunks[i - 1].end_seconds).abs() < 0.001,
                "gap between chunk {} and {}",
                i - 1,
                i
            );
        }
    }

    // --- Command builder tests ---

    #[test]
    fn test_build_audio_command_args() {
        let config = FfmpegConfig::default();
        let chunk = Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 180.0,
        };
        let cmd = build_audio_command(
            &config,
            Path::new("/tmp/video.mp4"),
            &chunk,
            Path::new("/tmp/out/audio.wav"),
        );
        let prog = cmd.as_std().get_program().to_str().unwrap().to_string();
        let args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|a| a.to_str().unwrap().to_string())
            .collect();

        assert_eq!(prog, "ffmpeg");
        assert!(args.contains(&"-y".to_string()));
        assert!(args.contains(&"-vn".to_string()));
        assert!(args.contains(&"-acodec".to_string()));
        assert!(args.contains(&"pcm_s16le".to_string()));
        assert!(args.contains(&"-ar".to_string()));
        assert!(args.contains(&"16000".to_string()));
        assert!(args.contains(&"-ac".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(args.contains(&"/tmp/video.mp4".to_string()));
        assert!(args.contains(&"/tmp/out/audio.wav".to_string()));
    }

    #[test]
    fn test_build_frames_command_args() {
        let config = FfmpegConfig::default();
        let vision = VisionConfig {
            fps: 2.0,
            max_tokens: 4096,
            max_frames_per_request: 360,
            ..Default::default()
        };
        let chunk = Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 180.0,
        };
        let cmd = build_frames_command(
            &config,
            &vision,
            Path::new("/tmp/video.mp4"),
            &chunk,
            Path::new("/tmp/out/frames"),
        );
        let args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|a| a.to_str().unwrap().to_string())
            .collect();

        assert!(args.contains(&"-y".to_string()));
        assert!(args.contains(&"-vf".to_string()));
        assert!(args.contains(&"fps=2".to_string()));
        assert!(args.contains(&"-q:v".to_string()));
        assert!(args.contains(&"2".to_string()));
        assert!(args.iter().any(|a| a.contains("frame_%06d.jpg")));
    }

    #[test]
    fn test_build_frames_command_png_format() {
        let mut config = FfmpegConfig::default();
        config.frame_format = "png".to_string();
        let vision = VisionConfig {
            fps: 1.0,
            max_tokens: 4096,
            max_frames_per_request: 360,
            ..Default::default()
        };
        let chunk = Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 60.0,
        };
        let cmd = build_frames_command(
            &config,
            &vision,
            Path::new("/tmp/video.mp4"),
            &chunk,
            Path::new("/tmp/out/frames"),
        );
        let args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|a| a.to_str().unwrap().to_string())
            .collect();

        assert!(args.iter().any(|a| a.contains("frame_%06d.png")));
        assert!(args.contains(&"fps=1".to_string()));
    }

    // --- Integration tests (require ffmpeg installed) ---

    #[tokio::test]
    #[ignore]
    async fn test_check_ffmpeg_available() {
        let config = FfmpegConfig::default();
        let info = check_ffmpeg(&config).await.unwrap();
        assert!(info.ffmpeg_version.contains("ffmpeg"));
        assert!(info.ffprobe_version.contains("ffprobe"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_probe_duration() {
        let config = FfmpegConfig::default();
        let dir = tempfile::tempdir().unwrap();
        let video_path = dir.path().join("test.mp4");

        // Generate a 5-second test video
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=5:size=320x240:rate=30",
                "-f", "lavfi",
                "-i", "sine=frequency=440:duration=5",
                "-shortest",
            ])
            .arg(&video_path)
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());

        let duration = probe_duration(&config, &video_path).await.unwrap();
        assert!((duration - 5.0).abs() < 0.5);
    }

    #[tokio::test]
    #[ignore]
    async fn test_extract_audio_wav() {
        let config = FfmpegConfig::default();
        let dir = tempfile::tempdir().unwrap();
        let video_path = dir.path().join("test.mp4");

        std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=5:size=320x240:rate=30",
                "-f", "lavfi",
                "-i", "sine=frequency=440:duration=5",
                "-shortest",
            ])
            .arg(&video_path)
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();

        let chunk = Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 5.0,
        };
        let out_dir = dir.path().join("chunk_000");
        tokio::fs::create_dir_all(&out_dir).await.unwrap();

        let audio_path = extract_audio(&config, &video_path, &chunk, &out_dir)
            .await
            .unwrap();
        assert!(audio_path.exists());
        assert_eq!(audio_path.extension().unwrap(), "wav");
    }

    #[tokio::test]
    #[ignore]
    async fn test_extract_frames_jpeg() {
        let config = FfmpegConfig::default();
        let vision = VisionConfig {
            fps: 2.0,
            max_tokens: 4096,
            max_frames_per_request: 360,
            ..Default::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let video_path = dir.path().join("test.mp4");

        std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=5:size=320x240:rate=30",
                "-f", "lavfi",
                "-i", "sine=frequency=440:duration=5",
                "-shortest",
            ])
            .arg(&video_path)
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();

        let chunk = Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 5.0,
        };
        let out_dir = dir.path().join("chunk_000");
        tokio::fs::create_dir_all(&out_dir).await.unwrap();

        let frames = extract_frames(&config, &vision, &video_path, &chunk, &out_dir)
            .await
            .unwrap();
        // 5 seconds at 2 fps = ~10 frames
        assert!(frames.len() >= 9 && frames.len() <= 11);
        assert!(frames[0].extension().unwrap() == "jpg");
    }

    #[tokio::test]
    #[ignore]
    async fn test_prepare_chunks_end_to_end() {
        let mut config = ServerConfig::default();
        let dir = tempfile::tempdir().unwrap();
        config.processing.temp_dir = dir.path().display().to_string();
        config.ffmpeg.chunk_duration_secs = 3;

        let video_path = dir.path().join("test.mp4");
        std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=8:size=320x240:rate=30",
                "-f", "lavfi",
                "-i", "sine=frequency=440:duration=8",
                "-shortest",
            ])
            .arg(&video_path)
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();

        let manifest = prepare_chunks(&config, &video_path, "test-job").await.unwrap();

        assert!((manifest.duration_seconds - 8.0).abs() < 0.5);
        // 8 seconds / 3 second chunks = 2 full + 1 remainder = 3 chunks
        assert_eq!(manifest.chunks.len(), 3);

        for artifact in &manifest.chunks {
            assert!(artifact.audio_path.exists());
            assert!(!artifact.frame_paths.is_empty());
        }
    }
}
