use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vtt_core::{OpenAIConfig, Timeline};

const DEFAULT_SYSTEM_PROMPT: &str = "\
Format this video timeline JSON into a human-readable Markdown document \
with a summary, character identification, and scene-by-scene transcript.";

// --- OpenAI API types ---

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_completion_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
}

/// Compute the output path for a formatted file.
pub fn compute_format_output_path(input_path: &Path) -> PathBuf {
    let stem = input_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    input_path.with_file_name(format!("{stem}_formatted.md"))
}

/// Load the system prompt from a file or return the default.
fn load_system_prompt(path: &Option<String>) -> Result<String, String> {
    match path {
        Some(p) if !p.is_empty() => std::fs::read_to_string(p)
            .map_err(|e| format!("failed to read format prompt '{p}': {e}")),
        _ => Ok(DEFAULT_SYSTEM_PROMPT.to_string()),
    }
}

/// Format a Timeline JSON file into human-readable Markdown via OpenAI API.
pub async fn run_format(
    config: &OpenAIConfig,
    input_path: &Path,
    output_path: Option<&Path>,
    model_override: Option<&str>,
) -> Result<(), String> {
    // Load API key from env
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY not set — add it to .env or set as environment variable".to_string())?;

    // Read Timeline JSON
    let json_str = tokio::fs::read_to_string(input_path)
        .await
        .map_err(|e| format!("failed to read {}: {e}", input_path.display()))?;

    // Validate it parses as Timeline
    let timeline: Timeline = serde_json::from_str(&json_str)
        .map_err(|e| format!("invalid Timeline JSON: {e}"))?;

    eprintln!(
        "Formatting {} ({} segments, {:.1}s)...",
        timeline.source,
        timeline.segments.len(),
        timeline.duration_seconds
    );

    // Load system prompt
    let system_prompt = load_system_prompt(&config.format_prompt_path)?;

    let model = model_override.unwrap_or(&config.model).to_string();

    // Build request
    let request = ChatRequest {
        model: model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            ChatMessage {
                role: "user".to_string(),
                content: json_str,
            },
        ],
        max_completion_tokens: config.max_tokens,
    };

    // Call OpenAI API
    let client = reqwest::Client::new();
    let resp = client
        .post(&config.endpoint)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("OpenAI API request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error ({status}): {body}"));
    }

    let chat_response: ChatResponse = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse OpenAI response: {e}"))?;

    let content = chat_response
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    if content.is_empty() {
        return Err("OpenAI returned empty content".to_string());
    }

    // Write output
    let out_path = output_path
        .map(PathBuf::from)
        .unwrap_or_else(|| compute_format_output_path(input_path));

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("failed to create output dir: {e}"))?;
        }
    }

    tokio::fs::write(&out_path, &content)
        .await
        .map_err(|e| format!("failed to write {}: {e}", out_path.display()))?;

    eprintln!("Wrote {} (model: {model})", out_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_format_output_path() {
        let path = compute_format_output_path(Path::new("/output/video.json"));
        assert_eq!(path, PathBuf::from("/output/video_formatted.md"));
    }

    #[test]
    fn test_compute_format_output_path_nested() {
        let path = compute_format_output_path(Path::new("results/my video.json"));
        assert_eq!(path, PathBuf::from("results/my video_formatted.md"));
    }

    #[test]
    fn test_load_system_prompt_default() {
        let result = load_system_prompt(&None).unwrap();
        assert!(result.contains("Format this video timeline"));
    }

    #[test]
    fn test_load_system_prompt_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prompt.txt");
        std::fs::write(&path, "Custom prompt").unwrap();
        let result = load_system_prompt(&Some(path.display().to_string())).unwrap();
        assert_eq!(result, "Custom prompt");
    }

    #[test]
    fn test_load_system_prompt_missing_file() {
        let result = load_system_prompt(&Some("/nonexistent.txt".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_chat_request_serialization() {
        let request = ChatRequest {
            model: "gpt-5.4".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are helpful.".to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "Format this.".to_string(),
                },
            ],
            max_completion_tokens: 4096,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "gpt-5.4");
        assert_eq!(json["messages"].as_array().unwrap().len(), 2);
        assert_eq!(json["max_completion_tokens"], 4096);
    }

    #[test]
    fn test_chat_response_parsing() {
        let json = r##"{"choices":[{"message":{"role":"assistant","content":"# Formatted output"}}]}"##;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content, "# Formatted output");
    }
}
