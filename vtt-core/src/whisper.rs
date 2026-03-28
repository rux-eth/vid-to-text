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

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(&model.config.language));
    params.set_n_threads(model.config.n_threads as i32);
    params.set_translate(false);
    params.set_print_realtime(false);
    params.set_print_progress(false);
    params.set_no_timestamps(false);

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

#[cfg(test)]
mod tests {
    use super::*;

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
