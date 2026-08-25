use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The type of content a segment represents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentType {
    Speech,
    Visual,
    Sound,
}

/// A single output entry in the timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    #[serde(rename = "type")]
    pub segment_type: SegmentType,
    pub start: String,
    pub end: String,
    pub content: String,
    /// Capture timestamps (HH:MM:SS.mmm) of the frames a visual segment was
    /// generated from. Empty for speech/sound segments and omitted from JSON when
    /// empty, so older timelines and checkpoints deserialise unchanged. Under
    /// content-adaptive sampling this is the only record of which instants the
    /// model actually saw. (PR-022)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<String>,
}

/// A time-bounded segment of a video, used as the unit of processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub index: u32,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

/// A processing request submitted to the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub source: String,
    pub status: JobStatus,
}

/// The current state of a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

/// The final merged output for a video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub source: String,
    pub duration_seconds: f64,
    pub segments: Vec<Segment>,
    /// How the visual track was captured. Fixed-fps output is reconstructible
    /// from segment spans; adaptive output is data-dependent and is not, so the
    /// parameters travel with the data. Omitted when absent. (PR-022)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<CaptureInfo>,
    /// Visual fidelity diagnostic summary (PR-023); the per-segment detail is in
    /// `fidelity.json` beside the results. Omitted when the diagnostic is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fidelity: Option<FidelitySummary>,
}

/// Corpus-level summary of the visual fidelity diagnostic. (PR-023)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FidelitySummary {
    /// "kept" or "candidates" -- what recall was scored against.
    pub reference: String,
    pub segments: usize,
    pub stated: usize,
    pub supported: usize,
    pub prominent: usize,
    pub mentioned: usize,
    pub precision: f64,
    pub recall: f64,
    pub f05: f64,
    /// True when the vision prompt was grounded on this same OCR (PR-024). The
    /// precision figure is then circular and is NOT evidence of accuracy.
    #[serde(default)]
    pub ocr_grounded: bool,
}

/// Frame-selection mode recorded in `CaptureInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SamplingMode {
    Fixed,
    Adaptive,
}

/// Capture provenance for a timeline: the parameters that determine which
/// frames the vision model saw and how it was prompted. (PR-022)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureInfo {
    pub vision_model: String,
    pub whisper_model: String,
    pub chunk_duration_secs: u32,
    /// Candidate rate in adaptive mode; sample rate in fixed mode.
    pub fps: f32,
    pub sampling: SamplingMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_gap_secs: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_trigger_interval_secs: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_frames_per_chunk: Option<u32>,
    pub max_frames_per_request: u32,
    pub use_transcript: bool,
    pub transcript_window: String,
    pub temperature: f32,
}

/// Format seconds into HH:MM:SS.mmm timestamp string.
pub fn format_timestamp(seconds: f64) -> String {
    let total_ms = (seconds * 1000.0).round() as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let secs = (total_ms % 60_000) / 1_000;
    let ms = total_ms % 1_000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, secs, ms)
}

/// Parse a HH:MM:SS.mmm timestamp string into seconds.
pub fn parse_timestamp(ts: &str) -> Option<f64> {
    let parts: Vec<&str> = ts.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: f64 = parts[0].parse().ok()?;
    let minutes: f64 = parts[1].parse().ok()?;
    let sec_parts: Vec<&str> = parts[2].split('.').collect();
    let secs: f64 = sec_parts[0].parse().ok()?;
    let ms: f64 = if sec_parts.len() > 1 {
        let ms_str = sec_parts[1];
        let ms_val: f64 = ms_str.parse().ok()?;
        ms_val / 10_f64.powi(ms_str.len() as i32)
    } else {
        0.0
    };
    Some(hours * 3600.0 + minutes * 60.0 + secs + ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0.0), "00:00:00.000");
        assert_eq!(format_timestamp(72.4), "00:01:12.400");
        assert_eq!(format_timestamp(3661.5), "01:01:01.500");
    }

    #[test]
    fn test_parse_timestamp() {
        assert_eq!(parse_timestamp("00:00:00.000"), Some(0.0));
        assert_eq!(parse_timestamp("00:01:12.400"), Some(72.4));
        assert_eq!(parse_timestamp("01:01:01.500"), Some(3661.5));
        assert_eq!(parse_timestamp("invalid"), None);
    }

    #[test]
    fn test_timestamp_roundtrip() {
        let values = [0.0, 1.234, 72.4, 3661.5, 7200.0];
        for val in values {
            let formatted = format_timestamp(val);
            let parsed = parse_timestamp(&formatted).unwrap();
            assert!((val - parsed).abs() < 0.001, "roundtrip failed for {val}");
        }
    }

    #[test]
    fn test_segment_json_roundtrip() {
        let segment = Segment {
            segment_type: SegmentType::Speech,
            start: "00:01:12.400".to_string(),
            end: "00:01:15.800".to_string(),
            content: "Have you seen this before?".to_string(),
            frames: Vec::new(),
        };
        let json = serde_json::to_string(&segment).unwrap();
        let deserialized: Segment = serde_json::from_str(&json).unwrap();
        assert_eq!(segment, deserialized);
    }

    /// PR-022: `frames` is provenance for visual segments. It must not appear in
    /// JSON when empty (speech/sound, and every pre-PR-022 timeline), and JSON
    /// written before the field existed must still deserialise.
    #[test]
    fn test_segment_frames_omitted_when_empty_and_legacy_json_loads() {
        let speech = Segment {
            segment_type: SegmentType::Speech,
            start: "00:00:00.000".to_string(),
            end: "00:00:01.000".to_string(),
            content: "hi".to_string(),
            frames: Vec::new(),
        };
        let json = serde_json::to_string(&speech).unwrap();
        assert!(!json.contains("frames"), "empty frames must be omitted: {json}");

        let legacy = r#"{"type":"visual","start":"00:00:00.000","end":"00:00:07.500","content":"x"}"#;
        let seg: Segment = serde_json::from_str(legacy).unwrap();
        assert!(seg.frames.is_empty());

        let visual = Segment {
            frames: vec!["00:00:00.000".to_string(), "00:00:12.500".to_string()],
            ..seg
        };
        let json = serde_json::to_string(&visual).unwrap();
        let back: Segment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.frames.len(), 2);
        assert_eq!(back.frames[1], "00:00:12.500");
    }

    /// PR-022: capture provenance is optional on the wire in both directions.
    #[test]
    fn test_timeline_capture_optional_roundtrip() {
        let legacy = r#"{"source":"v.mp4","duration_seconds":10.0,"segments":[]}"#;
        let t: Timeline = serde_json::from_str(legacy).unwrap();
        assert!(t.capture.is_none());
        assert!(!serde_json::to_string(&t).unwrap().contains("capture"));

        let with = Timeline {
            capture: Some(CaptureInfo {
                vision_model: "qwen3-vl:8b-instruct-q8_0".to_string(),
                whisper_model: "ggml-large-v3-turbo.bin".to_string(),
                chunk_duration_secs: 180,
                fps: 2.0,
                sampling: SamplingMode::Adaptive,
                scene_threshold: Some(0.08),
                max_gap_secs: Some(15.0),
                min_trigger_interval_secs: Some(2.0),
                max_frames_per_chunk: Some(45),
                max_frames_per_request: 15,
                use_transcript: false,
                transcript_window: "causal".to_string(),
                temperature: 0.0,
            }),
            ..t
        };
        let json = serde_json::to_string(&with).unwrap();
        assert!(json.contains("\"sampling\":\"adaptive\""), "{json}");
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(back, with);
    }

    #[test]
    fn test_segment_type_serialization() {
        assert_eq!(serde_json::to_string(&SegmentType::Speech).unwrap(), "\"speech\"");
        assert_eq!(serde_json::to_string(&SegmentType::Visual).unwrap(), "\"visual\"");
        assert_eq!(serde_json::to_string(&SegmentType::Sound).unwrap(), "\"sound\"");
    }

    #[test]
    fn test_chunk_json_roundtrip() {
        let chunk = Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 180.0,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: Chunk = serde_json::from_str(&json).unwrap();
        assert_eq!(chunk, deserialized);
    }

    #[test]
    fn test_job_json_roundtrip() {
        let job = Job {
            id: Uuid::new_v4(),
            source: "video.mp4".to_string(),
            status: JobStatus::Queued,
        };
        let json = serde_json::to_string(&job).unwrap();
        let deserialized: Job = serde_json::from_str(&json).unwrap();
        assert_eq!(job, deserialized);
    }

    #[test]
    fn test_job_status_serialization() {
        assert_eq!(serde_json::to_string(&JobStatus::Queued).unwrap(), "\"queued\"");
        assert_eq!(serde_json::to_string(&JobStatus::Processing).unwrap(), "\"processing\"");
        assert_eq!(serde_json::to_string(&JobStatus::Completed).unwrap(), "\"completed\"");
        assert_eq!(serde_json::to_string(&JobStatus::Failed).unwrap(), "\"failed\"");
    }

    #[test]
    fn test_timeline_json_roundtrip() {
        let timeline = Timeline {
            source: "video.mp4".to_string(),
            duration_seconds: 3600.0,
            segments: vec![
                Segment {
                    segment_type: SegmentType::Speech,
                    start: "00:01:12.400".to_string(),
                    end: "00:01:15.800".to_string(),
                    content: "Have you seen this before?".to_string(),
                    frames: Vec::new(),
                },
                Segment {
                    segment_type: SegmentType::Visual,
                    start: "00:01:12.000".to_string(),
                    end: "00:01:18.000".to_string(),
                    content: "A woman turns toward the camera in a dimly lit hallway.".to_string(),
                    frames: Vec::new(),
                },
                Segment {
                    segment_type: SegmentType::Sound,
                    start: "00:01:14.000".to_string(),
                    end: "00:01:16.500".to_string(),
                    content: "door creaking".to_string(),
                    frames: Vec::new(),
                },
            ],
            capture: None,
            fidelity: None,
        };
        let json = serde_json::to_string_pretty(&timeline).unwrap();
        let deserialized: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(timeline, deserialized);
    }
}
