use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::VttError;

// --- Client Config ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    pub server: ClientServerConfig,
    pub output: ClientOutputConfig,
    pub polling: ClientPollingConfig,
    pub openai: OpenAIConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientOutputConfig {
    pub dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientPollingConfig {
    pub poll_interval_secs: u64,
    pub timeout_secs: u64,
}

impl Default for ClientPollingConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 3,
            timeout_secs: 1800,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenAIConfig {
    pub model: String,
    pub endpoint: String,
    pub max_tokens: u32,
    pub format_prompt_path: Option<String>,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            model: "gpt-5.4".to_string(),
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            max_tokens: 4096,
            format_prompt_path: Some("prompts/format.txt".to_string()),
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server: ClientServerConfig::default(),
            output: ClientOutputConfig::default(),
            polling: ClientPollingConfig::default(),
            openai: OpenAIConfig::default(),
        }
    }
}

impl Default for ClientServerConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 3001,
        }
    }
}

impl Default for ClientOutputConfig {
    fn default() -> Self {
        Self { dir: None }
    }
}

impl ClientConfig {
    pub fn validate(&self) -> Result<(), VttError> {
        if self.server.host.is_empty() {
            return Err(VttError::Config("server.host must not be empty".into()));
        }
        if self.server.port == 0 {
            return Err(VttError::Config("server.port must be greater than 0".into()));
        }
        if self.polling.poll_interval_secs == 0 {
            return Err(VttError::Config(
                "polling.poll_interval_secs must be greater than 0".into(),
            ));
        }
        if self.polling.timeout_secs == 0 {
            return Err(VttError::Config(
                "polling.timeout_secs must be greater than 0".into(),
            ));
        }
        if self.openai.model.is_empty() {
            return Err(VttError::Config("openai.model must not be empty".into()));
        }
        if self.openai.endpoint.is_empty() {
            return Err(VttError::Config("openai.endpoint must not be empty".into()));
        }
        if self.openai.max_tokens == 0 {
            return Err(VttError::Config(
                "openai.max_tokens must be greater than 0".into(),
            ));
        }
        Ok(())
    }

    pub fn server_url(&self) -> String {
        format!("http://{}:{}", self.server.host, self.server.port)
    }
}

// --- Server Config ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub server: ServerListenConfig,
    pub ffmpeg: FfmpegConfig,
    pub whisper: WhisperConfig,
    pub ollama: OllamaConfig,
    pub vision: VisionConfig,
    pub processing: ProcessingConfig,
    pub ytdlp: YtDlpConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerListenConfig {
    pub listen_address: String,
    pub listen_port: u16,
    pub max_concurrent_jobs: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FfmpegConfig {
    pub path: String,
    pub chunk_duration_secs: u32,
    pub frame_format: String,
    pub frame_quality: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhisperConfig {
    pub model_path: String,
    pub model_size: String,
    pub language: String,
    pub n_threads: u16,
    /// Beam width. >1 selects beam search; 0 or 1 falls back to greedy.
    /// Measured on a 900s market-review clip (2026-08-24): beam5 removed all
    /// repetition, turbo 4.4% -> 0.0% duplicated 8-grams, with no content loss.
    pub beam_size: u16,
    /// Vocabulary priming. WARNING: measured harmful on this corpus. A 500-char
    /// domain prompt produced hallucinated repetition loops (90% of one segment)
    /// and deleted real content, under both greedy and beam search. It changed
    /// punctuation and little else. Leave empty unless re-validated for
    /// repetition rate on real audio.
    pub initial_prompt: String,
    /// Initial decoding temperature.
    pub temperature: f32,
    /// Fallback step: on a failed quality check the segment is retried at
    /// temperature + this increment. 0.0 disables fallback entirely.
    pub temperature_inc: f32,
    /// Quality gates that trigger the temperature fallback.
    pub entropy_thold: f32,
    pub logprob_thold: f32,
    pub no_speech_thold: f32,
    /// Post-hoc compression-ratio threshold for flagging repetitive speech.
    /// 2.4 matches OpenAI Whisper's own `compression_ratio_threshold`. Diagnostic
    /// only -- flagged windows are reported, never edited. (PR-020 Q5)
    pub repetition_report_thold: f64,
    /// Window over which the compression ratio is scored, in seconds. OpenAI
    /// calibrated 2.4 over 30-second decode windows; per-segment scoring
    /// under-flags because short segments compress worse. Re-scoring this
    /// corpus over 30 s windows recovered two real loops that per-segment
    /// scoring missed, one in a beam-5 transcript. (PR-022, decision 9)
    pub repetition_window_secs: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub endpoint: String,
    pub model: String,
    pub prompt_template_path: Option<String>,
    pub default_prompt: String,
    pub timeout_seconds: u64,
    pub num_ctx: u32,
    /// Sampling temperature. 0.0 selects greedy decoding, removing *sampling*
    /// variance between runs.
    ///
    /// This does NOT deliver bit-identical output. Non-determinism at temperature 0
    /// comes from batch-size dependence of reduction kernels, and the accepted fix
    /// (batch-invariant kernels, ~60% throughput overhead) is not implemented by
    /// Ollama. Runs are repeatable in distribution, not bit-exact.
    ///
    /// A fixed seed is deliberately NOT set: a seed governs the sampling step, and
    /// greedy decoding has none, so it would be inert. See PR-020 Q7.
    pub temperature: f32,
    /// Context tokens reserved for prompt text (template, transcript window,
    /// per-frame time labels) in the per-request pre-flight check:
    /// `max_frames_per_request * tokens_per_frame + prompt_reserve_tokens <= num_ctx`.
    /// Ollama truncates an over-long prompt silently, so the check fails the job
    /// before any GPU time is spent. (PR-022, decision 4)
    pub prompt_reserve_tokens: u32,
    /// Pixels per visual token axis for the served model. Qwen3-VL uses 16-px
    /// patches with 2x2 spatial merge = 32 px; measured on the deployed stack a
    /// frame costs `ceil(w/32) * ceil(h/32) + 2` tokens (1080p = 2042, 720p = 882,
    /// 360p = 222). Change if the vision model changes. (PR-022)
    pub vision_patch_px: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisionConfig {
    pub fps: f32,
    pub max_tokens: u32,
    pub max_frames_per_request: u32,
    /// How much of the chunk's speech the vision prompt may see.
    /// `Full` (default) hands every batch the WHOLE chunk transcript, so a
    /// visual segment at t=0 is generated from words spoken up to t=180 --
    /// up to `chunk_duration_secs` of look-ahead. Use `Causal` for ML features.
    pub transcript_window: TranscriptWindow,
    /// Whether vision sees the transcript at all. `false` makes visual segments
    /// independent observations of the frames; with it enabled, 98% of them
    /// cite the audio and are not independent evidence.
    pub use_transcript: bool,
    /// Content-adaptive frame selection. Disabled by default, so every numeric
    /// `fps` profile keeps its exact previous behaviour. (PR-022)
    pub adaptive: AdaptiveSamplingConfig,
}

/// Content-adaptive frame selection within each chunk (PR-022).
///
/// Frames are evaluated at the candidate rate `vision.fps`. A frame is kept when
/// it is the first of the chunk, or `max_gap_secs` have passed since the last
/// kept frame (the floor), or its ffmpeg `scene` score exceeds `scene_threshold`
/// and `min_trigger_interval_secs` have passed since the last kept frame (a
/// trigger). A chunk exceeding `max_frames_per_chunk` drops its lowest-scoring
/// triggers, never floor frames.
///
/// `scene` compares each candidate with the previous candidate, so it detects
/// change ONSETS (a chart switch, the start of a pan) and not settled states;
/// settled states and small-region changes are captured by the floor. Values
/// below were measured on the market-research corpus (0.037 = crosshair redraw,
/// 0.076/0.126 = chart pan steps, 0.448 = slide change); other content should be
/// re-characterised before these are trusted. See docs/ARCHITECTURE.md.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AdaptiveSamplingConfig {
    pub enabled: bool,
    /// ffmpeg `select` scene score in (0, 1) above which a candidate is a trigger.
    pub scene_threshold: f64,
    /// Floor: guaranteed maximum time between kept frames, in seconds.
    pub max_gap_secs: f64,
    /// Refractory: minimum time between a kept frame and the next trigger, in
    /// seconds. De-clusters bursts during redraws (13-28% of raw triggers).
    pub min_trigger_interval_secs: f64,
    /// Ceiling on kept frames per chunk. Must be at least the floor count
    /// `ceil(chunk_duration_secs / max_gap_secs)`.
    pub max_frames_per_chunk: u32,
}

impl Default for AdaptiveSamplingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scene_threshold: 0.08,
            max_gap_secs: 15.0,
            min_trigger_interval_secs: 2.0,
            max_frames_per_chunk: 45,
        }
    }
}

impl AdaptiveSamplingConfig {
    /// Number of floor frames a chunk of `chunk_secs` is guaranteed to keep:
    /// the first frame plus one every `max_gap_secs`.
    pub fn floor_frames(&self, chunk_secs: f64) -> u32 {
        if self.max_gap_secs <= 0.0 {
            return 1;
        }
        (chunk_secs / self.max_gap_secs).ceil().max(1.0) as u32
    }
}

/// Scope of speech made visible to the vision model for a given batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptWindow {
    /// Entire chunk. Original behaviour; leaks up to chunk_duration_secs.
    Full,
    /// Speech overlapping the batch window. Leak bounded by one segment span.
    Concurrent,
    /// Only speech that ended before the batch began. Zero look-ahead.
    Causal,
}

impl Default for TranscriptWindow {
    fn default() -> Self {
        Self::Full
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessingConfig {
    pub temp_dir: String,
    pub results_dir: String,
    pub max_upload_bytes: u64,
    pub cleanup_checkpoints: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct YtDlpConfig {
    pub path: String,
    pub max_resolution: String,
    pub max_fps: u32,
    pub timeout_seconds: u64,
}

impl Default for YtDlpConfig {
    fn default() -> Self {
        Self {
            path: "yt-dlp".to_string(),
            max_resolution: "1080".to_string(),
            max_fps: 30,
            timeout_seconds: 600,
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerListenConfig::default(),
            ffmpeg: FfmpegConfig::default(),
            whisper: WhisperConfig::default(),
            ollama: OllamaConfig::default(),
            vision: VisionConfig::default(),
            processing: ProcessingConfig::default(),
            ytdlp: YtDlpConfig::default(),
        }
    }
}

impl Default for ServerListenConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            listen_port: 3001,
            max_concurrent_jobs: 1,
        }
    }
}

impl Default for FfmpegConfig {
    fn default() -> Self {
        Self {
            path: "ffmpeg".to_string(),
            chunk_duration_secs: 180,
            frame_format: "jpg".to_string(),
            frame_quality: 2,
        }
    }
}

impl FfmpegConfig {
    /// Derive the ffprobe binary path from the ffmpeg path.
    pub fn ffprobe_path(&self) -> String {
        let path = std::path::Path::new(&self.path);
        match path.parent() {
            Some(dir) if dir != std::path::Path::new("") => {
                dir.join("ffprobe").display().to_string()
            }
            _ => "ffprobe".to_string(),
        }
    }
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            model_path: "models/ggml-large-v3-turbo.bin".to_string(),
            model_size: "large-v3-turbo".to_string(),
            language: "en".to_string(),
            n_threads: 8,
            beam_size: 5,
            initial_prompt: String::new(),
            temperature: 0.0,
            temperature_inc: 0.2,
            entropy_thold: 2.4,
            logprob_thold: -1.0,
            no_speech_thold: 0.6,
            repetition_report_thold: 2.4,
            repetition_window_secs: 30.0,
        }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".to_string(),
            model: "qwen3-vl:8b".to_string(),
            prompt_template_path: Some("prompts/vision.txt".to_string()),
            default_prompt: "Describe the visual content of these video frames in detail."
                .to_string(),
            timeout_seconds: 300,
            num_ctx: 65536,
            temperature: 0.0,
            prompt_reserve_tokens: 4096,
            vision_patch_px: 32,
        }
    }
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            fps: 2.0,
            max_tokens: 4096,
            max_frames_per_request: 15,
            transcript_window: TranscriptWindow::Full,
            use_transcript: true,
            adaptive: AdaptiveSamplingConfig::default(),
        }
    }
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        let results_dir = dirs::home_dir()
            .map(|h| h.join(".vid-to-text").join("server").join("results"))
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "/tmp/vtt-results".to_string());
        Self {
            temp_dir: "/tmp/vtt-jobs".to_string(),
            results_dir,
            max_upload_bytes: 4_294_967_296, // 4 GB
            cleanup_checkpoints: true,
        }
    }
}

impl ServerConfig {
    pub fn validate(&self) -> Result<(), VttError> {
        if self.server.listen_address.is_empty() {
            return Err(VttError::Config(
                "server.listen_address must not be empty".into(),
            ));
        }
        if self.server.listen_port == 0 {
            return Err(VttError::Config(
                "server.listen_port must be greater than 0".into(),
            ));
        }
        if self.server.max_concurrent_jobs == 0 {
            return Err(VttError::Config(
                "server.max_concurrent_jobs must be greater than 0".into(),
            ));
        }
        if self.ffmpeg.path.is_empty() {
            return Err(VttError::Config("ffmpeg.path must not be empty".into()));
        }
        if self.ffmpeg.chunk_duration_secs == 0 {
            return Err(VttError::Config(
                "ffmpeg.chunk_duration_secs must be greater than 0".into(),
            ));
        }
        if self.ffmpeg.frame_format != "jpg" && self.ffmpeg.frame_format != "png" {
            return Err(VttError::Config(
                "ffmpeg.frame_format must be \"jpg\" or \"png\"".into(),
            ));
        }
        if self.ffmpeg.frame_quality == 0 || self.ffmpeg.frame_quality > 31 {
            return Err(VttError::Config(
                "ffmpeg.frame_quality must be between 1 and 31".into(),
            ));
        }
        if self.whisper.model_path.is_empty() {
            return Err(VttError::Config(
                "whisper.model_path must not be empty".into(),
            ));
        }
        if self.whisper.n_threads == 0 {
            return Err(VttError::Config(
                "whisper.n_threads must be greater than 0".into(),
            ));
        }
        if self.ollama.endpoint.is_empty() {
            return Err(VttError::Config(
                "ollama.endpoint must not be empty".into(),
            ));
        }
        if self.ollama.model.is_empty() {
            return Err(VttError::Config("ollama.model must not be empty".into()));
        }
        if self.ollama.timeout_seconds == 0 {
            return Err(VttError::Config(
                "ollama.timeout_seconds must be greater than 0".into(),
            ));
        }
        if self.vision.fps <= 0.0 {
            return Err(VttError::Config(
                "vision.fps must be greater than 0. If a profile set it to 0, that is the \
                 deliberately unset sentinel PR-020 shipped while the sampling mechanism was \
                 undecided; PR-022 decided it (see docs/ARCHITECTURE.md, Frame Sampling). \
                 Set vision.fps explicitly in the profile before running."
                    .into(),
            ));
        }
        if self.vision.adaptive.enabled {
            let a = &self.vision.adaptive;
            if !(a.scene_threshold > 0.0 && a.scene_threshold < 1.0) {
                return Err(VttError::Config(format!(
                    "vision.adaptive.scene_threshold must be in (0, 1), got {}",
                    a.scene_threshold
                )));
            }
            if a.max_gap_secs <= 0.0 {
                return Err(VttError::Config(
                    "vision.adaptive.max_gap_secs must be greater than 0".into(),
                ));
            }
            if a.max_gap_secs * (self.vision.fps as f64) < 1.0 {
                return Err(VttError::Config(format!(
                    "vision.adaptive.max_gap_secs ({}) is shorter than one candidate interval at \
                     vision.fps {} (the floor cannot be honoured below 1/fps seconds)",
                    a.max_gap_secs, self.vision.fps
                )));
            }
            if a.min_trigger_interval_secs < 0.0 {
                return Err(VttError::Config(
                    "vision.adaptive.min_trigger_interval_secs must not be negative".into(),
                ));
            }
            let floor = a.floor_frames(self.ffmpeg.chunk_duration_secs as f64);
            if a.max_frames_per_chunk < floor {
                return Err(VttError::Config(format!(
                    "vision.adaptive.max_frames_per_chunk ({}) is below the floor count ({}) for \
                     chunk_duration_secs {} at max_gap_secs {}; the cap would have to drop floor frames",
                    a.max_frames_per_chunk, floor, self.ffmpeg.chunk_duration_secs, a.max_gap_secs
                )));
            }
        }
        if self.ollama.vision_patch_px == 0 {
            return Err(VttError::Config(
                "ollama.vision_patch_px must be greater than 0".into(),
            ));
        }
        if self.whisper.repetition_window_secs <= 0.0 {
            return Err(VttError::Config(
                "whisper.repetition_window_secs must be greater than 0".into(),
            ));
        }
        if self.vision.max_tokens == 0 {
            return Err(VttError::Config(
                "vision.max_tokens must be greater than 0".into(),
            ));
        }
        if self.vision.max_frames_per_request == 0 {
            return Err(VttError::Config(
                "vision.max_frames_per_request must be greater than 0".into(),
            ));
        }
        if self.processing.temp_dir.is_empty() {
            return Err(VttError::Config(
                "processing.temp_dir must not be empty".into(),
            ));
        }
        if self.processing.results_dir.is_empty() {
            return Err(VttError::Config(
                "processing.results_dir must not be empty".into(),
            ));
        }
        if self.ytdlp.path.is_empty() {
            return Err(VttError::Config("ytdlp.path must not be empty".into()));
        }
        if self.ytdlp.max_fps == 0 {
            return Err(VttError::Config(
                "ytdlp.max_fps must be greater than 0".into(),
            ));
        }
        if self.ytdlp.timeout_seconds == 0 {
            return Err(VttError::Config(
                "ytdlp.timeout_seconds must be greater than 0".into(),
            ));
        }
        Ok(())
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server.listen_address, self.server.listen_port)
    }
}

// --- Config Loading ---

pub fn config_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|p| p.join(".vid-to-text").join("config"))
}

pub fn config_file_path(filename: &str) -> Option<PathBuf> {
    config_dir().map(|d| d.join(filename))
}

pub fn config_file_exists(filename: &str) -> bool {
    config_file_path(filename)
        .map(|p| p.exists())
        .unwrap_or(false)
}

pub fn load_config<T>(filename: &str) -> Result<T, VttError>
where
    T: Default + serde::de::DeserializeOwned,
{
    let path = config_file_path(filename)
        .ok_or_else(|| VttError::Config("could not determine config directory".into()))?;

    if !path.exists() {
        return Ok(T::default());
    }

    let contents = std::fs::read_to_string(&path)
        .map_err(|e| VttError::Config(format!("failed to read {}: {}", path.display(), e)))?;

    toml::from_str(&contents)
        .map_err(|e| VttError::Config(format!("failed to parse {}: {}", path.display(), e)))
}

/// Returns the profiles directory path.
pub fn profiles_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("profiles"))
}

/// Load a server config profile by name and merge it with a base config.
/// The profile only needs to contain the fields being overridden.
pub fn load_profile(base: &ServerConfig, profile_name: &str) -> Result<ServerConfig, VttError> {
    let dir = profiles_dir()
        .ok_or_else(|| VttError::Config("could not determine profiles directory".into()))?;

    let path = dir.join(format!("{profile_name}.toml"));
    if !path.exists() {
        return Err(VttError::Config(format!(
            "profile not found: {} (looked in {})",
            profile_name,
            path.display()
        )));
    }

    let contents = std::fs::read_to_string(&path)
        .map_err(|e| VttError::Config(format!("failed to read profile {}: {}", path.display(), e)))?;

    // Parse the profile TOML into a Value to merge selectively
    let overrides: toml::Value = toml::from_str(&contents)
        .map_err(|e| VttError::Config(format!("failed to parse profile {}: {}", path.display(), e)))?;

    // Serialize the base config to TOML Value, merge, then deserialize back
    let base_str = toml::to_string(base)
        .map_err(|e| VttError::Config(format!("failed to serialize base config: {e}")))?;
    let mut base_val: toml::Value = toml::from_str(&base_str)
        .map_err(|e| VttError::Config(format!("failed to parse base config: {e}")))?;

    // Deep merge: override values replace base values
    merge_toml(&mut base_val, &overrides);

    let merged: ServerConfig = base_val.try_into()
        .map_err(|e| VttError::Config(format!("failed to apply profile {profile_name}: {e}")))?;

    Ok(merged)
}

/// List available profile names.
pub fn list_profiles() -> Vec<String> {
    let dir = match profiles_dir() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".toml") {
                Some(name.trim_end_matches(".toml").to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Deep merge TOML values: override replaces base at the leaf level.
fn merge_toml(base: &mut toml::Value, overrides: &toml::Value) {
    match (base.as_table_mut(), overrides.as_table()) {
        (Some(base_table), Some(override_table)) => {
            for (key, override_val) in override_table {
                if let Some(base_val) = base_table.get_mut(key) {
                    merge_toml(base_val, override_val);
                } else {
                    base_table.insert(key.clone(), override_val.clone());
                }
            }
        }
        _ => {
            *base = overrides.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Default validity ---

    #[test]
    fn test_client_config_defaults_are_valid() {
        let config = ClientConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_server_config_defaults_are_valid() {
        let config = ServerConfig::default();
        assert!(config.validate().is_ok());
    }

    // --- Default values ---

    #[test]
    fn test_client_config_default_values() {
        let config = ClientConfig::default();
        assert_eq!(config.server.host, "localhost");
        assert_eq!(config.server.port, 3001);
        assert_eq!(config.output.dir, None);
    }

    #[test]
    fn test_server_config_default_values() {
        let config = ServerConfig::default();
        assert_eq!(config.server.listen_address, "0.0.0.0");
        assert_eq!(config.server.listen_port, 3001);
        assert_eq!(config.ffmpeg.path, "ffmpeg");
        assert_eq!(config.ffmpeg.chunk_duration_secs, 180);
        assert_eq!(config.whisper.language, "en");
        assert_eq!(config.ollama.endpoint, "http://localhost:11434");
        assert_eq!(config.ollama.model, "qwen3-vl:8b");
        assert_eq!(config.vision.fps, 2.0);
        assert_eq!(config.vision.max_tokens, 4096);
        assert_eq!(config.processing.temp_dir, "/tmp/vtt-jobs");
    }

    // --- TOML roundtrip ---

    #[test]
    fn test_client_config_toml_roundtrip() {
        let config = ClientConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: ClientConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn test_server_config_toml_roundtrip() {
        let config = ServerConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: ServerConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }

    // --- Partial TOML fills defaults ---

    #[test]
    fn test_partial_client_config() {
        let toml_str = r#"
[server]
host = "myserver.ts.net"
"#;
        let config: ClientConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "myserver.ts.net");
        assert_eq!(config.server.port, 3001);
        assert_eq!(config.output.dir, None);
    }

    #[test]
    fn test_partial_server_config() {
        let toml_str = r#"
[ffmpeg]
chunk_duration_secs = 120

[vision]
fps = 1.0
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.ffmpeg.chunk_duration_secs, 120);
        assert_eq!(config.vision.fps, 1.0);
        assert_eq!(config.server.listen_port, 3001);
        assert_eq!(config.ollama.model, "qwen3-vl:8b");
    }

    // --- Empty TOML gives defaults ---

    #[test]
    fn test_empty_toml_gives_client_defaults() {
        let config: ClientConfig = toml::from_str("").unwrap();
        assert_eq!(config, ClientConfig::default());
    }

    #[test]
    fn test_empty_toml_gives_server_defaults() {
        let config: ServerConfig = toml::from_str("").unwrap();
        assert_eq!(config, ServerConfig::default());
    }

    // --- Full TOML with every field ---

    #[test]
    fn test_client_config_full_toml() {
        let toml_str = r#"
[server]
host = "192.168.1.100"
port = 8080

[output]
dir = "/home/user/output"
"#;
        let config: ClientConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "192.168.1.100");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.output.dir, Some("/home/user/output".to_string()));
    }

    #[test]
    fn test_server_config_full_toml() {
        let toml_str = r#"
[server]
listen_address = "127.0.0.1"
listen_port = 8080
max_concurrent_jobs = 2

[ffmpeg]
path = "/usr/local/bin/ffmpeg"
chunk_duration_secs = 120
frame_format = "png"
frame_quality = 5

[whisper]
model_path = "/models/whisper.bin"
model_size = "medium"
language = "ja"
n_threads = 4

[ollama]
endpoint = "http://192.168.1.100:11434"
model = "qwen3-vl:latest"
prompt_template_path = "/etc/vtt/prompt.txt"
default_prompt = "Custom default prompt"
timeout_seconds = 600
num_ctx = 65536

[vision]
fps = 1.0
max_tokens = 8192
max_frames_per_request = 500

[processing]
temp_dir = "/var/tmp/vtt"
max_upload_bytes = 8589934592
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.listen_address, "127.0.0.1");
        assert_eq!(config.server.listen_port, 8080);
        assert_eq!(config.server.max_concurrent_jobs, 2);
        assert_eq!(config.ffmpeg.path, "/usr/local/bin/ffmpeg");
        assert_eq!(config.ffmpeg.chunk_duration_secs, 120);
        assert_eq!(config.ffmpeg.frame_format, "png");
        assert_eq!(config.ffmpeg.frame_quality, 5);
        assert_eq!(config.whisper.model_path, "/models/whisper.bin");
        assert_eq!(config.whisper.model_size, "medium");
        assert_eq!(config.whisper.language, "ja");
        assert_eq!(config.whisper.n_threads, 4);
        assert_eq!(config.ollama.endpoint, "http://192.168.1.100:11434");
        assert_eq!(config.ollama.model, "qwen3-vl:latest");
        assert_eq!(
            config.ollama.prompt_template_path,
            Some("/etc/vtt/prompt.txt".to_string())
        );
        assert_eq!(config.ollama.default_prompt, "Custom default prompt");
        assert_eq!(config.ollama.timeout_seconds, 600);
        assert_eq!(config.ollama.num_ctx, 65536);
        assert_eq!(config.vision.fps, 1.0);
        assert_eq!(config.vision.max_tokens, 8192);
        assert_eq!(config.vision.max_frames_per_request, 500);
        assert_eq!(config.processing.temp_dir, "/var/tmp/vtt");
        assert_eq!(config.processing.max_upload_bytes, 8589934592);
    }

    // --- Validation rejects bad values ---

    #[test]
    fn test_client_validation_rejects_empty_host() {
        let mut config = ClientConfig::default();
        config.server.host = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_client_validation_rejects_zero_port() {
        let mut config = ClientConfig::default();
        config.server.port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_validation_rejects_zero_port() {
        let mut config = ServerConfig::default();
        config.server.listen_port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_validation_rejects_zero_chunk_duration() {
        let mut config = ServerConfig::default();
        config.ffmpeg.chunk_duration_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_validation_rejects_zero_fps() {
        let mut config = ServerConfig::default();
        config.vision.fps = 0.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_validation_rejects_negative_fps() {
        let mut config = ServerConfig::default();
        config.vision.fps = -1.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_validation_rejects_empty_ollama_endpoint() {
        let mut config = ServerConfig::default();
        config.ollama.endpoint = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_validation_rejects_zero_max_tokens() {
        let mut config = ServerConfig::default();
        config.vision.max_tokens = 0;
        assert!(config.validate().is_err());
    }

    // --- Invalid TOML ---

    #[test]
    fn test_invalid_toml_syntax() {
        let result = toml::from_str::<ServerConfig>("this is [not valid");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_type_in_toml() {
        let result = toml::from_str::<ServerConfig>(
            r#"
[server]
listen_port = "not a number"
"#,
        );
        assert!(result.is_err());
    }

    // --- load_config with missing file ---

    #[test]
    fn test_load_config_missing_file_returns_defaults() {
        let config: ClientConfig = load_config("nonexistent_test_file.toml").unwrap();
        assert_eq!(config, ClientConfig::default());
    }

    // --- Helper methods ---

    #[test]
    fn test_client_server_url() {
        let config = ClientConfig::default();
        assert_eq!(config.server_url(), "http://localhost:3001");
    }

    #[test]
    fn test_server_bind_address() {
        let config = ServerConfig::default();
        assert_eq!(config.bind_address(), "0.0.0.0:3001");
    }

    #[test]
    fn test_ffprobe_path_from_bare_ffmpeg() {
        let config = FfmpegConfig::default();
        assert_eq!(config.ffprobe_path(), "ffprobe");
    }

    #[test]
    fn test_ffprobe_path_from_absolute_path() {
        let mut config = FfmpegConfig::default();
        config.path = "/usr/local/bin/ffmpeg".to_string();
        assert_eq!(config.ffprobe_path(), "/usr/local/bin/ffprobe");
    }

    #[test]
    fn test_server_validation_rejects_invalid_frame_format() {
        let mut config = ServerConfig::default();
        config.ffmpeg.frame_format = "gif".to_string();
        assert!(config.validate().is_err());
    }

    /// A profile that deliberately leaves `fps` unset must FAIL, not silently fall
    /// through to the code default. Omitting the key in TOML yields the default 2.0,
    /// which would run a 106-hour corpus while the operator believes fps is unlocked.
    /// The sentinel 0.0 makes the intent explicit and blocks the job at validation,
    /// before any GPU time is spent. (PR-020 review)
    #[test]
    fn test_validation_rejects_deliberately_unset_fps_with_actionable_message() {
        let mut config = ServerConfig::default();
        config.vision.fps = 0.0;
        let err = config.validate().expect_err("fps 0.0 must be rejected");
        let msg = format!("{err}");
        assert!(msg.contains("vision.fps"), "message must name the key: {msg}");
        assert!(
            msg.contains("deliberately unset") || msg.contains("PR-022"),
            "message must explain the deliberate-unset case, not just 'must be > 0': {msg}"
        );
    }

    #[test]
    fn test_server_validation_rejects_zero_frame_quality() {
        let mut config = ServerConfig::default();
        config.ffmpeg.frame_quality = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_validation_rejects_high_frame_quality() {
        let mut config = ServerConfig::default();
        config.ffmpeg.frame_quality = 32;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_whisper_n_threads_default() {
        let config = WhisperConfig::default();
        assert_eq!(config.n_threads, 8);
    }

    #[test]
    fn test_server_validation_rejects_zero_n_threads() {
        let mut config = ServerConfig::default();
        config.whisper.n_threads = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ollama_timeout_default() {
        let config = OllamaConfig::default();
        assert_eq!(config.timeout_seconds, 300);
    }

    #[test]
    fn test_vision_max_frames_default() {
        let config = VisionConfig::default();
        assert_eq!(config.max_frames_per_request, 15);
    }

    #[test]
    fn test_server_validation_rejects_zero_timeout() {
        let mut config = ServerConfig::default();
        config.ollama.timeout_seconds = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_validation_rejects_zero_max_frames() {
        let mut config = ServerConfig::default();
        config.vision.max_frames_per_request = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_polling_config_defaults() {
        let config = ClientPollingConfig::default();
        assert_eq!(config.poll_interval_secs, 3);
        assert_eq!(config.timeout_secs, 1800);
    }

    #[test]
    fn test_client_validation_rejects_zero_poll_interval() {
        let mut config = ClientConfig::default();
        config.polling.poll_interval_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_client_validation_rejects_zero_timeout() {
        let mut config = ClientConfig::default();
        config.polling.timeout_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_max_upload_bytes_default() {
        let config = ProcessingConfig::default();
        assert_eq!(config.max_upload_bytes, 4_294_967_296);
    }

    #[test]
    fn test_ytdlp_config_defaults() {
        let config = YtDlpConfig::default();
        assert_eq!(config.path, "yt-dlp");
        assert_eq!(config.max_resolution, "1080");
        assert_eq!(config.max_fps, 30);
        assert_eq!(config.timeout_seconds, 600);
    }

    #[test]
    fn test_server_validation_rejects_zero_ytdlp_fps() {
        let mut config = ServerConfig::default();
        config.ytdlp.max_fps = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_server_validation_rejects_zero_ytdlp_timeout() {
        let mut config = ServerConfig::default();
        config.ytdlp.timeout_seconds = 0;
        assert!(config.validate().is_err());
    }
    // --- PR-022: adaptive sampling config ---

    /// Absent `[vision.adaptive]` must mean disabled, so every numeric-fps profile
    /// written before PR-022 behaves exactly as it did.
    #[test]
    fn test_adaptive_absent_means_disabled_and_defaults_are_calibrated_values() {
        let toml_str = r#"
[vision]
fps = 0.5
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.vision.adaptive.enabled);
        assert_eq!(config.vision.fps, 0.5);
        let d = AdaptiveSamplingConfig::default();
        assert_eq!(d.scene_threshold, 0.08);
        assert_eq!(d.max_gap_secs, 15.0);
        assert_eq!(d.min_trigger_interval_secs, 2.0);
        assert_eq!(d.max_frames_per_chunk, 45);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_adaptive_table_parses_and_validates() {
        let toml_str = r#"
[vision]
fps = 2.0
[vision.adaptive]
enabled = true
scene_threshold = 0.1
max_gap_secs = 30.0
min_trigger_interval_secs = 2.0
max_frames_per_chunk = 45
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert!(config.vision.adaptive.enabled);
        assert_eq!(config.vision.adaptive.scene_threshold, 0.1);
        assert_eq!(config.vision.adaptive.max_gap_secs, 30.0);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_adaptive_validation_rejects_bad_values() {
        let mut config = ServerConfig::default();
        config.vision.adaptive.enabled = true;

        config.vision.adaptive.scene_threshold = 0.0;
        assert!(config.validate().unwrap_err().to_string().contains("scene_threshold"));
        config.vision.adaptive.scene_threshold = 1.0;
        assert!(config.validate().unwrap_err().to_string().contains("scene_threshold"));
        config.vision.adaptive.scene_threshold = 0.08;

        config.vision.adaptive.max_gap_secs = 0.0;
        assert!(config.validate().unwrap_err().to_string().contains("max_gap_secs"));
        // Floor shorter than one candidate interval: 0.25 fps => 4 s per candidate.
        config.vision.fps = 0.25;
        config.vision.adaptive.max_gap_secs = 2.0;
        assert!(config.validate().unwrap_err().to_string().contains("candidate interval"));
        config.vision.fps = 2.0;
        config.vision.adaptive.max_gap_secs = 15.0;

        config.vision.adaptive.min_trigger_interval_secs = -1.0;
        assert!(config.validate().unwrap_err().to_string().contains("min_trigger_interval_secs"));
        config.vision.adaptive.min_trigger_interval_secs = 2.0;

        // Cap below the floor count (180 s / 15 s = 12 floor frames).
        config.vision.adaptive.max_frames_per_chunk = 11;
        let msg = config.validate().unwrap_err().to_string();
        assert!(msg.contains("max_frames_per_chunk") && msg.contains("floor count (12)"), "{msg}");
        config.vision.adaptive.max_frames_per_chunk = 12;
        assert!(config.validate().is_ok());
    }

    /// The same bad values must be IGNORED when adaptive sampling is disabled, or
    /// a stale `[vision.adaptive]` table could block a fixed-fps profile.
    #[test]
    fn test_adaptive_values_not_validated_when_disabled() {
        let mut config = ServerConfig::default();
        config.vision.adaptive.enabled = false;
        config.vision.adaptive.scene_threshold = 5.0;
        config.vision.adaptive.max_frames_per_chunk = 0;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_floor_frames_counts_first_frame_plus_every_gap() {
        let a = AdaptiveSamplingConfig { max_gap_secs: 15.0, ..Default::default() };
        assert_eq!(a.floor_frames(180.0), 12);
        let a = AdaptiveSamplingConfig { max_gap_secs: 30.0, ..Default::default() };
        assert_eq!(a.floor_frames(180.0), 6);
        assert_eq!(a.floor_frames(10.0), 1);
        assert_eq!(a.floor_frames(31.0), 2);
    }

    /// The profile mechanism serialises the base config to TOML, deep-merges the
    /// profile, and deserialises the result. A nested table must survive that
    /// round-trip -- this is why `fps = "auto"` was rejected in design.
    #[test]
    fn test_adaptive_table_survives_profile_merge_roundtrip() {
        let base = ServerConfig::default();
        let base_str = toml::to_string(&base).unwrap();
        let mut base_val: toml::Value = toml::from_str(&base_str).unwrap();
        let overrides: toml::Value = toml::from_str(
            "[vision]\nfps = 2.0\n[vision.adaptive]\nenabled = true\nmax_gap_secs = 30.0\n",
        )
        .unwrap();
        merge_toml(&mut base_val, &overrides);
        let merged: ServerConfig = base_val.try_into().unwrap();
        assert!(merged.vision.adaptive.enabled);
        assert_eq!(merged.vision.adaptive.max_gap_secs, 30.0);
        // untouched keys keep their defaults
        assert_eq!(merged.vision.adaptive.scene_threshold, 0.08);
        assert_eq!(merged.vision.max_frames_per_request, 15);
    }

    #[test]
    fn test_new_ollama_and_whisper_fields_default_and_validate() {
        let config = ServerConfig::default();
        assert_eq!(config.ollama.prompt_reserve_tokens, 4096);
        assert_eq!(config.ollama.vision_patch_px, 32);
        assert_eq!(config.whisper.repetition_window_secs, 30.0);
        let mut bad = config.clone();
        bad.whisper.repetition_window_secs = 0.0;
        assert!(bad.validate().unwrap_err().to_string().contains("repetition_window_secs"));
        let mut bad = config;
        bad.ollama.vision_patch_px = 0;
        assert!(bad.validate().unwrap_err().to_string().contains("vision_patch_px"));
    }
    /// Guard the repo's locked profile against TOML table re-homing: a key that
    /// follows a `[vision.adaptive]` header belongs to that table, so placing the
    /// header above `use_transcript` silently reverted two locked values to their
    /// defaults (caught by the capture provenance on the first PR-022 validation
    /// run). This test loads the actual file and asserts every locked value.
    #[test]
    fn test_market_research_profile_locked_values_survive_table_layout() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../config/profiles/market-research.toml");
        let text = std::fs::read_to_string(path).expect("repo profile present");
        let overrides: toml::Value = toml::from_str(&text).unwrap();
        let mut base_val: toml::Value = toml::from_str(&toml::to_string(&ServerConfig::default()).unwrap()).unwrap();
        merge_toml(&mut base_val, &overrides);
        let cfg: ServerConfig = base_val.try_into().unwrap();
        assert!(!cfg.vision.use_transcript, "use_transcript must be false (PR-020 Q3)");
        assert_eq!(cfg.vision.transcript_window, TranscriptWindow::Causal, "PR-020 Q4");
        assert_eq!(cfg.vision.fps, 2.0);
        assert!(cfg.vision.adaptive.enabled);
        assert_eq!(cfg.vision.adaptive.scene_threshold, 0.08);
        assert_eq!(cfg.vision.adaptive.max_gap_secs, 15.0);
        assert_eq!(cfg.vision.adaptive.min_trigger_interval_secs, 2.0);
        assert_eq!(cfg.vision.adaptive.max_frames_per_chunk, 45);
        assert_eq!(cfg.whisper.beam_size, 5);
        assert_eq!(cfg.whisper.initial_prompt, "");
        assert_eq!(cfg.whisper.repetition_window_secs, 30.0);
        assert_eq!(cfg.ollama.temperature, 0.0);
        // no key may have been re-homed under the adaptive table
        let adaptive = overrides["vision"]["adaptive"].as_table().unwrap();
        for k in adaptive.keys() {
            assert!(
                ["enabled", "scene_threshold", "max_gap_secs", "min_trigger_interval_secs", "max_frames_per_chunk"].contains(&k.as_str()),
                "unexpected key under [vision.adaptive]: {k}"
            );
        }
        cfg.validate().unwrap();
    }
}
