use thiserror::Error;

#[derive(Debug, Error)]
pub enum VttError {
    #[error("ffmpeg error: {0}")]
    Ffmpeg(String),

    #[error("whisper error: {0}")]
    Whisper(String),

    #[error("vision model error: {0}")]
    Vision(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("server error: {0}")]
    Server(String),

    #[error("client error: {0}")]
    Client(String),
}
