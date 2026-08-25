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

/// One extracted frame with its real presentation time. (PR-022)
///
/// `timestamp` is absolute (chunk start + ffmpeg `pts_time`), never derived
/// from an assumed frame spacing -- see the Visual Timestamps Are Frame
/// Timestamps constraint. `scene_score` is ffmpeg's `select` scene score of this
/// candidate against the previous candidate (0..1); it is what adaptive
/// selection triggered on and what the per-chunk cap ranks by.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSample {
    pub path: PathBuf,
    pub timestamp: f64,
    pub scene_score: f64,
}

/// Artifacts produced for a single chunk.
#[derive(Debug, Clone)]
pub struct ChunkArtifacts {
    pub chunk: Chunk,
    pub audio_path: PathBuf,
    pub frames: Vec<FrameSample>,
}

/// Describes all prepared artifacts for a job.
#[derive(Debug)]
pub struct ChunkManifest {
    pub job_dir: PathBuf,
    pub duration_seconds: f64,
    /// Source resolution, probed once; drives the per-request token pre-flight.
    pub width: u32,
    pub height: u32,
    pub chunks: Vec<ChunkArtifacts>,
}

/// Container-level facts from ffprobe.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
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

/// Probe a video file for its duration and the resolution of its first video stream.
pub async fn probe_video(config: &FfmpegConfig, video_path: &Path) -> Result<VideoInfo, VttError> {
    let output = Command::new(&config.ffprobe_path())
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            "-select_streams",
            "v:0",
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
    parse_probe_json(&json)
}

/// Pure parser for `ffprobe -show_format -show_streams -select_streams v:0` JSON.
fn parse_probe_json(json: &serde_json::Value) -> Result<VideoInfo, VttError> {
    let duration_str = json["format"]["duration"]
        .as_str()
        .ok_or_else(|| VttError::Ffmpeg("ffprobe output missing format.duration".into()))?;
    let duration_seconds = duration_str
        .parse::<f64>()
        .map_err(|e| VttError::Ffmpeg(format!("failed to parse duration '{duration_str}': {e}")))?;
    let stream = json["streams"]
        .get(0)
        .ok_or_else(|| VttError::Ffmpeg("ffprobe output has no video stream".into()))?;
    let dim = |key: &str| -> Result<u32, VttError> {
        stream[key]
            .as_u64()
            .filter(|v| *v > 0)
            .map(|v| v as u32)
            .ok_or_else(|| VttError::Ffmpeg(format!("ffprobe output missing streams[0].{key}")))
    };
    Ok(VideoInfo {
        duration_seconds,
        width: dim("width")?,
        height: dim("height")?,
    })
}

/// Probe a video file and return its duration in seconds.
pub async fn probe_duration(config: &FfmpegConfig, video_path: &Path) -> Result<f64, VttError> {
    Ok(probe_video(config, video_path).await?.duration_seconds)
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

/// The ffmpeg `select` expression for a chunk. (PR-022)
///
/// Fixed mode keeps every candidate (`gte(scene,0)` is always true) but still
/// computes the scene score so `metadata=print` emits a line per frame -- without
/// a computed key it prints nothing. Adaptive mode keeps the first frame, a floor
/// frame whenever `max_gap_secs` have passed since the last kept frame, and a
/// trigger when the scene score exceeds the threshold and the refractory interval
/// has passed. `prev_selected_t` is the previously SELECTED frame, so both the
/// floor and the refractory are measured from what was actually kept.
pub fn select_expression(vision: &VisionConfig) -> String {
    if !vision.adaptive.enabled {
        return "gte(scene,0)".to_string();
    }
    let a = &vision.adaptive;
    format!(
        "eq(n,0)+gte(t-prev_selected_t,{})+gt(scene,{})*gte(t-prev_selected_t,{})",
        a.max_gap_secs, a.scene_threshold, a.min_trigger_interval_secs
    )
}

/// Parse ffmpeg's `metadata=print` log lines into `(pts_time, scene_score)` pairs.
///
/// Every kept frame produces a `... pts_time:<secs>` line followed by a
/// `lavfi.scene_score=<score>` line. Other log lines are ignored. A frame line
/// without a score, or a score without a frame, is an error: the timestamps are
/// what every visual segment is labelled with, so guessing is not an option.
pub fn parse_frame_metadata(log: &str) -> Result<Vec<(f64, f64)>, VttError> {
    let mut out = Vec::new();
    let mut pending: Option<f64> = None;
    for line in log.lines() {
        if let Some(idx) = line.find("pts_time:") {
            if pending.is_some() {
                return Err(VttError::Ffmpeg(
                    "frame metadata: pts_time line without a following scene score".into(),
                ));
            }
            let rest = &line[idx + "pts_time:".len()..];
            let tok = rest.split_whitespace().next().unwrap_or("");
            let t = tok.parse::<f64>().map_err(|e| {
                VttError::Ffmpeg(format!("frame metadata: bad pts_time '{tok}': {e}"))
            })?;
            pending = Some(t);
        } else if let Some(idx) = line.find("lavfi.scene_score=") {
            let rest = &line[idx + "lavfi.scene_score=".len()..];
            let tok = rest.split_whitespace().next().unwrap_or("");
            let score = tok.parse::<f64>().map_err(|e| {
                VttError::Ffmpeg(format!("frame metadata: bad scene_score '{tok}': {e}"))
            })?;
            let t = pending.take().ok_or_else(|| {
                VttError::Ffmpeg("frame metadata: scene score without a preceding pts_time".into())
            })?;
            out.push((t, score));
        }
    }
    if pending.is_some() {
        return Err(VttError::Ffmpeg(
            "frame metadata: trailing pts_time line without a scene score".into(),
        ));
    }
    Ok(out)
}

/// Pair the image files ffmpeg wrote (sorted by name) with the metadata it logged.
///
/// The counts must agree exactly; a mismatch means the labels would be wrong for
/// every frame after the divergence, so the chunk fails instead.
pub fn pair_frames(
    paths: Vec<PathBuf>,
    metadata: Vec<(f64, f64)>,
    chunk_start_seconds: f64,
) -> Result<Vec<FrameSample>, VttError> {
    if paths.len() != metadata.len() {
        return Err(VttError::Ffmpeg(format!(
            "frame extraction wrote {} image file(s) but logged metadata for {} frame(s); \
             refusing to guess timestamps (is -vsync vfr in effect?)",
            paths.len(),
            metadata.len()
        )));
    }
    Ok(paths
        .into_iter()
        .zip(metadata)
        .map(|(path, (pts_time, scene_score))| FrameSample {
            path,
            timestamp: chunk_start_seconds + pts_time,
            scene_score,
        })
        .collect())
}

/// Enforce `max_frames_per_chunk`: drop the lowest-scoring TRIGGER frames until
/// the chunk fits. Floor frames -- the first frame and any frame at least
/// `max_gap_secs` after its predecessor -- are never dropped, so the floor
/// guarantee survives capping (a dropped trigger between two floor frames can
/// widen one gap to at most 2 x max_gap_secs). Returns frames in time order.
pub fn apply_frame_cap(mut frames: Vec<FrameSample>, cap: usize, max_gap_secs: f64) -> Vec<FrameSample> {
    if frames.len() <= cap {
        return frames;
    }
    frames.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap_or(std::cmp::Ordering::Equal));
    let eps = 1e-6;
    let mut is_floor = vec![false; frames.len()];
    for i in 0..frames.len() {
        is_floor[i] = i == 0 || frames[i].timestamp - frames[i - 1].timestamp >= max_gap_secs - eps;
    }
    // Trigger indices, lowest score first; later frames first on ties.
    let mut triggers: Vec<usize> = (0..frames.len()).filter(|&i| !is_floor[i]).collect();
    triggers.sort_by(|&a, &b| {
        frames[a]
            .scene_score
            .partial_cmp(&frames[b].scene_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(frames[b].timestamp.partial_cmp(&frames[a].timestamp).unwrap_or(std::cmp::Ordering::Equal))
    });
    let to_drop = frames.len() - cap;
    let dropped: std::collections::HashSet<usize> = triggers.into_iter().take(to_drop).collect();
    frames
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !dropped.contains(i))
        .map(|(_, f)| f)
        .collect()
}

/// Extract frames for a single chunk as image files, with real timestamps.
///
/// Both modes run `fps=N,select='...',metadata=print` under `-vsync vfr`; the
/// metadata lines on stderr give every kept frame's `pts_time` and scene score.
/// Adaptive mode additionally applies `max_frames_per_chunk`.
pub async fn extract_frames(
    config: &FfmpegConfig,
    vision_config: &VisionConfig,
    video_path: &Path,
    chunk: &Chunk,
    output_dir: &Path,
) -> Result<Vec<FrameSample>, VttError> {
    let frames_dir = output_dir.join("frames");
    tokio::fs::create_dir_all(&frames_dir)
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to create frames dir: {e}")))?;

    let output = build_frames_command(config, vision_config, video_path, chunk, &frames_dir)
        .output()
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to run ffmpeg frame extraction: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
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

    let metadata = parse_frame_metadata(&stderr)?;
    let frames = pair_frames(frame_paths, metadata, chunk.start_seconds)?;

    let a = &vision_config.adaptive;
    if a.enabled && frames.len() > a.max_frames_per_chunk as usize {
        eprintln!(
            "[frames] chunk_{} kept {} frames, capping to {} (dropping lowest-scoring triggers)",
            chunk.index,
            frames.len(),
            a.max_frames_per_chunk
        );
        return Ok(apply_frame_cap(frames, a.max_frames_per_chunk as usize, a.max_gap_secs));
    }
    Ok(frames)
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

    let info = probe_video(&config.ffmpeg, video_path).await?;
    let duration_seconds = info.duration_seconds;
    let chunks = compute_chunks(duration_seconds, config.ffmpeg.chunk_duration_secs);

    let mut artifacts = Vec::new();
    for chunk in &chunks {
        let chunk_dir = job_dir.join(format!("chunk_{:03}", chunk.index));
        tokio::fs::create_dir_all(&chunk_dir)
            .await
            .map_err(|e| VttError::Ffmpeg(format!("failed to create chunk dir: {e}")))?;

        let audio_path =
            extract_audio(&config.ffmpeg, video_path, chunk, &chunk_dir).await?;
        let frames =
            extract_frames(&config.ffmpeg, &config.vision, video_path, chunk, &chunk_dir).await?;

        artifacts.push(ChunkArtifacts {
            chunk: chunk.clone(),
            audio_path,
            frames,
        });
    }

    Ok(ChunkManifest {
        job_dir,
        duration_seconds,
        width: info.width,
        height: info.height,
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
    // `-nostats` keeps progress lines off stderr so only log lines remain;
    // `metadata=print` writes one `pts_time` + `lavfi.scene_score` pair per kept
    // frame there. `-vsync vfr` is REQUIRED: without it ffmpeg duplicates frames
    // to hold a constant rate after `select` drops any (393 files were written
    // for 9 selected frames on 4.4.2). (PR-022)
    cmd.args([
        "-nostats",
        "-vf",
        &format!(
            "fps={},select='{}',metadata=print",
            vision_config.fps,
            select_expression(vision_config)
        ),
        "-vsync",
        "vfr",
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
        let vf = args.iter().find(|a| a.starts_with("fps=")).expect("-vf value");
        assert!(vf.starts_with("fps=2,select='gte(scene,0)',metadata=print"), "{vf}");
        assert!(args.contains(&"-vsync".to_string()) && args.contains(&"vfr".to_string()));
        assert!(args.contains(&"-nostats".to_string()));
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
        assert!(args.iter().any(|a| a.starts_with("fps=1,select=")));
    }

    // --- PR-022: adaptive selection, metadata parsing, capping ---

    #[test]
    fn test_select_expression_fixed_keeps_everything_but_scores() {
        let vision = VisionConfig::default();
        assert_eq!(select_expression(&vision), "gte(scene,0)");
    }

    #[test]
    fn test_select_expression_adaptive_encodes_floor_trigger_refractory() {
        let mut vision = VisionConfig::default();
        vision.adaptive.enabled = true;
        vision.adaptive.scene_threshold = 0.08;
        vision.adaptive.max_gap_secs = 15.0;
        vision.adaptive.min_trigger_interval_secs = 2.0;
        assert_eq!(
            select_expression(&vision),
            "eq(n,0)+gte(t-prev_selected_t,15)+gt(scene,0.08)*gte(t-prev_selected_t,2)"
        );
    }

    #[test]
    fn test_build_frames_command_adaptive_uses_select_and_vfr() {
        let config = FfmpegConfig::default();
        let mut vision = VisionConfig::default();
        vision.adaptive.enabled = true;
        let chunk = Chunk { index: 0, start_seconds: 0.0, end_seconds: 180.0 };
        let cmd = build_frames_command(&config, &vision, Path::new("/tmp/v.mp4"), &chunk, Path::new("/tmp/f"));
        let args: Vec<String> = cmd.as_std().get_args().map(|a| a.to_str().unwrap().to_string()).collect();
        let vf = args.iter().find(|a| a.starts_with("fps=")).unwrap();
        assert!(vf.contains("select='eq(n,0)+gte(t-prev_selected_t,15)"), "{vf}");
        assert!(vf.ends_with(",metadata=print"), "{vf}");
        let i = args.iter().position(|a| a == "-vsync").unwrap();
        assert_eq!(args[i + 1], "vfr");
    }

    /// Real ffmpeg 4.4.2 output, including an interleaved progress line, which
    /// appears when -nostats is absent and must not confuse the parser.
    #[test]
    fn test_parse_frame_metadata_real_log() {
        let log = "[Parsed_metadata_2 @ 0x59cb] frame:0    pts:0       pts_time:0\n\
[Parsed_metadata_2 @ 0x59cb] lavfi.scene_score=0.000000\n\
frame=    1 fps=0.0 q=0.0 size=N/A time=00:00:00.00 bitrate=N/A speed=   0x    [Parsed_metadata_2 @ 0x59cb] frame:1    pts:25      pts_time:12.5\n\
[Parsed_metadata_2 @ 0x59cb] lavfi.scene_score=0.447830\n\
video:0kB audio:0kB subtitle:0kB other streams:0kB global headers:0kB muxing overhead: unknown\n";
        let parsed = parse_frame_metadata(log).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], (0.0, 0.0));
        assert!((parsed[1].0 - 12.5).abs() < 1e-9);
        assert!((parsed[1].1 - 0.447830).abs() < 1e-9);
    }

    #[test]
    fn test_parse_frame_metadata_rejects_orphans() {
        assert!(parse_frame_metadata("x pts_time:1.0\nx pts_time:2.0\nx lavfi.scene_score=0.1\n").is_err());
        assert!(parse_frame_metadata("x lavfi.scene_score=0.1\n").is_err());
        assert!(parse_frame_metadata("x pts_time:1.0\n").is_err());
        assert!(parse_frame_metadata("").unwrap().is_empty());
    }

    #[test]
    fn test_pair_frames_offsets_by_chunk_start_and_rejects_mismatch() {
        let paths = vec![PathBuf::from("/f/1.jpg"), PathBuf::from("/f/2.jpg")];
        let frames = pair_frames(paths.clone(), vec![(0.0, 0.0), (12.5, 0.4)], 180.0).unwrap();
        assert_eq!(frames[1].timestamp, 192.5);
        assert_eq!(frames[1].scene_score, 0.4);
        assert_eq!(frames[1].path, PathBuf::from("/f/2.jpg"));
        let err = pair_frames(paths, vec![(0.0, 0.0)], 0.0).unwrap_err().to_string();
        assert!(err.contains("2 image file(s)") && err.contains("1 frame(s)"), "{err}");
    }

    fn fs(t: f64, score: f64) -> FrameSample {
        FrameSample { path: PathBuf::from(format!("/f/{t}.jpg")), timestamp: t, scene_score: score }
    }

    #[test]
    fn test_apply_frame_cap_drops_lowest_triggers_never_floor() {
        // As ffmpeg's rule produces them (floor = 30 s since the previous KEPT frame):
        // 0 floor; 10 trigger 0.5; 12 trigger 0.1; 42 floor (30 s after 12);
        // 50 trigger 0.3; 80 floor (30 s after 50).
        let frames = vec![fs(0.0, 0.0), fs(10.0, 0.5), fs(12.0, 0.1), fs(42.0, 0.0), fs(50.0, 0.3), fs(80.0, 0.0)];
        let kept = apply_frame_cap(frames.clone(), 4, 30.0);
        let ts: Vec<f64> = kept.iter().map(|f| f.timestamp).collect();
        assert_eq!(ts, vec![0.0, 10.0, 42.0, 80.0], "drops 12 (0.1) then 50 (0.3), keeps all floor frames");
        // cap not exceeded: untouched
        assert_eq!(apply_frame_cap(frames.clone(), 6, 30.0), frames);
        // cap equal to floor count: only floor frames remain, in order
        let kept = apply_frame_cap(frames, 3, 30.0);
        let ts: Vec<f64> = kept.iter().map(|f| f.timestamp).collect();
        assert_eq!(ts, vec![0.0, 42.0, 80.0]);
    }

    #[test]
    fn test_parse_probe_json_reads_duration_and_resolution() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"streams":[{"width":1920,"height":1080}],"format":{"duration":"1534.313605"}}"#,
        )
        .unwrap();
        let info = parse_probe_json(&json).unwrap();
        assert_eq!(info.width, 1920);
        assert_eq!(info.height, 1080);
        assert!((info.duration_seconds - 1534.313605).abs() < 1e-9);
        let bad: serde_json::Value = serde_json::from_str(r#"{"streams":[],"format":{"duration":"1.0"}}"#).unwrap();
        assert!(parse_probe_json(&bad).is_err());
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
        // 5 seconds at 2 fps = ~10 frames, each with a real pts
        assert!(frames.len() >= 9 && frames.len() <= 11);
        assert!(frames[0].path.extension().unwrap() == "jpg");
        assert!((frames[0].timestamp - 0.0).abs() < 1e-6);
        assert!((frames[1].timestamp - 0.5).abs() < 1e-6);
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
            assert!(!artifact.frames.is_empty());
        }
    }
}
