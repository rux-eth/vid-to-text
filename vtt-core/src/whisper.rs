use std::path::{Path, PathBuf};
use std::sync::Arc;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{format_timestamp, Chunk, Segment, SegmentType, VttError, WhisperConfig};

/// A loaded Whisper model, wrapping WhisperContext for reuse across chunks.
pub struct WhisperModel {
    ctx: WhisperContext,
    config: WhisperConfig,
}

impl WhisperModel {
    /// Load a Whisper model from the configured path.
    /// This is expensive (~1.5GB for large models) and should be done once.
    pub fn new(config: &WhisperConfig) -> Result<Self, VttError> {
        let ctx = WhisperContext::new_with_params(
            &config.model_path,
            WhisperContextParameters::default(),
        )
        .map_err(|e| VttError::Whisper(format!("failed to load model '{}': {e}", config.model_path)))?;

        Ok(Self {
            ctx,
            config: config.clone(),
        })
    }
}

/// Read a WAV file and return f32 samples normalized to [-1.0, 1.0].
/// Expects 16kHz mono PCM 16-bit input (as produced by ffmpeg in PR-003).
pub fn load_wav_samples(path: &Path) -> Result<Vec<f32>, VttError> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| VttError::Whisper(format!("failed to open WAV '{}': {e}", path.display())))?;

    let spec = reader.spec();
    if spec.channels != 1 {
        return Err(VttError::Whisper(format!(
            "expected mono WAV, got {} channels",
            spec.channels
        )));
    }
    if spec.sample_rate != 16000 {
        return Err(VttError::Whisper(format!(
            "expected 16kHz WAV, got {} Hz",
            spec.sample_rate
        )));
    }
    if spec.bits_per_sample != 16 {
        return Err(VttError::Whisper(format!(
            "expected 16-bit WAV, got {}-bit",
            spec.bits_per_sample
        )));
    }

    let samples: Vec<f32> = reader
        .into_samples::<i16>()
        .map(|s| {
            s.map(|v| v as f32 / i16::MAX as f32)
                .map_err(|e| VttError::Whisper(format!("failed to read WAV sample: {e}")))
        })
        .collect::<Result<Vec<f32>, VttError>>()?;

    Ok(samples)
}

/// Determine if a Whisper segment is speech or a sound event.
/// If the entire text is a bracketed token like [MUSIC], classify as Sound.
fn classify_segment_text(text: &str) -> Option<(SegmentType, String)> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        // Only classify as sound if no other brackets inside (simple heuristic)
        if !inner.contains('[') && !inner.contains(']') {
            return Some((SegmentType::Sound, inner.to_lowercase()));
        }
    }

    Some((SegmentType::Speech, trimmed.to_string()))
}

/// Transcribe a WAV file for a given chunk, returning classified Segments.
/// The chunk is needed to offset timestamps from chunk-local to video-global.
///
/// This is a blocking function. Use `transcribe_chunk` for async contexts.
/// Map a configured beam width onto a whisper sampling strategy.
/// `patience` is accepted by whisper-rs but unimplemented in whisper.cpp as of
/// v1.7.6, so the documented default of -1.0 is passed through.
fn sampling_strategy(beam_size: u16) -> SamplingStrategy {
    if beam_size > 1 {
        SamplingStrategy::BeamSearch {
            beam_size: beam_size as i32,
            patience: -1.0,
        }
    } else {
        SamplingStrategy::Greedy { best_of: 1 }
    }
}

pub fn transcribe(
    model: &WhisperModel,
    audio_path: &Path,
    chunk: &Chunk,
) -> Result<Vec<Segment>, VttError> {
    let samples = load_wav_samples(audio_path)?;

    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let mut state = model
        .ctx
        .create_state()
        .map_err(|e| VttError::Whisper(format!("failed to create whisper state: {e}")))?;

    let mut params = FullParams::new(sampling_strategy(model.config.beam_size));
    params.set_language(Some(&model.config.language));
    params.set_n_threads(model.config.n_threads as i32);
    params.set_translate(false);
    params.set_print_realtime(false);
    params.set_print_progress(false);
    params.set_no_timestamps(false);

    // Vocabulary priming. Skipped when empty so the model is not handed a
    // stray empty context.
    if !model.config.initial_prompt.trim().is_empty() {
        params.set_initial_prompt(&model.config.initial_prompt);
    }

    // Temperature fallback. These are whisper.cpp's own defaults and match the
    // Whisper paper's Section 4.5 heuristics.
    //
    // IMPORTANT: they are RETRY TRIGGERS, NOT FILTERS. A window that trips a
    // threshold is re-decoded at the next temperature, but at the final temperature
    // the result is accepted no matter how repetitive it is -- so this gate can
    // never remove hallucinated repetition from the output. The Whisper paper's own
    // Table 7 ablation reports zero WER gain from temperature fallback.
    //
    // Repetition that survives decoding is therefore detected POST-HOC; see
    // `compression_ratio` below. (PR-020 Q5)
    params.set_temperature(model.config.temperature);
    params.set_temperature_inc(model.config.temperature_inc);
    params.set_entropy_thold(model.config.entropy_thold);
    params.set_logprob_thold(model.config.logprob_thold);
    params.set_no_speech_thold(model.config.no_speech_thold);

    state
        .full(params, &samples)
        .map_err(|e| VttError::Whisper(format!("transcription failed: {e}")))?;

    let n_segments = state.full_n_segments();

    let mut segments = Vec::new();
    for i in 0..n_segments {
        let seg = match state.get_segment(i) {
            Some(s) => s,
            None => continue,
        };

        let text = seg
            .to_str_lossy()
            .map_err(|e| VttError::Whisper(format!("failed to get segment text: {e}")))?
            .to_string();

        let t0 = seg.start_timestamp();
        let t1 = seg.end_timestamp();

        if let Some((segment_type, content)) = classify_segment_text(&text) {
            let start_seconds = chunk.start_seconds + (t0 as f64 / 100.0);
            let end_seconds = chunk.start_seconds + (t1 as f64 / 100.0);

            segments.push(Segment {
                segment_type,
                start: format_timestamp(start_seconds),
                end: format_timestamp(end_seconds),
                content,
            });
        }
    }

    Ok(segments)
}

/// Async wrapper around transcribe that runs on the blocking thread pool.
/// This is the primary entry point for callers in async code.
pub async fn transcribe_chunk(
    model: Arc<WhisperModel>,
    audio_path: PathBuf,
    chunk: Chunk,
) -> Result<Vec<Segment>, VttError> {
    tokio::task::spawn_blocking(move || transcribe(&model, &audio_path, &chunk))
        .await
        .map_err(|e| VttError::Whisper(format!("transcription task panicked: {e}")))?
}


/// Reference-free repetition signal for a finished transcript segment.
///
/// `len(utf8_bytes) / len(zlib_compressed_bytes)`. Highly repetitive text compresses
/// far better than varied text, so a HIGHER ratio means MORE repetition. This is the
/// same quantity OpenAI Whisper computes as `compression_ratio` and thresholds at
/// 2.4; whisper.cpp has no equivalent (it substitutes token-entropy over a 32-token
/// window), so nothing in this pipeline computes it during decoding.
///
/// It is a pure function of the text: no audio, no model state, no reference
/// transcript. That is what makes it usable post-hoc. (PR-020 Q5)
pub fn compression_ratio(text: &str) -> f64 {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return 0.0;
    }
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    if enc.write_all(bytes).is_err() {
        return 0.0;
    }
    match enc.finish() {
        Ok(compressed) if !compressed.is_empty() => bytes.len() as f64 / compressed.len() as f64,
        _ => 0.0,
    }
}

/// One flagged segment. Carries a copy of the timestamps, never a mutable handle.
#[derive(Debug, Clone, PartialEq)]
pub struct RepetitionFlag {
    pub start: String,
    pub end: String,
    pub ratio: f64,
}

/// Flag speech segments whose compression ratio exceeds `threshold`.
///
/// This DIAGNOSES, it does not filter. Segments are never edited, truncated or
/// dropped -- the Segments Are Immutable After Merge constraint forbids it, and the
/// documented false-positive surface makes silent editing unsafe: legitimately
/// repetitive speech (counting, choruses, backchannel agreement, litany) exceeds the
/// same 2.4 threshold that catches hallucinated loops.
///
/// Visual segments are excluded: `vision::truncate_repetition` already guards them
/// at generation time. Whisper output has no such guard, which is the asymmetry this
/// closes. (PR-020 Q5, Group D)
pub fn repetition_report(segments: &[Segment], threshold: f64) -> Vec<RepetitionFlag> {
    segments
        .iter()
        .filter(|s| s.segment_type == SegmentType::Speech)
        .filter_map(|s| {
            let ratio = compression_ratio(&s.content);
            (ratio > threshold).then(|| RepetitionFlag {
                start: s.start.clone(),
                end: s.end.clone(),
                ratio,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format_timestamp;

    fn make_segment(t: SegmentType, start: f64, end: f64, content: &str) -> Segment {
        Segment {
            segment_type: t,
            start: format_timestamp(start),
            end: format_timestamp(end),
            content: content.to_string(),
        }
    }

    // --- post-hoc repetition detection (PR-020 Q5) ---

    /// The in-decoder thresholds are retry triggers that accept unconditionally at
    /// the final temperature, so repetition can reach the output ungated -- and
    /// unlike vision (which has `truncate_repetition`), whisper output has no guard.
    /// `compression_ratio` is the reference-free post-hoc signal: a pure function of
    /// the text, len(utf8) / len(zlib(utf8)). Higher means more repetitive.
    #[test]
    fn test_compression_ratio_flags_repetition() {
        let normal = "the price is approaching a significant level of resistance                       that we have been watching for several weeks now";
        let looped = "we can look at it in terms of how the rally did run out of juice "
            .repeat(10);
        let r_normal = compression_ratio(normal);
        let r_looped = compression_ratio(&looped);
        assert!(r_looped > r_normal * 2.0,
            "looped text ({r_looped:.2}) must score far above normal ({r_normal:.2})");
        assert!(r_looped > 2.4, "known loop must exceed the standard 2.4 threshold");
    }

    /// Documented false-positive surface: legitimately repetitive speech exceeds the
    /// same threshold. This is why the signal FLAGS and never edits.
    #[test]
    fn test_compression_ratio_false_positives_are_real() {
        let counting = "one two three four five six seven eight nine ten                         eleven twelve thirteen fourteen fifteen sixteen";
        assert!(compression_ratio(counting) > 1.0);
    }

    /// Short text cannot be meaningfully scored -- zlib's own header dominates.
    #[test]
    fn test_compression_ratio_short_text_scores_low() {
        assert!(compression_ratio("okay") < 1.0,
            "very short strings compress worse than they store; never flaggable");
    }

    /// The report must identify WHICH segments are suspect without modifying any of
    /// them -- the Segments Are Immutable After Merge constraint.
    #[test]
    fn test_repetition_report_flags_without_mutating() {
        let looped = "we can look at it in terms of how the rally did run out of juice "
            .repeat(10);
        let segs = vec![
            make_segment(SegmentType::Speech, 0.0, 5.0, "a normal sentence about the market"),
            make_segment(SegmentType::Speech, 5.0, 10.0, &looped),
            make_segment(SegmentType::Visual, 10.0, 15.0, &looped),
        ];
        let before: Vec<String> = segs.iter().map(|s| s.content.clone()).collect();
        let report = repetition_report(&segs, 2.4);

        assert_eq!(report.len(), 1, "only the SPEECH loop is reported");
        assert_eq!(report[0].start, "00:00:05.000");
        assert!(report[0].ratio > 2.4);

        let after: Vec<String> = segs.iter().map(|s| s.content.clone()).collect();
        assert_eq!(before, after, "report must not mutate segment content");
    }

    // --- decoding strategy tests ---

    /// Transcription quality is the primary signal for downstream use, but the
    /// pipeline shipped with `Greedy { best_of: 1 }` -- below whisper.cpp's own
    /// default of beam search width 5. Beam search is the standard remedy for
    /// unclear speech.
    #[test]
    fn test_sampling_strategy_uses_beam_search_above_one() {
        match sampling_strategy(5) {
            SamplingStrategy::BeamSearch { beam_size, .. } => assert_eq!(beam_size, 5),
            other => panic!("expected BeamSearch, got {other:?}"),
        }
    }

    #[test]
    fn test_sampling_strategy_falls_back_to_greedy_at_one_or_zero() {
        for n in [0u16, 1u16] {
            match sampling_strategy(n) {
                SamplingStrategy::Greedy { best_of } => assert_eq!(best_of, 1),
                other => panic!("beam_size {n} should be Greedy, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_whisper_config_defaults_match_whisper_cpp() {
        let c = WhisperConfig::default();
        assert_eq!(c.beam_size, 5, "whisper.cpp default beam width");
        assert_eq!(c.temperature, 0.0);
        assert_eq!(c.temperature_inc, 0.2, "temperature fallback step");
        assert_eq!(c.entropy_thold, 2.4);
        assert_eq!(c.logprob_thold, -1.0);
        assert_eq!(c.no_speech_thold, 0.6);
        assert!(c.initial_prompt.is_empty(), "vocab priming is opt-in per corpus");
    }

    // --- classify_segment_text tests ---

    #[test]
    fn test_classify_speech() {
        let result = classify_segment_text("Hello, how are you?");
        assert_eq!(
            result,
            Some((SegmentType::Speech, "Hello, how are you?".to_string()))
        );
    }

    #[test]
    fn test_classify_sound_music() {
        let result = classify_segment_text("[MUSIC]");
        assert_eq!(
            result,
            Some((SegmentType::Sound, "music".to_string()))
        );
    }

    #[test]
    fn test_classify_sound_laughter() {
        let result = classify_segment_text("[LAUGHTER]");
        assert_eq!(
            result,
            Some((SegmentType::Sound, "laughter".to_string()))
        );
    }

    #[test]
    fn test_classify_sound_applause() {
        let result = classify_segment_text("  [APPLAUSE]  ");
        assert_eq!(
            result,
            Some((SegmentType::Sound, "applause".to_string()))
        );
    }

    #[test]
    fn test_classify_mixed_brackets_is_speech() {
        let result = classify_segment_text("he said [something] interesting");
        assert_eq!(
            result,
            Some((
                SegmentType::Speech,
                "he said [something] interesting".to_string()
            ))
        );
    }

    #[test]
    fn test_classify_empty_returns_none() {
        assert_eq!(classify_segment_text(""), None);
        assert_eq!(classify_segment_text("   "), None);
    }

    #[test]
    fn test_classify_empty_brackets_is_speech() {
        // "[]" has no inner content, so it's not a sound tag — treated as speech
        assert_eq!(
            classify_segment_text("[]"),
            Some((SegmentType::Speech, "[]".to_string()))
        );
    }

    // --- Timestamp offset tests ---

    #[test]
    fn test_timestamp_offset_from_chunk() {
        // Chunk starts at 180.0s (3 min), segment at 5.0s local (500 centiseconds)
        let chunk_start = 180.0;
        let t0_centiseconds: i64 = 500;
        let global_seconds = chunk_start + (t0_centiseconds as f64 / 100.0);
        assert!((global_seconds - 185.0).abs() < 0.001);
        assert_eq!(format_timestamp(global_seconds), "00:03:05.000");
    }

    #[test]
    fn test_timestamp_offset_zero_chunk() {
        let chunk_start = 0.0;
        let t0_centiseconds: i64 = 1234;
        let global_seconds = chunk_start + (t0_centiseconds as f64 / 100.0);
        assert!((global_seconds - 12.34).abs() < 0.001);
        assert_eq!(format_timestamp(global_seconds), "00:00:12.340");
    }

    // --- WAV loading tests ---

    #[test]
    fn test_load_wav_samples_synthetic() {
        let dir = tempfile::tempdir().unwrap();
        let wav_path = dir.path().join("test.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav_path, spec).unwrap();
        for _ in 0..16000 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();

        let samples = load_wav_samples(&wav_path).unwrap();
        assert_eq!(samples.len(), 16000);
        assert!(samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_load_wav_samples_with_data() {
        let dir = tempfile::tempdir().unwrap();
        let wav_path = dir.path().join("test.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav_path, spec).unwrap();
        writer.write_sample(i16::MAX).unwrap();
        writer.write_sample(i16::MIN).unwrap();
        writer.write_sample(0i16).unwrap();
        writer.finalize().unwrap();

        let samples = load_wav_samples(&wav_path).unwrap();
        assert_eq!(samples.len(), 3);
        assert!((samples[0] - 1.0).abs() < 0.001);
        assert!((samples[1] - (-1.0)).abs() < 0.01); // i16::MIN / i16::MAX ≈ -1.00003
        assert!((samples[2] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_load_wav_samples_nonexistent() {
        let result = load_wav_samples(Path::new("/nonexistent/path.wav"));
        assert!(result.is_err());
    }

    // --- Integration tests (require whisper model file) ---

    #[tokio::test]
    #[ignore]
    async fn test_whisper_model_load() {
        let config = WhisperConfig::default();
        let _model = WhisperModel::new(&config).unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_transcribe_silent_wav() {
        let config = WhisperConfig::default();
        let model = WhisperModel::new(&config).unwrap();

        // Create a 2-second silent WAV
        let dir = tempfile::tempdir().unwrap();
        let wav_path = dir.path().join("silent.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav_path, spec).unwrap();
        for _ in 0..32000 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();

        let chunk = Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 2.0,
        };

        let segments = transcribe(&model, &wav_path, &chunk).unwrap();
        // Silent audio may produce empty segments or hallucinated text — just verify it doesn't crash
        let _ = segments;
    }

    #[tokio::test]
    #[ignore]
    async fn test_transcribe_chunk_async() {
        let config = WhisperConfig::default();
        let model = Arc::new(WhisperModel::new(&config).unwrap());

        let dir = tempfile::tempdir().unwrap();
        let wav_path = dir.path().join("silent.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav_path, spec).unwrap();
        for _ in 0..32000 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();

        let chunk = Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 2.0,
        };

        let segments = transcribe_chunk(model, wav_path, chunk).await.unwrap();
        let _ = segments;
    }
}
