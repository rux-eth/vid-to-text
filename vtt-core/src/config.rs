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
            format_prompt_path: None,
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
            port: 3000,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub endpoint: String,
    pub model: String,
    pub prompt_template_path: Option<String>,
    pub default_prompt: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisionConfig {
    pub fps: f32,
    pub max_tokens: u32,
    pub max_frames_per_request: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessingConfig {
    pub temp_dir: String,
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
            listen_port: 3000,
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
        }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".to_string(),
            model: "qwen3-vl:8b".to_string(),
            prompt_template_path: None,
            default_prompt: "You are analyzing a sequence of video frames extracted at regular \
                intervals. The frames are in chronological order and represent a continuous \
                segment of video. Describe the visual content you observe: the setting, people, \
                objects, actions, and any changes across the sequence. Be specific and concise. \
                Focus on what is visually happening, not on speculating about audio or dialogue."
                .to_string(),
            timeout_seconds: 300,
        }
    }
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            fps: 2.0,
            max_tokens: 4096,
            max_frames_per_request: 15,
        }
    }
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            temp_dir: "/tmp/vtt-jobs".to_string(),
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
                "vision.fps must be greater than 0".into(),
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
    dirs::config_dir().map(|p| p.join("vid-to-text"))
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
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.output.dir, None);
    }

    #[test]
    fn test_server_config_default_values() {
        let config = ServerConfig::default();
        assert_eq!(config.server.listen_address, "0.0.0.0");
        assert_eq!(config.server.listen_port, 3000);
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
        assert_eq!(config.server.port, 3000);
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
        assert_eq!(config.server.listen_port, 3000);
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
        assert_eq!(config.server_url(), "http://localhost:3000");
    }

    #[test]
    fn test_server_bind_address() {
        let config = ServerConfig::default();
        assert_eq!(config.bind_address(), "0.0.0.0:3000");
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
}
