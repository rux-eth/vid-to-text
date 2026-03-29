use std::path::Path;
use std::sync::Arc;

use crate::{
    clear_checkpoints, load_checkpoints, parse_timestamp, prepare_chunks, save_checkpoint,
    transcribe_chunk, OllamaClient, Segment, SegmentType, ServerConfig, Timeline, VttError,
    WhisperModel,
};

/// Process a video file through the full pipeline:
/// chunk → Whisper (CPU) → Vision with transcript (GPU) → merge → Timeline.
///
/// For each chunk, Whisper runs first so the transcript can be passed
/// to the vision model as context. Completed chunks are checkpointed
/// to disk for resumability. If `force` is true, existing checkpoints
/// are cleared and all chunks are reprocessed.
pub async fn process_video(
    config: &ServerConfig,
    video_path: &Path,
    job_id: &str,
    force: bool,
) -> Result<Timeline, VttError> {
    config.validate()?;

    let manifest = prepare_chunks(config, video_path, job_id).await?;

    let source = video_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| video_path.display().to_string());

    // Load or clear checkpoints
    let cached = if force {
        clear_checkpoints(&config.processing.temp_dir, job_id).await?;
        std::collections::HashMap::new()
    } else {
        load_checkpoints(&config.processing.temp_dir, job_id).await?
    };

    let all_cached = cached.len() == manifest.chunks.len() && !manifest.chunks.is_empty();

    // Only initialize models if there are uncached chunks to process
    let whisper_model = if all_cached {
        None
    } else {
        Some(Arc::new(WhisperModel::new(&config.whisper)?))
    };

    let ollama_client = if all_cached {
        None
    } else {
        Some(OllamaClient::new(&config.ollama, &config.vision)?)
    };

    let mut all_chunk_segments = Vec::new();

    for artifact in &manifest.chunks {
        let chunk_segments = if let Some(segments) = cached.get(&artifact.chunk.index) {
            segments.clone()
        } else {
            let model = whisper_model.as_ref().unwrap();
            let client = ollama_client.as_ref().unwrap();

            // Step 1: Whisper transcription (CPU, via spawn_blocking)
            let whisper_segments = transcribe_chunk(
                Arc::clone(model),
                artifact.audio_path.clone(),
                artifact.chunk.clone(),
            )
            .await?;

            // Step 2: Extract transcript for vision context
            let transcript = extract_transcript(&whisper_segments);

            // Step 3: Vision description (GPU, via Ollama HTTP)
            let vision_segments = client
                .describe_chunk(
                    &artifact.chunk,
                    &artifact.frame_paths,
                    transcript.as_deref(),
                )
                .await?;

            // Combine segments from both pipelines
            let mut combined = whisper_segments;
            combined.extend(vision_segments);

            // Checkpoint only after both pipelines complete
            save_checkpoint(
                &config.processing.temp_dir,
                job_id,
                artifact.chunk.index,
                &combined,
            )
            .await?;

            combined
        };

        all_chunk_segments.push(chunk_segments);
    }

    let timeline = merge_segments(&source, manifest.duration_seconds, all_chunk_segments);

    // Clean up checkpoints after successful merge
    if config.processing.cleanup_checkpoints {
        let _ = clear_checkpoints(&config.processing.temp_dir, job_id).await;
    }

    Ok(timeline)
}

/// Merge segments from all chunks into a single sorted Timeline.
/// Segments are sorted by start timestamp. Pure function, no I/O.
pub fn merge_segments(
    source: &str,
    duration_seconds: f64,
    chunk_segments: Vec<Vec<Segment>>,
) -> Timeline {
    let mut segments: Vec<Segment> = chunk_segments.into_iter().flatten().collect();

    segments.sort_by(|a, b| {
        let a_time = parse_timestamp(&a.start).unwrap_or(0.0);
        let b_time = parse_timestamp(&b.start).unwrap_or(0.0);
        a_time
            .partial_cmp(&b_time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Timeline {
        source: source.to_string(),
        duration_seconds,
        segments,
    }
}

/// Extract speech content from Whisper segments to use as vision context.
/// Returns None if there are no speech segments.
pub fn extract_transcript(segments: &[Segment]) -> Option<String> {
    let speech: Vec<&str> = segments
        .iter()
        .filter(|s| s.segment_type == SegmentType::Speech)
        .map(|s| s.content.as_str())
        .filter(|c| !c.trim().is_empty())
        .collect();

    if speech.is_empty() {
        None
    } else {
        Some(speech.join(" "))
    }
}

/// Helper to create a Segment for use in tests.
#[cfg(test)]
fn make_segment(segment_type: SegmentType, start_secs: f64, end_secs: f64, content: &str) -> Segment {
    use crate::format_timestamp;
    Segment {
        segment_type,
        start: format_timestamp(start_secs),
        end: format_timestamp(end_secs),
        content: content.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- merge_segments tests ---

    #[test]
    fn test_merge_empty_input() {
        let timeline = merge_segments("video.mp4", 100.0, vec![]);
        assert_eq!(timeline.source, "video.mp4");
        assert_eq!(timeline.duration_seconds, 100.0);
        assert!(timeline.segments.is_empty());
    }

    #[test]
    fn test_merge_single_segment() {
        let seg = make_segment(SegmentType::Speech, 1.0, 2.0, "Hello");
        let timeline = merge_segments("video.mp4", 10.0, vec![vec![seg]]);
        assert_eq!(timeline.segments.len(), 1);
        assert_eq!(timeline.segments[0].content, "Hello");
    }

    #[test]
    fn test_merge_multiple_types_one_chunk() {
        let speech = make_segment(SegmentType::Speech, 1.0, 3.0, "Hello");
        let visual = make_segment(SegmentType::Visual, 0.0, 5.0, "A person waves");
        let sound = make_segment(SegmentType::Sound, 2.0, 2.5, "music");
        let timeline = merge_segments("v.mp4", 5.0, vec![vec![speech, visual, sound]]);
        assert_eq!(timeline.segments.len(), 3);
        assert_eq!(timeline.segments[0].segment_type, SegmentType::Visual);
        assert_eq!(timeline.segments[1].segment_type, SegmentType::Speech);
        assert_eq!(timeline.segments[2].segment_type, SegmentType::Sound);
    }

    #[test]
    fn test_merge_multiple_chunks_sorted() {
        let chunk1 = vec![make_segment(SegmentType::Speech, 5.0, 8.0, "Later speech")];
        let chunk2 = vec![make_segment(SegmentType::Speech, 1.0, 3.0, "Earlier speech")];
        let timeline = merge_segments("v.mp4", 10.0, vec![chunk1, chunk2]);
        assert_eq!(timeline.segments.len(), 2);
        assert_eq!(timeline.segments[0].content, "Earlier speech");
        assert_eq!(timeline.segments[1].content, "Later speech");
    }

    #[test]
    fn test_merge_interleaved_timestamps() {
        let chunk1 = vec![
            make_segment(SegmentType::Speech, 0.0, 2.0, "A"),
            make_segment(SegmentType::Speech, 4.0, 6.0, "C"),
        ];
        let chunk2 = vec![
            make_segment(SegmentType::Visual, 1.0, 3.0, "B"),
            make_segment(SegmentType::Visual, 5.0, 7.0, "D"),
        ];
        let timeline = merge_segments("v.mp4", 10.0, vec![chunk1, chunk2]);
        let contents: Vec<&str> = timeline.segments.iter().map(|s| s.content.as_str()).collect();
        assert_eq!(contents, vec!["A", "B", "C", "D"]);
    }

    #[test]
    fn test_merge_same_start_time() {
        let seg1 = make_segment(SegmentType::Speech, 1.0, 3.0, "Speech");
        let seg2 = make_segment(SegmentType::Visual, 1.0, 5.0, "Visual");
        let timeline = merge_segments("v.mp4", 5.0, vec![vec![seg1, seg2]]);
        assert_eq!(timeline.segments.len(), 2);
    }

    #[test]
    fn test_merge_preserves_source_and_duration() {
        let timeline = merge_segments("my_video.mp4", 3600.5, vec![vec![
            make_segment(SegmentType::Speech, 0.0, 1.0, "Hi"),
        ]]);
        assert_eq!(timeline.source, "my_video.mp4");
        assert_eq!(timeline.duration_seconds, 3600.5);
    }

    #[test]
    fn test_merge_large_segment_count() {
        let segments: Vec<Segment> = (0..500)
            .map(|i| make_segment(SegmentType::Speech, i as f64, (i + 1) as f64, &format!("seg{i}")))
            .collect();
        let timeline = merge_segments("v.mp4", 500.0, vec![segments]);
        assert_eq!(timeline.segments.len(), 500);
        for i in 1..timeline.segments.len() {
            let prev = parse_timestamp(&timeline.segments[i - 1].start).unwrap();
            let curr = parse_timestamp(&timeline.segments[i].start).unwrap();
            assert!(prev <= curr);
        }
    }

    #[test]
    fn test_merge_json_roundtrip() {
        let timeline = merge_segments("video.mp4", 60.0, vec![vec![
            make_segment(SegmentType::Speech, 1.0, 3.0, "Hello"),
            make_segment(SegmentType::Visual, 0.0, 5.0, "A person waves"),
            make_segment(SegmentType::Sound, 2.0, 2.5, "music"),
        ]]);
        let json = serde_json::to_string_pretty(&timeline).unwrap();
        let parsed: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(timeline, parsed);
    }

    // --- extract_transcript tests ---

    #[test]
    fn test_extract_transcript_empty() {
        assert_eq!(extract_transcript(&[]), None);
    }

    #[test]
    fn test_extract_transcript_only_sound() {
        let segments = vec![make_segment(SegmentType::Sound, 0.0, 1.0, "music")];
        assert_eq!(extract_transcript(&segments), None);
    }

    #[test]
    fn test_extract_transcript_only_visual() {
        let segments = vec![make_segment(SegmentType::Visual, 0.0, 5.0, "A cat sits")];
        assert_eq!(extract_transcript(&segments), None);
    }

    #[test]
    fn test_extract_transcript_single_speech() {
        let segments = vec![make_segment(SegmentType::Speech, 0.0, 2.0, "Hello world")];
        assert_eq!(extract_transcript(&segments), Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_transcript_multiple_speech() {
        let segments = vec![
            make_segment(SegmentType::Speech, 0.0, 2.0, "Hello"),
            make_segment(SegmentType::Speech, 2.0, 4.0, "world"),
        ];
        assert_eq!(extract_transcript(&segments), Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_transcript_mixed_types() {
        let segments = vec![
            make_segment(SegmentType::Speech, 0.0, 2.0, "Hello"),
            make_segment(SegmentType::Sound, 2.0, 3.0, "music"),
            make_segment(SegmentType::Speech, 3.0, 5.0, "world"),
            make_segment(SegmentType::Visual, 0.0, 5.0, "A person"),
        ];
        assert_eq!(extract_transcript(&segments), Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_transcript_skips_empty_speech() {
        let segments = vec![
            make_segment(SegmentType::Speech, 0.0, 1.0, "  "),
            make_segment(SegmentType::Speech, 1.0, 2.0, "Hello"),
        ];
        assert_eq!(extract_transcript(&segments), Some("Hello".to_string()));
    }

    // --- Integration tests (require ffmpeg + Whisper model + Ollama) ---

    #[tokio::test]
    #[ignore]
    async fn test_process_video_end_to_end() {
        let mut config = ServerConfig::default();
        let dir = tempfile::tempdir().unwrap();
        config.processing.temp_dir = dir.path().display().to_string();
        config.ffmpeg.chunk_duration_secs = 5;

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

        let timeline = process_video(&config, &video_path, "test-job", false).await.unwrap();

        assert!(!timeline.segments.is_empty());
        assert_eq!(timeline.source, "test.mp4");
        assert!(timeline.duration_seconds > 7.0);

        for i in 1..timeline.segments.len() {
            let prev = parse_timestamp(&timeline.segments[i - 1].start).unwrap();
            let curr = parse_timestamp(&timeline.segments[i].start).unwrap();
            assert!(prev <= curr);
        }

        assert!(timeline
            .segments
            .iter()
            .any(|s| s.segment_type == SegmentType::Visual));
    }
}
