use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::{
    format_timestamp, parse_timestamp, Chunk, OllamaConfig, Segment, SegmentType,
    TranscriptWindow, VisionConfig, VttError,
};

// --- Ollama API serde types ---

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaChatMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    images: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    num_predict: u32,
    num_ctx: u32,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

// --- Ollama tag list (for health check) ---

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelInfo>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelInfo {
    name: String,
}

/// HTTP client for the Ollama vision API.
pub struct OllamaClient {
    client: reqwest::Client,
    endpoint: String,
    model: String,
    prompt_template: String,
    max_tokens: u32,
    max_frames_per_request: u32,
    num_ctx: u32,
    temperature: f32,
    transcript_window: TranscriptWindow,
    use_transcript: bool,
}

impl OllamaClient {
    /// Create a new Ollama client from config.
    pub fn new(
        ollama_config: &OllamaConfig,
        vision_config: &VisionConfig,
    ) -> Result<Self, VttError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(ollama_config.timeout_seconds))
            .build()
            .map_err(|e| VttError::Vision(format!("failed to create HTTP client: {e}")))?;

        let prompt_template = load_prompt_template(&ollama_config.prompt_template_path, &ollama_config.default_prompt)?;

        Ok(Self {
            client,
            endpoint: ollama_config.endpoint.clone(),
            model: ollama_config.model.clone(),
            prompt_template,
            max_tokens: vision_config.max_tokens,
            max_frames_per_request: vision_config.max_frames_per_request,
            num_ctx: ollama_config.num_ctx,
            temperature: ollama_config.temperature,
            transcript_window: vision_config.transcript_window,
            use_transcript: vision_config.use_transcript,
        })
    }

    /// Check that Ollama is running and the configured model is available.
    pub async fn check_health(&self) -> Result<(), VttError> {
        let url = format!("{}/api/tags", self.endpoint);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| VttError::Vision(format!("failed to reach Ollama at {url}: {e}")))?;

        if !resp.status().is_success() {
            return Err(VttError::Vision(format!(
                "Ollama health check returned status {}",
                resp.status()
            )));
        }

        let tags: OllamaTagsResponse = resp
            .json()
            .await
            .map_err(|e| VttError::Vision(format!("failed to parse Ollama tags response: {e}")))?;

        let model_found = tags.models.iter().any(|m| m.name.starts_with(&self.model));
        if !model_found {
            return Err(VttError::Vision(format!(
                "model '{}' not found in Ollama (available: {})",
                self.model,
                tags.models
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        Ok(())
    }

    /// Describe the visual content of a chunk's frames.
    /// If a transcript is provided, it is included in the prompt so the vision
    /// model can relate visual content to what was said/heard.
    /// If previous_context is provided, it gives the model continuity from the prior chunk.
    /// Returns one Visual segment per batch of frames for fine-grained temporal resolution.
    pub async fn describe_chunk(
        &self,
        chunk: &Chunk,
        frame_paths: &[PathBuf],
        whisper_segments: &[Segment],
        fps: f32,
        previous_context: Option<&str>,
    ) -> Result<Vec<Segment>, VttError> {
        if frame_paths.is_empty() {
            return Ok(Vec::new());
        }

        let encoded = encode_frames_base64(frame_paths).await?;
        let batch_size = self.max_frames_per_request as usize;
        let seconds_per_frame = 1.0 / fps as f64;

        let mut segments = Vec::new();
        for (batch_idx, batch) in encoded.chunks(batch_size).enumerate() {
            let frame_offset = batch_idx * batch_size;
            let batch_start = chunk.start_seconds + (frame_offset as f64 * seconds_per_frame);
            let batch_end = (chunk.start_seconds
                + ((frame_offset + batch.len()) as f64 * seconds_per_frame))
                .min(chunk.end_seconds);

            // Build the prompt PER BATCH. Previously this was hoisted outside the
            // loop, so every batch in a chunk shared one prompt containing the
            // whole chunk's speech -- up to 180s of look-ahead for a 7.5s segment.
            let windowed = if self.use_transcript {
                transcript_for_window(
                    whisper_segments,
                    batch_start,
                    batch_end,
                    self.transcript_window,
                )
            } else {
                None
            };
            let ctx = if self.use_transcript { previous_context } else { None };
            let prompt = build_prompt(&self.prompt_template, windowed.as_deref(), ctx);

            // Retry up to 3 times on empty response
            let mut description = None;
            for attempt in 0..3 {
                let request = build_chat_request(
                    &self.model,
                    &prompt,
                    batch.to_vec(),
                    self.max_tokens,
                    self.num_ctx,
                    self.temperature,
                );

                let url = format!("{}/api/chat", self.endpoint);
                let resp = self
                    .client
                    .post(&url)
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| VttError::Vision(format!("Ollama request failed: {e}")))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    return Err(VttError::Vision(format!(
                        "Ollama returned status {status}: {body}"
                    )));
                }

                let body = resp
                    .text()
                    .await
                    .map_err(|e| {
                        VttError::Vision(format!("failed to read Ollama response: {e}"))
                    })?;

                match parse_vision_response(&body) {
                    Ok(desc) => {
                        let cleaned = truncate_repetition(&desc);
                        if cleaned.len() < desc.len() {
                            eprintln!(
                                "[vision] batch {} had repetitive output, truncated from {} to {} chars",
                                batch_idx, desc.len(), cleaned.len()
                            );
                        }
                        description = Some(cleaned);
                        break;
                    }
                    Err(_) if attempt < 2 => {
                        eprintln!(
                            "[vision] batch {} returned empty, retrying ({}/3)...",
                            batch_idx,
                            attempt + 2
                        );
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }

            if let Some(content) = description {
                segments.push(Segment {
                    segment_type: SegmentType::Visual,
                    start: format_timestamp(batch_start),
                    end: format_timestamp(batch_end),
                    content,
                });
            }
        }

        Ok(segments)
    }
}

/// Load prompt template from file or return the default.
fn load_prompt_template(path: &Option<String>, default_prompt: &str) -> Result<String, VttError> {
    match path {
        Some(p) if !p.is_empty() => std::fs::read_to_string(p)
            .map_err(|e| VttError::Vision(format!("failed to read prompt template '{p}': {e}"))),
        _ => Ok(default_prompt.to_string()),
    }
}

/// Detect and truncate repetitive model output (generation loops).
/// If any sentence appears more than 3 times, truncate at the second occurrence.
fn truncate_repetition(text: &str) -> String {
    use std::collections::HashMap;

    let sentences: Vec<&str> = text.split('.').collect();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut result = Vec::new();

    for sent in &sentences {
        let key = sent.trim().to_lowercase();
        if key.len() < 15 {
            result.push(*sent);
            continue;
        }
        let count = seen.entry(key).or_insert(0);
        *count += 1;
        if *count <= 2 {
            result.push(*sent);
        } else if *count == 3 {
            // Stop here — repetition detected
            break;
        }
    }

    let joined = result.join(".");
    let trimmed = joined.trim();
    if trimmed.is_empty() {
        text.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Build the full prompt by combining the template with optional context and transcript.
/// Select the speech a batch's prompt is allowed to see.
///
/// `Full` reproduces the original behaviour (whole chunk, i.e. look-ahead up to
/// `chunk_duration_secs`). `Concurrent` bounds the leak to one segment span.
/// `Causal` admits only speech that finished before the batch began, giving a
/// visual feature that contains no information from its own future.
pub fn transcript_for_window(
    segments: &[Segment],
    batch_start: f64,
    batch_end: f64,
    mode: TranscriptWindow,
) -> Option<String> {
    let picked: Vec<&str> = segments
        .iter()
        .filter(|s| s.segment_type == SegmentType::Speech)
        .filter(|s| !s.content.trim().is_empty())
        .filter(|s| {
            let start = parse_timestamp(&s.start).unwrap_or(0.0);
            let end = parse_timestamp(&s.end).unwrap_or(start);
            match mode {
                TranscriptWindow::Full => true,
                TranscriptWindow::Causal => end <= batch_start,
                TranscriptWindow::Concurrent => start < batch_end && end > batch_start,
            }
        })
        .map(|s| s.content.as_str())
        .collect();

    if picked.is_empty() {
        None
    } else {
        Some(picked.join(" "))
    }
}

fn build_prompt(
    template: &str,
    transcript: Option<&str>,
    previous_context: Option<&str>,
) -> String {
    let mut prompt = template.to_string();

    if let Some(ctx) = previous_context {
        if !ctx.trim().is_empty() {
            prompt.push_str(&format!(
                "\n\nContext from previous segment:\n\"{ctx}\"\n\
                 Maintain continuity with what was previously described."
            ));
        }
    }

    if let Some(t) = transcript {
        if !t.trim().is_empty() {
            prompt.push_str(&format!(
                "\n\nAudio transcript for this segment:\n\"{t}\"\n\n\
                 Use this transcript as context to enrich your visual description. \
                 Note how the visual content relates to what is being said or heard."
            ));
        }
    }

    prompt
}

/// Read JPEG/PNG files and encode them as base64 strings.
async fn encode_frames_base64(frame_paths: &[PathBuf]) -> Result<Vec<String>, VttError> {
    let mut encoded = Vec::with_capacity(frame_paths.len());
    for path in frame_paths {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| VttError::Vision(format!("failed to read frame '{}': {e}", path.display())))?;
        encoded.push(base64::engine::general_purpose::STANDARD.encode(&bytes));
    }
    Ok(encoded)
}

/// Construct the Ollama chat request payload.
#[allow(clippy::too_many_arguments)]
fn build_chat_request(
    model: &str,
    prompt: &str,
    images: Vec<String>,
    max_tokens: u32,
    num_ctx: u32,
    temperature: f32,
) -> OllamaChatRequest {
    OllamaChatRequest {
        model: model.to_string(),
        messages: vec![OllamaChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
            images,
        }],
        stream: false,
        options: OllamaOptions {
            num_predict: max_tokens,
            num_ctx,
            temperature,
        },
    }
}

/// Parse the Ollama chat response, stripping thinking tags if present.
fn parse_vision_response(body: &str) -> Result<String, VttError> {
    let resp: OllamaChatResponse = serde_json::from_str(body)
        .map_err(|e| VttError::Vision(format!("failed to parse Ollama response: {e}")))?;

    let full_content = resp.message.content.trim();
    if full_content.is_empty() {
        return Err(VttError::Vision("Ollama returned empty content".into()));
    }

    let stripped = strip_thinking_tags(full_content);

    // If stripping thinking tags leaves nothing, extract the thinking content itself
    if stripped.is_empty() {
        if let (Some(start), Some(end)) = (full_content.find("<think>"), full_content.find("</think>")) {
            let thinking = full_content[start + "<think>".len()..end].trim();
            if !thinking.is_empty() {
                return Ok(thinking.to_string());
            }
        }
        return Err(VttError::Vision("Ollama returned empty content".into()));
    }

    Ok(stripped.to_string())
}

/// Strip `<think>...</think>` tags from Qwen3-VL Thinking mode output.
/// Returns the content after the closing tag, or the full content if no tags present.
fn strip_thinking_tags(content: &str) -> &str {
    if let Some(end_pos) = content.find("</think>") {
        content[end_pos + "</think>".len()..].trim()
    } else {
        content.trim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Prompt template tests ---

    #[test]
    fn test_default_prompt_from_config_not_empty() {
        let config = OllamaConfig::default();
        assert!(!config.default_prompt.is_empty());
        assert!(config.default_prompt.contains("frames"));
        assert!(config.default_prompt.contains("visual"));
    }

    #[test]
    fn test_load_prompt_template_default() {
        let config = OllamaConfig::default();
        let result = load_prompt_template(&None, &config.default_prompt).unwrap();
        assert_eq!(result, config.default_prompt);
    }

    #[test]
    fn test_load_prompt_template_empty_string() {
        let config = OllamaConfig::default();
        let result = load_prompt_template(&Some(String::new()), &config.default_prompt).unwrap();
        assert_eq!(result, config.default_prompt);
    }

    #[test]
    fn test_load_prompt_template_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prompt.txt");
        std::fs::write(&path, "Custom prompt template").unwrap();

        let result = load_prompt_template(&Some(path.display().to_string()), "unused default").unwrap();
        assert_eq!(result, "Custom prompt template");
    }

    #[test]
    fn test_load_prompt_template_missing_file() {
        let result = load_prompt_template(&Some("/nonexistent/prompt.txt".to_string()), "unused default");
        assert!(result.is_err());
    }

    // --- transcript windowing (PR-018) ---

    fn spk(start: f64, end: f64, text: &str) -> Segment {
        Segment {
            segment_type: SegmentType::Speech,
            start: format_timestamp(start),
            end: format_timestamp(end),
            content: text.to_string(),
        }
    }

    fn sample() -> Vec<Segment> {
        vec![
            spk(0.0, 10.0, "alpha"),
            spk(10.0, 20.0, "bravo"),
            spk(20.0, 30.0, "charlie"),
            spk(30.0, 40.0, "delta"),
        ]
    }

    /// Causal: only speech that has already finished before the batch starts.
    /// This is what removes look-ahead entirely.
    #[test]
    fn test_transcript_window_causal_excludes_future_speech() {
        let got = transcript_for_window(&sample(), 20.0, 30.0, TranscriptWindow::Causal)
            .unwrap();
        assert!(got.contains("alpha") && got.contains("bravo"));
        assert!(!got.contains("charlie"), "charlie is concurrent, not past");
        assert!(!got.contains("delta"), "delta is in the future: LOOK-AHEAD");
    }

    #[test]
    fn test_transcript_window_causal_empty_at_start_returns_none() {
        assert!(transcript_for_window(&sample(), 0.0, 10.0, TranscriptWindow::Causal).is_none());
    }

    /// Concurrent: bounds leakage to one segment span instead of the whole chunk.
    #[test]
    fn test_transcript_window_concurrent_overlaps_only() {
        let got = transcript_for_window(&sample(), 15.0, 25.0, TranscriptWindow::Concurrent)
            .unwrap();
        assert!(got.contains("bravo") && got.contains("charlie"));
        assert!(!got.contains("delta"), "delta starts after the window ends");
    }

    /// Full reproduces the old behaviour: the entire chunk, 180s of look-ahead.
    #[test]
    fn test_transcript_window_full_returns_everything() {
        let got = transcript_for_window(&sample(), 0.0, 10.0, TranscriptWindow::Full).unwrap();
        for w in ["alpha", "bravo", "charlie", "delta"] {
            assert!(got.contains(w), "Full must include {w}");
        }
    }

    /// With the transcript disabled the prompt must contain neither the text nor
    /// the instruction telling the model to cross-reference audio -- that
    /// instruction is why 98% of visual segments cite the transcript.
    #[test]
    fn test_build_prompt_without_transcript_omits_instruction() {
        let p = build_prompt("TEMPLATE", None, None);
        assert!(!p.contains("Audio transcript"));
        assert!(!p.contains("relates to what is being said"));
        let p2 = build_prompt("TEMPLATE", Some("hello"), None);
        assert!(p2.contains("hello"));
    }

    /// REGRESSION GUARD for the real defect: build_prompt was called ONCE
    /// outside the batch loop, so every batch in a chunk shared one prompt and
    /// per-batch windowing was impossible. Different batches must now differ.
    #[test]
    fn test_per_batch_prompts_differ_under_windowing() {
        let segs = sample();
        let b0 = transcript_for_window(&segs, 0.0, 10.0, TranscriptWindow::Concurrent);
        let b3 = transcript_for_window(&segs, 30.0, 40.0, TranscriptWindow::Concurrent);
        let p0 = build_prompt("T", b0.as_deref(), None);
        let p3 = build_prompt("T", b3.as_deref(), None);
        assert_ne!(p0, p3, "batches must get different transcript windows");
    }

    // --- build_chat_request tests ---

    /// temperature=0 selects greedy decoding, removing sampling variance. It does
    /// NOT give bit-identical output (batch-size-dependent reduction kernels), and a
    /// seed is deliberately absent because greedy decoding has no sampling step to
    /// seed -- it would be inert. See PR-020 Q7 and Group D.
    #[test]
    fn test_build_chat_request_pins_temperature_and_sends_no_seed() {
        let request = build_chat_request("model", "prompt", vec!["img".into()], 1024, 32768, 0.0);
        assert_eq!(request.options.temperature, 0.0, "temperature must be pinned");
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"temperature\""), "temperature must reach Ollama");
        assert!(
            !json.contains("\"seed\""),
            "seed must NOT be sent: inert under greedy decoding, and sending it implies \
             a determinism guarantee the serving stack does not provide"
        );
    }

    #[test]
    fn test_build_chat_request_passes_through_non_default_temperature() {
        let request = build_chat_request("model", "prompt", vec![], 1024, 32768, 0.7);
        assert_eq!(request.options.temperature, 0.7);
    }

    #[test]
    fn test_build_chat_request_structure() {
        let request = build_chat_request("qwen3-vl:8b", "Describe this.", vec!["abc".into(), "def".into()], 4096, 32768, 0.0);
        assert_eq!(request.model, "qwen3-vl:8b");
        assert!(!request.stream);
        assert_eq!(request.options.num_predict, 4096);
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "user");
        assert_eq!(request.messages[0].content, "Describe this.");
        assert_eq!(request.messages[0].images.len(), 2);
    }

    #[test]
    fn test_build_chat_request_empty_images() {
        let request = build_chat_request("model", "prompt", vec![], 1024, 32768, 0.0);
        assert!(request.messages[0].images.is_empty());
    }

    #[test]
    fn test_chat_request_serialization() {
        let request = build_chat_request("model", "prompt", vec!["img1".into()], 1024, 32768, 0.0);
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "model");
        assert_eq!(json["stream"], false);
        assert_eq!(json["options"]["num_predict"], 1024);
        assert_eq!(json["messages"][0]["images"][0], "img1");
    }

    #[test]
    fn test_chat_request_serialization_skips_empty_images() {
        let request = build_chat_request("model", "prompt", vec![], 1024, 32768, 0.0);
        let json = serde_json::to_value(&request).unwrap();
        assert!(json["messages"][0].get("images").is_none());
    }

    // --- parse_vision_response tests ---

    #[test]
    fn test_parse_vision_response_normal() {
        let body = r#"{"message":{"role":"assistant","content":"A cat sits on a windowsill."}}"#;
        let result = parse_vision_response(body).unwrap();
        assert_eq!(result, "A cat sits on a windowsill.");
    }

    #[test]
    fn test_parse_vision_response_with_thinking_tags() {
        let body = r#"{"message":{"role":"assistant","content":"<think>The frames show a domestic scene with a cat...</think>A cat sits on a windowsill looking outside."}}"#;
        let result = parse_vision_response(body).unwrap();
        assert_eq!(result, "A cat sits on a windowsill looking outside.");
    }

    #[test]
    fn test_parse_vision_response_thinking_only_with_content() {
        let body = r#"{"message":{"role":"assistant","content":"<think>The scene shows a cat on a windowsill.</think>   "}}"#;
        let result = parse_vision_response(body).unwrap();
        assert_eq!(result, "The scene shows a cat on a windowsill.");
    }

    #[test]
    fn test_parse_vision_response_thinking_empty() {
        let body = r#"{"message":{"role":"assistant","content":"<think>   </think>   "}}"#;
        let result = parse_vision_response(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_vision_response_empty_content() {
        let body = r#"{"message":{"role":"assistant","content":""}}"#;
        let result = parse_vision_response(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_vision_response_invalid_json() {
        let result = parse_vision_response("not json");
        assert!(result.is_err());
    }

    // --- strip_thinking_tags tests ---

    #[test]
    fn test_strip_thinking_tags_present() {
        assert_eq!(
            strip_thinking_tags("<think>reasoning</think>The answer"),
            "The answer"
        );
    }

    #[test]
    fn test_strip_thinking_tags_absent() {
        assert_eq!(strip_thinking_tags("Just a normal response"), "Just a normal response");
    }

    #[test]
    fn test_strip_thinking_tags_with_whitespace() {
        assert_eq!(
            strip_thinking_tags("<think>blah</think>  \n  The answer  \n"),
            "The answer"
        );
    }

    // --- encode_frames_base64 tests ---

    #[tokio::test]
    async fn test_encode_frames_base64_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.jpg");
        std::fs::write(&path, b"fake jpeg data").unwrap();

        let result = encode_frames_base64(&[path]).await.unwrap();
        assert_eq!(result.len(), 1);

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&result[0])
            .unwrap();
        assert_eq!(decoded, b"fake jpeg data");
    }

    #[tokio::test]
    async fn test_encode_frames_base64_missing_file() {
        let result = encode_frames_base64(&[PathBuf::from("/nonexistent/frame.jpg")]).await;
        assert!(result.is_err());
    }

    // --- Segment timestamp tests ---

    #[test]
    fn test_segment_timestamp_from_chunk() {
        let chunk = Chunk {
            index: 1,
            start_seconds: 180.0,
            end_seconds: 360.0,
        };
        let start = format_timestamp(chunk.start_seconds);
        let end = format_timestamp(chunk.end_seconds);
        assert_eq!(start, "00:03:00.000");
        assert_eq!(end, "00:06:00.000");
    }

    // --- build_prompt tests ---

    #[test]
    fn test_build_prompt_without_transcript() {
        let result = build_prompt("Describe the scene.", None, None);
        assert_eq!(result, "Describe the scene.");
    }

    #[test]
    fn test_build_prompt_with_transcript() {
        let result = build_prompt("Describe the scene.", Some("Hello, welcome to the show."), None);
        assert!(result.contains("Describe the scene."));
        assert!(result.contains("Hello, welcome to the show."));
        assert!(result.contains("Audio transcript"));
    }

    #[test]
    fn test_build_prompt_with_empty_transcript() {
        let result = build_prompt("Describe the scene.", Some("   "), None);
        assert_eq!(result, "Describe the scene.");
    }

    #[test]
    fn test_build_prompt_with_previous_context() {
        let result = build_prompt("Describe.", Some("Hello."), Some("A man waved goodbye."));
        assert!(result.contains("Describe."));
        assert!(result.contains("A man waved goodbye."));
        assert!(result.contains("Hello."));
        assert!(result.contains("Context from previous segment"));
    }

    #[test]
    fn test_build_prompt_with_context_no_transcript() {
        let result = build_prompt("Describe.", None, Some("A man waved goodbye."));
        assert!(result.contains("A man waved goodbye."));
        assert!(!result.contains("Audio transcript"));
    }

    // --- Integration tests (require Ollama running with qwen3-vl:8b) ---

    #[tokio::test]
    #[ignore]
    async fn test_ollama_health_check() {
        let ollama_config = OllamaConfig::default();
        let vision_config = VisionConfig::default();
        let client = OllamaClient::new(&ollama_config, &vision_config).unwrap();
        client.check_health().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_describe_chunk_single_frame() {
        let ollama_config = OllamaConfig::default();
        let vision_config = VisionConfig::default();
        let client = OllamaClient::new(&ollama_config, &vision_config).unwrap();

        // Create a minimal test image (1x1 red pixel JPEG would need a real encoder;
        // just use any existing JPEG for manual testing)
        let dir = tempfile::tempdir().unwrap();
        let frame_path = dir.path().join("frame_000001.jpg");
        // Write minimal bytes — Ollama will try to process it
        std::fs::write(&frame_path, &[0xFF, 0xD8, 0xFF, 0xE0]).unwrap();

        let chunk = Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 3.0,
        };

        let segments = client.describe_chunk(&chunk, &[frame_path], &[], 2.0, None).await.unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].segment_type, SegmentType::Visual);
        assert!(!segments[0].content.is_empty());
    }
}
