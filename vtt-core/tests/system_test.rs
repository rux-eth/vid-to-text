use vtt_core::*;

#[tokio::test]
#[ignore]
async fn test_full_pipeline_with_synthetic_video() {
    let dir = tempfile::tempdir().unwrap();
    let video_path = dir.path().join("test_video.mp4");

    // Generate a 10-second test video with test pattern + sine tone
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "testsrc=duration=10:size=320x240:rate=30",
            "-f", "lavfi",
            "-i", "sine=frequency=440:duration=10",
            "-shortest",
        ])
        .arg(&video_path)
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg failed to generate test video");

    let mut config = ServerConfig::default();
    config.processing.temp_dir = dir.path().join("jobs").display().to_string();
    config.ffmpeg.chunk_duration_secs = 5;
    // Use absolute path — override for desktop
    config.whisper.model_path = std::env::var("WHISPER_MODEL_PATH")
        .unwrap_or_else(|_| "models/ggml-large-v3-turbo.bin".to_string());

    let timeline = process_video(&config, &video_path, "system-test", false, None, None).await.unwrap();

    // Print results
    println!("\n=== TIMELINE ===");
    println!("Source: {}", timeline.source);
    println!("Duration: {:.1}s", timeline.duration_seconds);
    println!("Segments: {}", timeline.segments.len());
    println!();

    for seg in &timeline.segments {
        let type_tag = match seg.segment_type {
            SegmentType::Speech => "[SPEECH]",
            SegmentType::Visual => "[VISUAL]",
            SegmentType::Sound => "[SOUND]",
        };
        let content_preview = if seg.content.len() > 120 {
            format!("{}...", &seg.content[..120])
        } else {
            seg.content.clone()
        };
        println!("{} {}-{}: {}", type_tag, seg.start, seg.end, content_preview);
    }

    println!("\n=== JSON ===");
    println!("{}", serde_json::to_string_pretty(&timeline).unwrap());

    // Assertions
    assert_eq!(timeline.source, "test_video.mp4");
    assert!(timeline.duration_seconds > 9.0);
    assert!(!timeline.segments.is_empty());

    let visual_count = timeline.segments.iter()
        .filter(|s| s.segment_type == SegmentType::Visual)
        .count();
    assert!(visual_count >= 2, "Expected at least 2 visual segments, got {visual_count}");

    // Verify sorted
    for i in 1..timeline.segments.len() {
        let prev = parse_timestamp(&timeline.segments[i - 1].start).unwrap();
        let curr = parse_timestamp(&timeline.segments[i].start).unwrap();
        assert!(prev <= curr, "Not sorted: {} > {}", timeline.segments[i-1].start, timeline.segments[i].start);
    }

    println!("\n=== SYSTEM TEST PASSED ===");
}
