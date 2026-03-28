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
        };
        let json = serde_json::to_string(&segment).unwrap();
        let deserialized: Segment = serde_json::from_str(&json).unwrap();
        assert_eq!(segment, deserialized);
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
                },
                Segment {
                    segment_type: SegmentType::Visual,
                    start: "00:01:12.000".to_string(),
                    end: "00:01:18.000".to_string(),
                    content: "A woman turns toward the camera in a dimly lit hallway.".to_string(),
                },
                Segment {
                    segment_type: SegmentType::Sound,
                    start: "00:01:14.000".to_string(),
                    end: "00:01:16.500".to_string(),
                    content: "door creaking".to_string(),
                },
            ],
        };
        let json = serde_json::to_string_pretty(&timeline).unwrap();
        let deserialized: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(timeline, deserialized);
    }
}
