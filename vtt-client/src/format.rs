use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vtt_core::{OpenAIConfig, Timeline};

const DEFAULT_SYSTEM_PROMPT: &str = "\
Format this video timeline JSON into a human-readable Markdown document \
with a summary, character identification, and scene-by-scene transcript.";

// --- Chat Completions API types ---

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

// --- Responses API types (for stored prompts) ---

#[derive(Debug, Serialize)]
struct ResponsesRequest {
    model: String,
    prompt: PromptRef,
    input: String,
}

#[derive(Debug, Serialize)]
struct PromptRef {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ResponsesResponse {
    output: Vec<ResponsesOutput>,
}

#[derive(Debug, Deserialize)]
struct ResponsesOutput {
    #[serde(rename = "type")]
    output_type: String,
    #[serde(default)]
    content: Vec<ResponsesContent>,
}

#[derive(Debug, Deserialize)]
struct ResponsesContent {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: String,
}

/// Compute the output path for a formatted file.
/// If input is inside the cache dir, put formatted.md alongside it.
/// Otherwise put _formatted.md next to the input file.
pub fn compute_format_output_path(input_path: &Path) -> PathBuf {
    let cache_marker = ".vid-to-text/cache/";
    let path_str = input_path.display().to_string();
    if path_str.contains(cache_marker) {
        // Inside cache — put formatted.md in the same cache entry dir
        input_path.with_file_name("formatted.md")
    } else {
        let stem = input_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        input_path.with_file_name(format!("{stem}_formatted.md"))
    }
}

/// Load the system prompt from a file or return the default.
fn load_system_prompt(path: &Option<String>) -> Result<String, String> {
    match path {
        Some(p) if !p.is_empty() => std::fs::read_to_string(p)
            .map_err(|e| format!("failed to read format prompt '{p}': {e}")),
        _ => Ok(DEFAULT_SYSTEM_PROMPT.to_string()),
    }
}

/// Format a Timeline JSON file via OpenAI.
/// Uses Responses API if prompt_id is provided, Chat Completions otherwise.
pub async fn run_format(
    config: &OpenAIConfig,
    input_path: &Path,
    output_path: Option<&Path>,
    model_override: Option<&str>,
    prompt_override: Option<&Path>,
    prompt_id: Option<&str>,
) -> Result<(), String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY not set — add it to .env or set as environment variable".to_string())?;

    let json_str = tokio::fs::read_to_string(input_path)
        .await
        .map_err(|e| format!("failed to read {}: {e}", input_path.display()))?;

    let timeline: Timeline = serde_json::from_str(&json_str)
        .map_err(|e| format!("invalid Timeline JSON: {e}"))?;

    eprintln!(
        "Formatting {} ({} segments, {:.1}s)...",
        timeline.source,
        timeline.segments.len(),
        timeline.duration_seconds
    );

    let model = model_override.unwrap_or(&config.model).to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .pool_max_idle_per_host(0)
        .build()
        .map_err(|e| format!("failed to create HTTP client: {e}"))?;

    let content = if let Some(pid) = prompt_id {
        // Responses API with stored prompt
        eprintln!("Using stored prompt: {pid}");
        call_responses_api(&client, &api_key, &model, pid, &json_str).await?
    } else {
        // Chat Completions API with local/config prompt
        let prompt_path = prompt_override
            .map(|p| Some(p.display().to_string()))
            .unwrap_or(config.format_prompt_path.clone());
        let system_prompt = load_system_prompt(&prompt_path)?;

        // Estimate token count (~4 chars per token)
        let est_tokens = json_str.len() / 4;
        let max_input_tokens = 25_000; // Keep chunks under ~200KB for reliable rustls

        if est_tokens > max_input_tokens {
            eprintln!(
                "Large input (~{}K tokens), chunking into sections...",
                est_tokens / 1000
            );
            format_chunked(
                &client, &api_key, &model, &system_prompt, &timeline, config.max_tokens,
            )
            .await?
        } else {
            call_chat_api(
                &client, &api_key, &model, &system_prompt, &json_str, config.max_tokens,
            )
            .await?
        }
    };

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

/// Call the Chat Completions API with a system prompt + user message.
async fn call_chat_api(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_content: &str,
    max_tokens: u32,
) -> Result<String, String> {
    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_content.to_string(),
            },
        ],
        max_completion_tokens: max_tokens,
    };

    let body_json = serde_json::to_vec(&request)
        .map_err(|e| format!("failed to serialize request: {e}"))?;
    eprintln!("[format] sending {} bytes to OpenAI...", body_json.len());

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .body(body_json)
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

    Ok(content)
}

/// Call the Responses API with a stored prompt ID.
async fn call_responses_api(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    prompt_id: &str,
    input: &str,
) -> Result<String, String> {
    let request = ResponsesRequest {
        model: model.to_string(),
        prompt: PromptRef {
            id: prompt_id.to_string(),
        },
        input: input.to_string(),
    };

    let resp = client
        .post("https://api.openai.com/v1/responses")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("OpenAI Responses API request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("OpenAI Responses API error ({status}): {body}"));
    }

    let responses: ResponsesResponse = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse Responses API response: {e}"))?;

    // Extract text from the message output (skip reasoning blocks)
    let content = responses
        .output
        .iter()
        .filter(|o| o.output_type == "message")
        .flat_map(|o| o.content.iter())
        .filter(|c| c.content_type == "output_text")
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    if content.is_empty() {
        return Err("OpenAI Responses API returned empty content".to_string());
    }

    Ok(content)
}

/// Format a large timeline by splitting into chunks that fit within context limits.
/// Section 1 gets the full prompt (summary + characters + scenes).
/// Subsequent sections continue the transcript with context from section 1.
async fn format_chunked(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    timeline: &Timeline,
    max_tokens: u32,
) -> Result<String, String> {
    let segments = &timeline.segments;
    let max_input_tokens = 25_000;

    // Split segments into chunks that fit under the token limit
    let mut chunks: Vec<Vec<&vtt_core::Segment>> = Vec::new();
    let mut current_chunk: Vec<&vtt_core::Segment> = Vec::new();
    let mut current_size = 0;

    for seg in segments {
        let seg_size = seg.content.len() / 4 + 50; // rough token estimate + overhead
        if current_size + seg_size > max_input_tokens && !current_chunk.is_empty() {
            chunks.push(current_chunk);
            current_chunk = Vec::new();
            current_size = 0;
        }
        current_chunk.push(seg);
        current_size += seg_size;
    }
    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    eprintln!("Split into {} sections", chunks.len());

    let mut full_output = String::new();
    let mut header_context = String::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let chunk_timeline = Timeline {
            source: timeline.source.clone(),
            duration_seconds: timeline.duration_seconds,
            segments: chunk.iter().map(|s| (*s).clone()).collect(),
            capture: timeline.capture.clone(),
            fidelity: timeline.fidelity.clone(),
        };
        let chunk_json = serde_json::to_string(&chunk_timeline)
            .map_err(|e| format!("failed to serialize chunk: {e}"))?;

        let prompt = if i == 0 {
            // First section: full format prompt
            system_prompt.to_string()
        } else {
            // Continuation sections
            format!(
                "You are continuing a video transcript document. \
                Here is the summary and character identification from the first section:\n\n\
                {header_context}\n\n\
                Now continue the scene-by-scene transcript for the next batch of segments.\n\n\
                Rules:\n\
                - Do NOT repeat the summary or character identification\n\
                - Do NOT re-introduce characters — they are already established\n\
                - Continue scene numbering from where the previous section left off\n\
                - Only describe what is in the segments provided — do not fabricate or infer beyond the data\n\
                - Maintain the same writing style and format as the previous section\n\
                - Start directly with the next scene heading\n\
                - IMPORTANT: Group consecutive segments into coherent SCENES based on visual context, \
                not raw segment boundaries. Multiple segments sharing the same chart, setting, or topic \
                belong in ONE scene with a descriptive heading. Do NOT create one scene per segment."
            )
        };

        eprintln!("  Section {}/{} ({} segments)...", i + 1, chunks.len(), chunk.len());

        let result = call_chat_api(client, api_key, model, &prompt, &chunk_json, max_tokens).await?;

        if i == 0 {
            // Extract header (summary + characters) for context in later sections
            // Look for the scene transcript start
            if let Some(scene_pos) = result.find("## Scene") {
                header_context = result[..scene_pos].to_string();
            } else if let Some(scene_pos) = result.find("### Scene") {
                header_context = result[..scene_pos].to_string();
            } else {
                // Fallback: take first 2000 chars as context
                header_context = result.chars().take(2000).collect();
            }
            full_output.push_str(&result);
        } else {
            full_output.push_str("\n\n");
            full_output.push_str(&result);
        }
    }

    Ok(full_output)
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
    fn test_responses_request_serialization() {
        let request = ResponsesRequest {
            model: "gpt-5.4".to_string(),
            prompt: PromptRef {
                id: "pmpt_abc123".to_string(),
            },
            input: "hello".to_string(),
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "gpt-5.4");
        assert_eq!(json["prompt"]["id"], "pmpt_abc123");
        assert_eq!(json["input"], "hello");
    }

    #[test]
    fn test_chat_response_parsing() {
        let json = r##"{"choices":[{"message":{"role":"assistant","content":"# Formatted output"}}]}"##;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content, "# Formatted output");
    }

    #[test]
    fn test_responses_response_parsing() {
        let json = r#"{"output":[{"type":"reasoning","content":[]},{"type":"message","content":[{"type":"output_text","text":"here are my questions"}]}]}"#;
        let resp: ResponsesResponse = serde_json::from_str(json).unwrap();
        let text: Vec<_> = resp.output.iter()
            .filter(|o| o.output_type == "message")
            .flat_map(|o| o.content.iter())
            .filter(|c| c.content_type == "output_text")
            .map(|c| c.text.as_str())
            .collect();
        assert_eq!(text, vec!["here are my questions"]);
    }
}
