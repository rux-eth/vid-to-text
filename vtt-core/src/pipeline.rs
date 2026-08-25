use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use tokio_util::sync::CancellationToken;

use crate::{
    check_context_budget, clear_checkpoints, load_checkpoints, parse_timestamp, prepare_chunks,
    run_ocr, save_checkpoint, score_segments, transcribe_chunk, write_thumbnails, CaptureInfo,
    FidelityReport, OcrFrame, OllamaClient, SamplingMode, Segment, SegmentType, ServerConfig,
    Timeline, TranscriptWindow, VttError, WhisperModel,
};
use crate::whisper::repetition_report;

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
    source_name: Option<&str>,
    cancel_token: Option<CancellationToken>,
) -> Result<Timeline, VttError> {
    config.validate()?;

    let check_cancelled = || -> Result<(), VttError> {
        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                return Err(VttError::Cancelled);
            }
        }
        Ok(())
    };

    let pipeline_start = Instant::now();

    let t = Instant::now();
    let manifest = prepare_chunks(config, video_path, job_id).await?;
    eprintln!("[timing] prepare_chunks: {:.1}s ({} chunks)", t.elapsed().as_secs_f64(), manifest.chunks.len());

    // Pre-flight the per-request token budget at the probed resolution BEFORE any
    // model is loaded: Ollama truncates an over-long prompt silently. (PR-022)
    let est = check_context_budget(&config.ollama, &config.vision, manifest.width, manifest.height)?;
    let total_frames: usize = manifest.chunks.iter().map(|c| c.frames.len()).sum();
    eprintln!(
        "[preflight] {}x{} source, {} frames selected ({}), full request ~{} of {} ctx tokens",
        manifest.width,
        manifest.height,
        total_frames,
        if config.vision.adaptive.enabled { "adaptive" } else { "fixed" },
        est,
        config.ollama.num_ctx
    );

    let source = source_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            video_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| video_path.display().to_string())
        });

    // Load or clear checkpoints
    let cached = if force {
        clear_checkpoints(&config.processing.temp_dir, job_id).await?;
        std::collections::HashMap::new()
    } else {
        load_checkpoints(&config.processing.temp_dir, job_id).await?
    };

    let all_cached = cached.len() == manifest.chunks.len() && !manifest.chunks.is_empty();

    // Only initialize models if there are uncached chunks to process
    let t = Instant::now();
    let whisper_model = if all_cached {
        None
    } else {
        Some(Arc::new(WhisperModel::new(&config.whisper)?))
    };
    if !all_cached {
        eprintln!("[timing] whisper_model_load: {:.1}s", t.elapsed().as_secs_f64());
    }

    let ollama_client = if all_cached {
        None
    } else {
        Some(OllamaClient::new(&config.ollama, &config.vision)?)
    };

    // OCR is needed by grounded prompts (PR-024) and/or the fidelity diagnostic
    // (PR-023). It runs once per chunk, pre-spawned one chunk ahead so it overlaps
    // GPU work -- the same pattern the pipeline already uses for Whisper -- and the
    // result serves both consumers.
    let ocr_needed = config.vision.ocr_grounding.enabled || config.fidelity.enabled;
    let mut ocr_by_chunk: std::collections::HashMap<u32, Vec<OcrFrame>> =
        std::collections::HashMap::new();
    let mut pending_ocr: Option<(u32, tokio::task::JoinHandle<Result<Vec<OcrFrame>, VttError>>)> =
        None;
    let frames_of = |a: &crate::ChunkArtifacts| -> Vec<(f64, std::path::PathBuf)> {
        a.frames.iter().map(|f| (f.timestamp, f.path.clone())).collect()
    };

    let mut all_chunk_segments = Vec::new();
    let mut prev_context: Option<String> = None;
    let mut pending_whisper: Option<
        tokio::task::JoinHandle<Result<Vec<Segment>, VttError>>,
    > = None;

    for (i, artifact) in manifest.chunks.iter().enumerate() {
        check_cancelled()?;
        let chunk_start = Instant::now();
        let ci = artifact.chunk.index;

        if ocr_needed {
            let t = Instant::now();
            let this = match pending_ocr.take() {
                Some((idx, handle)) if idx == ci => handle
                    .await
                    .map_err(|e| VttError::Config(format!("ocr task panicked: {e}")))??,
                other => {
                    if let Some((_, h)) = other {
                        h.abort();
                    }
                    run_ocr(&config.ocr, &frames_of(artifact)).await?
                }
            };
            let items: usize = this.iter().map(|f| f.items.len()).sum();
            eprintln!(
                "[ocr] chunk_{ci}: {} frame(s), {items} text region(s) ({:.1}s)",
                this.len(),
                t.elapsed().as_secs_f64()
            );
            ocr_by_chunk.insert(ci, this);
            if let Some(next) = manifest.chunks.get(i + 1) {
                let cfg = config.ocr.clone();
                let nf = frames_of(next);
                let nidx = next.chunk.index;
                pending_ocr = Some((nidx, tokio::spawn(async move { run_ocr(&cfg, &nf).await })));
            }
        }

        let chunk_segments = if let Some(segments) = cached.get(&artifact.chunk.index) {
            eprintln!("[timing] chunk_{ci}: loaded from checkpoint");
            // Build context from cached segments for next chunk
            let whisper_segs: Vec<_> = segments
                .iter()
                .filter(|s| s.segment_type == SegmentType::Speech)
                .cloned()
                .collect();
            let vision_segs: Vec<_> = segments
                .iter()
                .filter(|s| s.segment_type == SegmentType::Visual)
                .cloned()
                .collect();
            prev_context = Some(build_chunk_context(&whisper_segs, &vision_segs, config.vision.use_transcript));
            segments.clone()
        } else {
            let model = whisper_model.as_ref().unwrap();
            let client = ollama_client.as_ref().unwrap();

            // Step 1: Get Whisper result — from pre-spawned task or run now
            let t = Instant::now();
            let whisper_segments = if let Some(handle) = pending_whisper.take() {
                handle
                    .await
                    .map_err(|e| VttError::Whisper(format!("whisper task panicked: {e}")))?
                    ?
            } else {
                transcribe_chunk(
                    Arc::clone(model),
                    artifact.audio_path.clone(),
                    artifact.chunk.clone(),
                )
                .await?
            };
            eprintln!("[timing] chunk_{ci} whisper: {:.1}s ({} segments)", t.elapsed().as_secs_f64(), whisper_segments.len());

            check_cancelled()?;

            // Vision now receives the segments themselves so it can window the
            // transcript per batch (PR-018); a pre-flattened string cannot be
            // windowed and forced every batch to see the whole chunk.

            // Step 3: Pre-spawn Whisper for next chunk while Vision runs
            if let Some(next) = manifest.chunks.get(i + 1) {
                if !cached.contains_key(&next.chunk.index) {
                    let next_model = Arc::clone(model);
                    let next_audio = next.audio_path.clone();
                    let next_chunk = next.chunk.clone();
                    pending_whisper = Some(tokio::spawn(async move {
                        transcribe_chunk(next_model, next_audio, next_chunk).await
                    }));
                }
            }

            // Step 4: Vision description (GPU, via Ollama HTTP) with cross-chunk context
            let t = Instant::now();
            let vision_segments = client
                .describe_chunk(
                    &artifact.chunk,
                    &artifact.frames,
                    ocr_by_chunk.get(&ci).map(|v| v.as_slice()).unwrap_or(&[]),
                    &whisper_segments,
                    prev_context.as_deref(),
                )
                .await?;
            let n_batches = (artifact.frames.len() + config.vision.max_frames_per_request as usize - 1) / config.vision.max_frames_per_request as usize;
            eprintln!("[timing] chunk_{ci} vision: {:.1}s ({} frames, {} batches, {} segments)", t.elapsed().as_secs_f64(), artifact.frames.len(), n_batches, vision_segments.len());

            check_cancelled()?;

            // Build context for next chunk
            prev_context = Some(build_chunk_context(&whisper_segments, &vision_segments, config.vision.use_transcript));

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

        eprintln!("[timing] chunk_{ci} total: {:.1}s", chunk_start.elapsed().as_secs_f64());
        all_chunk_segments.push(chunk_segments);
    }

    let mut timeline = merge_segments(&source, manifest.duration_seconds, all_chunk_segments);
    timeline.capture = Some(capture_info(config));
    eprintln!("[timing] pipeline total: {:.1}s", pipeline_start.elapsed().as_secs_f64());

    // Post-hoc repetition diagnostic over the finished speech track.
    //
    // Whisper's in-decoder thresholds are retry triggers that accept unconditionally
    // at the final temperature, so repetition can reach the output ungated -- and
    // unlike vision, whisper output has no `truncate_repetition` equivalent. This
    // FLAGS suspect segments so a long unattended run surfaces them; it never edits
    // them (Segments Are Immutable After Merge, plus a real false-positive surface).
    let flags = repetition_report(
        &timeline.segments,
        config.whisper.repetition_report_thold,
        config.whisper.repetition_window_secs,
    );
    if !flags.is_empty() {
        eprintln!(
            "[repetition] {} window(s) of {:.0}s exceed compression ratio {:.1} -- possible \
             hallucinated repetition (NOT filtered; legitimate repetition also scores high):",
            flags.len(),
            config.whisper.repetition_window_secs,
            config.whisper.repetition_report_thold
        );
        for f in flags.iter().take(10) {
            eprintln!("[repetition]   {} -> {}  ratio {:.2} ({} segments)", f.start, f.end, f.ratio, f.segments);
        }
        if flags.len() > 10 {
            eprintln!("[repetition]   ... and {} more", flags.len() - 10);
        }
    }

    // Visual fidelity diagnostic (PR-023): OCR the kept frames, check each visual
    // segment's stated facts against them, keep thumbnails beside the results so
    // the check stays reproducible after the job dir is cleaned. Diagnoses only;
    // a failure here is logged and never fails the job.
    if config.fidelity.enabled {
        let t = Instant::now();
        // Run in its own task: a panic in the diagnostic becomes a JoinError here
        // instead of killing the job after the GPU work is done (found in PR-023
        // validation, where a tokenizer panic stranded a finished job).
        let mut kept: Vec<(f64, std::path::PathBuf)> = Vec::new();
        let mut ocr: Vec<OcrFrame> = Vec::new();
        for c in &manifest.chunks {
            match ocr_by_chunk.get(&c.chunk.index) {
                Some(done) => ocr.extend(done.iter().cloned()),
                // Only reachable if a chunk was never visited (all-cached job).
                None => kept.extend(frames_of(c)),
            }
        }
        let (cfg, job, segs) = (config.clone(), job_id.to_string(), timeline.segments.clone());
        let joined =
            tokio::spawn(async move { run_fidelity(&cfg, kept, ocr, &job, &segs).await }).await;
        let outcome = match joined {
            Ok(r) => r,
            Err(e) => Err(VttError::Config(format!("diagnostic task panicked: {e}"))),
        };
        match outcome {
            Ok(report) => {
                let s = &report.summary;
                eprintln!(
                    "[fidelity] {} visual segments: precision {:.3} ({}/{} stated facts on screen), \
                     recall {:.3} ({}/{} prominent on-screen facts mentioned), F0.5 {:.3}, reference={}{} ({:.1}s)",
                    s.segments, s.precision, s.supported, s.stated, s.recall, s.mentioned, s.prominent, s.f05,
                    s.reference,
                    if s.ocr_grounded {
                        " -- NOT INDEPENDENT: the prompt was grounded on this same OCR"
                    } else {
                        ""
                    },
                    t.elapsed().as_secs_f64()
                );
                timeline.fidelity = Some(report.summary.clone());
            }
            Err(e) => eprintln!("[fidelity] diagnostic failed (job output unaffected): {e}"),
        }
    }

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
        capture: None,
        fidelity: None,
    }
}

/// Run the fidelity diagnostic for a finished job: thumbnails, OCR of every kept
/// frame, scoring, and `fidelity.json` in the job's results dir.
async fn run_fidelity(
    config: &ServerConfig,
    missing: Vec<(f64, std::path::PathBuf)>,
    mut ocr: Vec<OcrFrame>,
    job_id: &str,
    segments: &[Segment],
) -> Result<FidelityReport, VttError> {
    let results_dir = Path::new(&config.processing.results_dir).join(job_id);
    let all: Vec<(f64, std::path::PathBuf)> = ocr
        .iter()
        .map(|f| (f.timestamp, std::path::PathBuf::from(&f.path)))
        .chain(missing.iter().cloned())
        .collect();
    write_thumbnails(
        &config.ffmpeg.path,
        &all,
        &results_dir.join("frames"),
        config.fidelity.thumbnail_width,
        config.fidelity.thumbnail_quality,
    )
    .await?;
    // Chunks loaded from checkpoint were never visited in the loop, so they have
    // no OCR yet; everything else is reused rather than re-run.
    if !missing.is_empty() {
        ocr.extend(run_ocr(&config.ocr, &missing).await?);
        ocr.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap_or(std::cmp::Ordering::Equal));
    }
    // Persist the kept-frame OCR facts so the report can be re-scored offline
    // (different tolerance, or a candidates reference) without re-running OCR.
    tokio::fs::write(results_dir.join("ocr.json"), serde_json::to_string(&ocr)?).await?;
    let mut report = score_segments(segments, &ocr, None, &config.fidelity);
    report.summary.ocr_grounded = config.vision.ocr_grounding.enabled;
    let json = serde_json::to_string_pretty(&report)?;
    tokio::fs::write(results_dir.join("fidelity.json"), json).await?;
    Ok(report)
}

/// Capture provenance recorded on every timeline (PR-022): the parameters that
/// determined which frames the model saw and how it was prompted.
pub fn capture_info(config: &ServerConfig) -> CaptureInfo {
    let a = &config.vision.adaptive;
    let whisper_model = std::path::Path::new(&config.whisper.model_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| config.whisper.model_path.clone());
    let (prompt_id, prompt_hash) = crate::vision::prompt_provenance(&config.ollama);
    let transcript_window = match config.vision.transcript_window {
        TranscriptWindow::Full => "full",
        TranscriptWindow::Concurrent => "concurrent",
        TranscriptWindow::Causal => "causal",
    }
    .to_string();
    CaptureInfo {
        vision_model: config.ollama.model.clone(),
        whisper_model,
        chunk_duration_secs: config.ffmpeg.chunk_duration_secs,
        fps: config.vision.fps,
        sampling: if a.enabled { SamplingMode::Adaptive } else { SamplingMode::Fixed },
        scene_threshold: a.enabled.then_some(a.scene_threshold),
        max_gap_secs: a.enabled.then_some(a.max_gap_secs),
        min_trigger_interval_secs: a.enabled.then_some(a.min_trigger_interval_secs),
        max_frames_per_chunk: a.enabled.then_some(a.max_frames_per_chunk),
        max_frames_per_request: config.vision.max_frames_per_request,
        use_transcript: config.vision.use_transcript,
        transcript_window,
        temperature: config.ollama.temperature,
        vision_prompt: Some(prompt_id),
        vision_prompt_sha256: prompt_hash,
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

/// Build a context summary from a chunk's segments for the next chunk's vision prompt.
/// Includes the last ~30s of speech and the last visual description.
pub fn build_chunk_context(
    whisper_segments: &[Segment],
    vision_segments: &[Segment],
    include_dialogue: bool,
) -> String {
    let mut parts = Vec::new();

    // Last few speech segments (roughly last 30s worth). Skipped when the
    // visual track is meant to be an independent observation.
    let speech: Vec<&str> = if !include_dialogue { Vec::new() } else { whisper_segments
        .iter()
        .filter(|s| s.segment_type == SegmentType::Speech && !s.content.trim().is_empty())
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|s| s.content.as_str())
        .collect() };

    if !speech.is_empty() {
        parts.push(format!("Recent dialogue: {}", speech.join(" ")));
    }

    // Last visual description (truncated)
    if let Some(last_visual) = vision_segments.last() {
        let desc = if last_visual.content.len() > 200 {
            // `len()` and slicing are both byte-based, so a cap of 200 can land
            // inside a multi-byte char and panic. Walk back to a char boundary.
            let mut end = 200;
            while !last_visual.content.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &last_visual.content[..end])
        } else {
            last_visual.content.clone()
        };
        parts.push(format!("Last visual: {desc}"));
    }

    parts.join(" ")
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
        frames: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the visual track meant as an independent observation, the
    /// cross-chunk context must not smuggle speech back in via "Recent
    /// dialogue" -- but visual continuity should survive.
    #[test]
    fn test_build_chunk_context_can_exclude_dialogue() {
        let speech = vec![make_segment(SegmentType::Speech, 0.0, 5.0, "spoken words here")];
        let vision = vec![make_segment(SegmentType::Visual, 0.0, 7.5, "a chart on screen")];

        let with = build_chunk_context(&speech, &vision, true);
        assert!(with.contains("Recent dialogue"));
        assert!(with.contains("spoken words here"));

        let without = build_chunk_context(&speech, &vision, false);
        assert!(!without.contains("Recent dialogue"), "speech must not leak in");
        assert!(!without.contains("spoken words here"));
        assert!(without.contains("Last visual"), "visual continuity must survive");
        assert!(without.contains("a chart on screen"));
    }

    // --- build_chunk_context UTF-8 regression ---

    /// Regression for a panic seen in production:
    ///   "byte index 200 is not a char boundary; it is inside '\u{201D}'
    ///    (bytes 198..201)"
    /// The chart title `\u{201C}SPDR S&P 500 ETF TRUST - 1W - Arca,\u{201D}` placed a 3-byte
    /// curly quote across byte 200, and `&content[..200]` sliced into it.
    #[test]
    fn test_build_chunk_context_multibyte_at_truncation_boundary() {
        let mut content = String::new();
        while content.len() < 198 {
            content.push('a');
        }
        content.push('\u{201D}'); // 3 bytes, occupying 198..201
        content.push_str(" trailing text so the value exceeds the 200 byte cap");

        assert!(content.len() > 200, "setup: must exceed the truncation cap");
        assert!(
            !content.is_char_boundary(200),
            "setup: byte 200 must land inside a multi-byte char"
        );

        let vision = vec![make_segment(SegmentType::Visual, 0.0, 7.5, &content)];
        let speech = vec![make_segment(SegmentType::Speech, 0.0, 5.0, "hello there")];

        let ctx = build_chunk_context(&speech, &vision, true);
        assert!(ctx.contains("Last visual:"));
        assert!(ctx.ends_with("..."), "long descriptions should be truncated");
    }

    /// Truncation must not split a char for any cap position in a dense
    /// multi-byte string.
    #[test]
    fn test_build_chunk_context_all_multibyte_never_panics() {
        let content: String = std::iter::repeat('\u{4e16}').take(300).collect(); // 3 bytes each
        let vision = vec![make_segment(SegmentType::Visual, 0.0, 7.5, &content)];
        let ctx = build_chunk_context(&[], &vision, true);
        assert!(ctx.contains("Last visual:"));
    }

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

    // --- build_chunk_context tests ---

    #[test]
    fn test_build_chunk_context_with_speech_and_visual() {
        let whisper = vec![
            make_segment(SegmentType::Speech, 0.0, 5.0, "Hello world"),
            make_segment(SegmentType::Speech, 5.0, 10.0, "How are you"),
        ];
        let vision = vec![
            make_segment(SegmentType::Visual, 0.0, 10.0, "A man stands in a park"),
        ];
        let ctx = build_chunk_context(&whisper, &vision, true);
        assert!(ctx.contains("Hello world"));
        assert!(ctx.contains("How are you"));
        assert!(ctx.contains("A man stands in a park"));
    }

    #[test]
    fn test_build_chunk_context_empty() {
        let ctx = build_chunk_context(&[], &[], true);
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_build_chunk_context_truncates_long_visual() {
        let vision = vec![
            make_segment(SegmentType::Visual, 0.0, 10.0, &"x".repeat(500)),
        ];
        let ctx = build_chunk_context(&[], &vision, true);
        assert!(ctx.len() < 300);
        assert!(ctx.contains("..."));
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

        let timeline = process_video(&config, &video_path, "test-job", false, None, None).await.unwrap();

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
    /// PR-022: provenance must mirror the config that produced the timeline, and
    /// adaptive parameters must be absent (not zero) in fixed mode.
    #[test]
    fn test_capture_info_mirrors_config() {
        let mut config = ServerConfig::default();
        config.whisper.model_path = "/home/rux/models/ggml-large-v3-turbo.bin".to_string();
        let fixed = capture_info(&config);
        assert_eq!(fixed.sampling, SamplingMode::Fixed);
        assert!(fixed.scene_threshold.is_none() && fixed.max_frames_per_chunk.is_none());
        assert_eq!(fixed.whisper_model, "ggml-large-v3-turbo.bin");
        assert_eq!(fixed.transcript_window, "full");
        assert_eq!(fixed.fps, 2.0);

        config.vision.adaptive.enabled = true;
        config.vision.transcript_window = TranscriptWindow::Causal;
        config.vision.use_transcript = false;
        let adaptive = capture_info(&config);
        assert_eq!(adaptive.sampling, SamplingMode::Adaptive);
        assert_eq!(adaptive.scene_threshold, Some(0.08));
        assert_eq!(adaptive.max_gap_secs, Some(15.0));
        assert_eq!(adaptive.max_frames_per_chunk, Some(45));
        assert_eq!(adaptive.transcript_window, "causal");
        assert!(!adaptive.use_transcript);
    }

    /// PR-026: prompt provenance travels with the timeline, so two captures made
    /// under different prompts are distinguishable in the data. The 2026-08 corpus
    /// lost 36 timelines to exactly this gap for a different parameter.
    #[test]
    fn test_capture_info_records_prompt_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vision-chart.txt");
        std::fs::write(&path, "chart prompt").unwrap();
        let mut config = ServerConfig::default();
        config.ollama.prompt_template_path = Some(path.display().to_string());

        let a = capture_info(&config);
        assert_eq!(a.vision_prompt.as_deref(), Some(path.display().to_string().as_str()));
        let hash_a = a.vision_prompt_sha256.clone().expect("hash present");

        // Same config, different prompt CONTENT -> different provenance.
        std::fs::write(&path, "a different chart prompt").unwrap();
        let b = capture_info(&config);
        assert_ne!(hash_a, b.vision_prompt_sha256.unwrap(), "content change must be visible");
    }
}
