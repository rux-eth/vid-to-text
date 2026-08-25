use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::{
    format_timestamp, parse_timestamp, Chunk, FrameSample, OcrFrame, OcrGroundingConfig,
    OllamaConfig, Segment, SegmentType, TranscriptWindow, VisionConfig, VttError,
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
    /// Adaptive mode balances batch sizes (see `batch_sizes`); fixed mode keeps
    /// the legacy fill-then-remainder split so its output is byte-identical.
    balanced_batches: bool,
    /// OCR-grounded prompts (PR-024).
    grounding: OcrGroundingConfig,
    /// Degenerate-enumeration cap (PR-025).
    max_numeric_run: usize,
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
            balanced_batches: vision_config.adaptive.enabled,
            grounding: vision_config.ocr_grounding.clone(),
            max_numeric_run: vision_config.max_numeric_run as usize,
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
    /// Returns one Visual segment per batch of frames. Segment bounds and the
    /// per-frame time labels in the prompt come from the frames' real timestamps
    /// (PR-022), never from an assumed spacing.
    pub async fn describe_chunk(
        &self,
        chunk: &Chunk,
        frames: &[FrameSample],
        ocr: &[OcrFrame],
        whisper_segments: &[Segment],
        previous_context: Option<&str>,
    ) -> Result<Vec<Segment>, VttError> {
        if frames.is_empty() {
            return Ok(Vec::new());
        }

        let encoded = encode_frames_base64(frames).await?;
        let sizes = batch_sizes(frames.len(), self.max_frames_per_request as usize, self.balanced_batches);

        let mut segments = Vec::new();
        let mut frame_offset = 0usize;
        for (batch_idx, &size) in sizes.iter().enumerate() {
            let batch = &encoded[frame_offset..frame_offset + size];
            let (batch_start, batch_end) =
                batch_bounds(frames, frame_offset, batch.len(), chunk.end_seconds);
            let frame_times: Vec<String> = frames[frame_offset..frame_offset + batch.len()]
                .iter()
                .map(|f| format_timestamp(f.timestamp))
                .collect();
            // OCR text for exactly these frames, positionally aligned with them
            // (`run_ocr` preserves input order). Empty when grounding is off.
            let frame_text: Vec<String> = if self.grounding.enabled && ocr.len() == frames.len() {
                ocr[frame_offset..frame_offset + batch.len()]
                    .iter()
                    .map(|f| {
                        f.prompt_items(self.grounding.min_score, self.grounding.max_items_per_frame as usize)
                            .iter()
                            .map(|it| it.text.trim())
                            .filter(|t| !t.is_empty())
                            .collect::<Vec<_>>()
                            .join(" | ")
                    })
                    .collect()
            } else {
                Vec::new()
            };

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
            let prompt = build_prompt(&self.prompt_template, windowed.as_deref(), ctx, &frame_times, &frame_text);

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
                        // Degenerate numeric enumeration: no sentence repeats, so
                        // truncate_repetition cannot see it. (PR-025)
                        let (guarded, run) = truncate_numeric_run(&cleaned, self.max_numeric_run);
                        if run > 0 {
                            eprintln!(
                                "[vision] batch {} enumerated {} consecutive numbers (cap {}), truncated from {} to {} chars",
                                batch_idx, run, self.max_numeric_run, cleaned.len(), guarded.len()
                            );
                        }
                        description = Some(guarded);
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
                    frames: frame_times,
                });
            }
            frame_offset += size;
        }

        Ok(segments)
    }
}

/// Split `n` frames into request batches of at most `max` frames.
///
/// Legacy (fixed mode): fill each batch to `max` and put the remainder last, so
/// pre-PR-022 output is reproduced exactly. Balanced (adaptive mode): the same
/// number of batches, sized as evenly as possible -- 16 kept frames become 8+8
/// rather than 15+1, so no visual segment is a single frame standing for a few
/// seconds while its neighbour covers three minutes. (PR-022)
pub fn batch_sizes(n: usize, max: usize, balanced: bool) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let max = max.max(1);
    let k = (n + max - 1) / max;
    if !balanced {
        let mut v = vec![max; k];
        let rem = n - max * (k - 1);
        v[k - 1] = rem;
        return v;
    }
    let base = n / k;
    let extra = n % k;
    (0..k).map(|i| base + usize::from(i < extra)).collect()
}

/// Time span a batch of frames describes: from its first frame's timestamp to the
/// next batch's first timestamp, or the chunk end for the last batch. (PR-022)
///
/// For uniformly spaced frames this equals the previous
/// `chunk.start + frame_offset / fps` arithmetic exactly; for adaptive frames it
/// is the only correct answer, because a kept frame stands for the screen state
/// until the next kept frame.
pub fn batch_bounds(frames: &[FrameSample], offset: usize, len: usize, chunk_end: f64) -> (f64, f64) {
    let start = frames[offset].timestamp;
    let end = frames
        .get(offset + len)
        .map(|f| f.timestamp)
        .unwrap_or(chunk_end)
        .max(start);
    (start, end)
}

/// Visual tokens one frame costs on the served model: one token per
/// `patch_px x patch_px` cell plus the two vision delimiter tokens. Each axis is
/// rounded to the nearest whole cell with ties to even, which is Qwen's
/// `smart_resize` (`round(h / factor) * factor`, Python rounding). Measured on
/// Ollama 0.18.3 + qwen3-vl:8b-instruct-q8_0: 1080p = 2042 (60 x 34),
/// 720p = 882 (40 x 22: 22.5 rounds to even), 360p = 222 (20 x 11).
pub fn tokens_per_frame(width: u32, height: u32, patch_px: u32) -> u32 {
    let cells = |px: u32| ((px as f64 / patch_px as f64).round_ties_even() as u32).max(1);
    cells(width) * cells(height) + 2
}

/// Pre-flight: refuse a job whose full-size request could not fit the context.
/// Ollama truncates an over-long prompt SILENTLY, so this must fail before any
/// GPU time is spent. Returns the estimated tokens of a full request.
pub fn check_context_budget(
    ollama: &OllamaConfig,
    vision: &VisionConfig,
    width: u32,
    height: u32,
) -> Result<u32, VttError> {
    let per_frame = tokens_per_frame(width, height, ollama.vision_patch_px);
    let images = per_frame.saturating_mul(vision.max_frames_per_request);
    // OCR-grounded prompts quote up to `max_items_per_frame` items per frame (PR-024).
    let ocr_tokens = if vision.ocr_grounding.enabled {
        vision
            .ocr_grounding
            .max_items_per_frame
            .saturating_mul(vision.ocr_grounding.tokens_per_item)
            .saturating_mul(vision.max_frames_per_request)
    } else {
        0
    };
    let total = images
        .saturating_add(ocr_tokens)
        .saturating_add(ollama.prompt_reserve_tokens);
    if total > ollama.num_ctx {
        let per_frame_all = per_frame.saturating_add(if vision.ocr_grounding.enabled {
            vision.ocr_grounding.max_items_per_frame.saturating_mul(vision.ocr_grounding.tokens_per_item)
        } else {
            0
        });
        let fit = ollama.num_ctx.saturating_sub(ollama.prompt_reserve_tokens) / per_frame_all.max(1);
        return Err(VttError::Config(format!(
            "vision request would not fit ollama.num_ctx: {} frames x {} tokens ({}x{} at {} px per \
             token) + {} reserved = {} > {}. Lower vision.max_frames_per_request to {} or raise \
             ollama.num_ctx; Ollama would otherwise truncate the prompt silently.",
            vision.max_frames_per_request,
            per_frame,
            width,
            height,
            ollama.vision_patch_px,
            ollama.prompt_reserve_tokens,
            total,
            ollama.num_ctx,
            fit
        )));
    }
    Ok(total)
}

/// Load prompt template from file or return the default.
fn load_prompt_template(path: &Option<String>, default_prompt: &str) -> Result<String, VttError> {
    match path {
        Some(p) if !p.is_empty() => std::fs::read_to_string(p)
            .map_err(|e| VttError::Vision(format!("failed to read prompt template '{p}': {e}"))),
        _ => Ok(default_prompt.to_string()),
    }
}

/// Truncate a degenerate numeric enumeration, keeping the legitimate prefix. (PR-025)
///
/// Vision output degenerates in two measured ways, neither visible to
/// `truncate_repetition` (which keys on repeated sentences): it continues a real
/// list of chart levels into a generated ramp -- "0, 0.25, 0.5, 0.618, 0.786, 1,
/// 1.272, 1.382, 1.493, 1.618, 1.738" running on into "1.801, 1.802, 1.803, ..."
/// for 500+ terms, 82% of the segment's words -- or repeats one value ("1.738T,
/// 1.738T, ..." x40). Capping the run length catches both.
///
/// A "run" is consecutive numeric tokens separated only by whitespace, commas or
/// semicolons. When a run exceeds `max_run` tokens the text is cut at the start of
/// the first token past the cap, so the legitimate head of the list survives.
/// Returns the text unchanged when `max_run` is 0 or no run exceeds it.
pub fn truncate_numeric_run(text: &str, max_run: usize) -> (String, usize) {
    if max_run == 0 {
        return (text.to_string(), 0);
    }
    let b = text.as_bytes();
    // A numeric token: optional sign/currency, digits, grouped or decimal digits,
    // optional unit suffix. Returns its end offset when one starts exactly at `i`.
    let number_end = |mut i: usize| -> Option<usize> {
        if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
            i += 1;
        }
        if i < b.len() && b[i] == b'$' {
            i += 1;
        }
        let digits = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == digits {
            return None;
        }
        while i + 1 < b.len() && (b[i] == b',' || b[i] == b'.') && b[i + 1].is_ascii_digit() {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        }
        if i < b.len() && matches!(b[i], b'%' | b'k' | b'K' | b'M' | b'B' | b'T') {
            i += 1;
        }
        Some(i)
    };

    let mut toks: Vec<(usize, usize)> = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let at_boundary = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        if at_boundary {
            if let Some(end) = number_end(i) {
                toks.push((i, end));
                i = end;
                continue;
            }
        }
        i += 1;
    }
    // Two numeric tokens belong to the same run when only whitespace, commas or
    // semicolons separate them; any word between them starts a new run.
    let joined = |a: usize, c: usize| -> bool {
        let gap = &text[toks[a].1..toks[c].0];
        !gap.is_empty() && gap.chars().all(|ch| ch.is_whitespace() || ch == ',' || ch == ';')
    };
    let mut run_start = 0usize;
    for k in 0..toks.len() {
        if k > 0 && !joined(k - 1, k) {
            run_start = k;
        }
        if k - run_start + 1 > max_run {
            let mut last = k;
            while last + 1 < toks.len() && joined(last, last + 1) {
                last += 1;
            }
            let total = last - run_start + 1;
            let head = text[..toks[k].0]
                .trim_end_matches(|c: char| c.is_whitespace() || c == ',' || c == ';');
            let mut out = head.to_string();
            if !out.ends_with('.') && !out.ends_with(')') {
                out.push_str(" ...");
            }
            return (out, total);
        }
    }
    (text.to_string(), 0)
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
    frame_times: &[String],
    frame_text: &[String],
) -> String {
    let mut prompt = template.to_string();

    // Per-frame capture times, and (PR-024) the text OCR read from each frame.
    // Qwen3-VL's own processor interleaves "<X.X seconds>" before each frame's
    // vision tokens; Ollama renders all images before the message text, so
    // ordered labels are the faithful equivalent and were verified to be used
    // correctly. (PR-022)
    let grounded = frame_text.len() == frame_times.len() && !frame_text.is_empty();
    if !frame_times.is_empty() {
        if grounded {
            prompt.push_str(&format!(
                "\n\nThis request contains {} frame(s) in chronological order, with the time each \
                 was captured (HH:MM:SS.mmm) and the text an OCR engine detected in it:\n",
                frame_times.len()
            ));
            for (i, (t, text)) in frame_times.iter().zip(frame_text).enumerate() {
                if text.is_empty() {
                    prompt.push_str(&format!("Frame {} ({}): (no text detected)\n", i + 1, t));
                } else {
                    prompt.push_str(&format!("Frame {} ({}): {}\n", i + 1, t, text));
                }
            }
            prompt.push_str(
                "The OCR text is a reading aid, NOT ground truth: it misses stylised text and can \
                 misread digits. The images are authoritative. Use the OCR to check numbers and \
                 labels you are already reading from the image -- prefer it when a value is too \
                 small to read confidently -- but do not report anything you cannot see in the \
                 image itself, and do not simply list the detected text. ",
            );
        } else {
            prompt.push_str(&format!(
                "\n\nThis request contains {} frame(s) in chronological order. Capture time of each \
                 frame (HH:MM:SS.mmm):\n",
                frame_times.len()
            ));
            for (i, t) in frame_times.iter().enumerate() {
                prompt.push_str(&format!("Frame {}: {}\n", i + 1, t));
            }
        }
        prompt.push_str(
            "Frames may be unevenly spaced; use these times when describing when something \
             appeared or changed.",
        );
    }

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
async fn encode_frames_base64(frames: &[FrameSample]) -> Result<Vec<String>, VttError> {
    let mut encoded = Vec::with_capacity(frames.len());
    for path in frames.iter().map(|f| &f.path) {
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
    use std::path::PathBuf;

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
            frames: Vec::new(),
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
        let p = build_prompt("TEMPLATE", None, None, &[], &[]);
        assert!(!p.contains("Audio transcript"));
        assert!(!p.contains("relates to what is being said"));
        let p2 = build_prompt("TEMPLATE", Some("hello"), None, &[], &[]);
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
        let p0 = build_prompt("T", b0.as_deref(), None, &[], &[]);
        let p3 = build_prompt("T", b3.as_deref(), None, &[], &[]);
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

        let frames = [FrameSample { path, timestamp: 0.0, scene_score: 0.0 }];
        let result = encode_frames_base64(&frames).await.unwrap();
        assert_eq!(result.len(), 1);

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&result[0])
            .unwrap();
        assert_eq!(decoded, b"fake jpeg data");
    }

    #[tokio::test]
    async fn test_encode_frames_base64_missing_file() {
        let frames = [FrameSample { path: PathBuf::from("/nonexistent/frame.jpg"), timestamp: 0.0, scene_score: 0.0 }];
        let result = encode_frames_base64(&frames).await;
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
        let result = build_prompt("Describe the scene.", None, None, &[], &[]);
        assert_eq!(result, "Describe the scene.");
    }

    #[test]
    fn test_build_prompt_with_transcript() {
        let result = build_prompt("Describe the scene.", Some("Hello, welcome to the show."), None, &[], &[]);
        assert!(result.contains("Describe the scene."));
        assert!(result.contains("Hello, welcome to the show."));
        assert!(result.contains("Audio transcript"));
    }

    #[test]
    fn test_build_prompt_with_empty_transcript() {
        let result = build_prompt("Describe the scene.", Some("   "), None, &[], &[]);
        assert_eq!(result, "Describe the scene.");
    }

    #[test]
    fn test_build_prompt_with_previous_context() {
        let result = build_prompt("Describe.", Some("Hello."), Some("A man waved goodbye."), &[], &[]);
        assert!(result.contains("Describe."));
        assert!(result.contains("A man waved goodbye."));
        assert!(result.contains("Hello."));
        assert!(result.contains("Context from previous segment"));
    }

    #[test]
    fn test_build_prompt_with_context_no_transcript() {
        let result = build_prompt("Describe.", None, Some("A man waved goodbye."), &[], &[]);
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

        let frames = vec![FrameSample { path: frame_path, timestamp: 0.0, scene_score: 0.0 }];
        let segments = client.describe_chunk(&chunk, &frames, &[], &[], None).await.unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].segment_type, SegmentType::Visual);
        assert!(!segments[0].content.is_empty());
    }
    // --- PR-022: real timestamps, prompt time labels, context pre-flight ---

    fn fsample(t: f64) -> FrameSample {
        FrameSample { path: PathBuf::from(format!("/f/{t}.jpg")), timestamp: t, scene_score: 0.0 }
    }

    /// Uniform spacing must reproduce the pre-PR-022 arithmetic
    /// (`chunk.start + frame_offset / fps`) exactly, batch by batch.
    #[test]
    fn test_batch_bounds_uniform_matches_previous_arithmetic() {
        let fps = 2.0_f64;
        let start = 180.0;
        let frames: Vec<FrameSample> = (0..360).map(|i| fsample(start + i as f64 / fps)).collect();
        let batch = 15;
        for b in 0..24 {
            let off = b * batch;
            let (s, e) = batch_bounds(&frames, off, batch, 360.0);
            let old_s = start + off as f64 / fps;
            let old_e = (start + (off + batch) as f64 / fps).min(360.0);
            assert!((s - old_s).abs() < 1e-9 && (e - old_e).abs() < 1e-9, "batch {b}: {s}-{e} vs {old_s}-{old_e}");
        }
    }

    #[test]
    fn test_batch_bounds_non_uniform_uses_real_timestamps() {
        let frames = vec![fsample(0.0), fsample(12.5), fsample(42.5), fsample(110.0), fsample(140.0)];
        assert_eq!(batch_bounds(&frames, 0, 2, 180.0), (0.0, 42.5));
        assert_eq!(batch_bounds(&frames, 2, 2, 180.0), (42.5, 140.0));
        // last batch runs to the chunk end
        assert_eq!(batch_bounds(&frames, 4, 1, 180.0), (140.0, 180.0));
    }

    #[test]
    fn test_build_prompt_lists_frame_times_in_order() {
        let times = vec!["00:00:00.000".to_string(), "00:00:12.500".to_string()];
        let p = build_prompt("T", None, None, &times, &[]);
        assert!(p.contains("2 frame(s)"));
        assert!(p.find("Frame 1: 00:00:00.000").unwrap() < p.find("Frame 2: 00:00:12.500").unwrap());
        assert!(!build_prompt("T", None, None, &[], &[]).contains("Frame 1"));
    }

    #[test]
    fn test_tokens_per_frame_matches_measured_stack() {
        assert_eq!(tokens_per_frame(1920, 1080, 32), 2042);
        assert_eq!(tokens_per_frame(1280, 720, 32), 882);
        assert_eq!(tokens_per_frame(640, 360, 32), 222);
    }

    #[test]
    fn test_check_context_budget_rejects_silent_overflow() {
        let ollama = OllamaConfig::default(); // num_ctx 65536, reserve 4096, patch 32
        let mut vision = VisionConfig::default(); // 15 frames
        assert_eq!(check_context_budget(&ollama, &vision, 1920, 1080).unwrap(), 15 * 2042 + 4096);
        // 30 x 2042 + 4096 = 65,356 still fits 65,536; 31 does not.
        vision.max_frames_per_request = 30;
        assert!(check_context_budget(&ollama, &vision, 1920, 1080).is_ok());
        vision.max_frames_per_request = 40;
        let msg = check_context_budget(&ollama, &vision, 1920, 1080).unwrap_err().to_string();
        assert!(msg.contains("40 frames x 2042") && msg.contains("Lower vision.max_frames_per_request to 30"), "{msg}");
        // 720p at 40 frames fits (40 x 882 + 4096 = 39,376)
        assert!(check_context_budget(&ollama, &vision, 1280, 720).is_ok());
    }
    #[test]
    fn test_batch_sizes_legacy_fill_then_remainder() {
        assert_eq!(batch_sizes(360, 15, false), vec![15; 24]);
        assert_eq!(batch_sizes(16, 15, false), vec![15, 1]);
        assert_eq!(batch_sizes(26, 15, false), vec![15, 11]);
        assert_eq!(batch_sizes(7, 15, false), vec![7]);
        assert!(batch_sizes(0, 15, false).is_empty());
    }

    #[test]
    fn test_batch_sizes_balanced_same_count_even_sizes() {
        assert_eq!(batch_sizes(360, 15, true), vec![15; 24]);
        assert_eq!(batch_sizes(16, 15, true), vec![8, 8]);
        assert_eq!(batch_sizes(19, 15, true), vec![10, 9]);
        assert_eq!(batch_sizes(31, 15, true), vec![11, 10, 10]);
        assert_eq!(batch_sizes(7, 15, true), vec![7]);
        // never exceeds max, always sums to n
        for n in 1..200 {
            let v = batch_sizes(n, 15, true);
            assert!(v.iter().all(|&s| s <= 15) && v.iter().sum::<usize>() == n, "n={n}: {v:?}");
        }
    }
    #[test]
    fn test_build_prompt_grounded_lists_text_and_subordinates_ocr_to_image() {
        let times = vec!["00:00:00.000".to_string(), "00:00:12.500".to_string()];
        let text = vec!["BITSTAMP | O71708 | H71958".to_string(), String::new()];
        let p = build_prompt("T", None, None, &times, &text);
        assert!(p.contains("Frame 1 (00:00:00.000): BITSTAMP | O71708 | H71958"));
        assert!(p.contains("Frame 2 (00:00:12.500): (no text detected)"));
        assert!(p.contains("NOT ground truth") && p.contains("images are authoritative"));
        assert!(p.contains("do not report anything you cannot see in the image"));
        // ungrounded keeps the PR-022 wording exactly
        let plain = build_prompt("T", None, None, &times, &[]);
        assert!(plain.contains("Frame 1: 00:00:00.000") && !plain.contains("OCR"));
    }

    #[test]
    fn test_context_budget_accounts_for_ocr_tokens() {
        let ollama = OllamaConfig::default();
        let mut vision = VisionConfig::default(); // 15 frames
        let plain = check_context_budget(&ollama, &vision, 1920, 1080).unwrap();
        vision.ocr_grounding.enabled = true; // 60 items x 12 tokens x 15 frames
        let grounded = check_context_budget(&ollama, &vision, 1920, 1080).unwrap();
        assert_eq!(grounded - plain, 60 * 12 * 15);
        // grounding shrinks how many frames fit, and the message says so
        vision.max_frames_per_request = 30;
        let msg = check_context_budget(&ollama, &vision, 1920, 1080).unwrap_err().to_string();
        assert!(msg.contains("Lower vision.max_frames_per_request to 22"), "{msg}");
    }
    // --- PR-025: degenerate numeric enumeration ---

    /// The real clip900 failure, verbatim: a legitimate Fibonacci list that runs on
    /// into a constant +0.001 ramp. The legitimate head must survive.
    #[test]
    fn test_truncate_numeric_run_on_the_real_degenerate_text() {
        let mut text = String::from(
            "The chart is populated with numerous horizontal lines representing key Fibonacci \
             retracement levels (e.g., 0, 0.25, 0.5, 0.618, 0.786, 1, 1.272, 1.382, 1.493, 1.618, 1.738",
        );
        let mut v = 1.801_f64;
        for _ in 0..130 {
            text.push_str(&format!(", {v:.3}"));
            v += 0.001;
        }
        text.push_str(") and a blue trendline.");
        let (out, run) = truncate_numeric_run(&text, 24);
        assert!(run > 130, "reports the observed run length, got {run}");
        assert!(out.len() < text.len() / 3, "the ramp is gone: {} -> {}", text.len(), out.len());
        for keep in ["0.618", "0.786", "1.272", "1.382", "1.493", "1.618", "1.738"] {
            assert!(out.contains(keep), "legitimate level {keep} was cut: {out}");
        }
        assert!(!out.contains("1.877") && !out.contains("1.928"), "ramp tail survived: {out}");
        assert!(out.starts_with("The chart is populated"));
    }

    /// Headroom check: the longest run in any non-degenerate segment measured on
    /// this corpus is 19, so a 19-number list must pass untouched at the default.
    #[test]
    fn test_truncate_numeric_run_leaves_legitimate_lists_alone() {
        let axis: Vec<String> = (0..19).map(|i| format!("{}", 63850 + i * 700)).collect();
        let text = format!("The price axis shows {} in USD.", axis.join(", "));
        let (out, run) = truncate_numeric_run(&text, 24);
        assert_eq!(run, 0, "19 numbers is under the cap");
        assert_eq!(out, text);

        // prose with scattered numbers is never a run
        let prose = "Price rose to 71958 before falling to 71580, a drop of 0.76% over 4 hours.";
        assert_eq!(truncate_numeric_run(prose, 24), (prose.to_string(), 0));

        // disabled
        let long: Vec<String> = (0..100).map(|i| i.to_string()).collect();
        let t = long.join(", ");
        assert_eq!(truncate_numeric_run(&t, 0), (t.clone(), 0));
        assert!(truncate_numeric_run(&t, 24).1 >= 100);
    }

    /// The default cap comes from measuring 2,433 visual segments with this same
    /// tokenizer: legitimate runs top out at 38, degenerate ones start at 166.
    #[test]
    fn test_default_cap_sits_in_the_measured_gap() {
        let cap = VisionConfig::default().max_numeric_run as usize;
        assert_eq!(cap, 40);
        let longest_legit: Vec<String> = (0..38).map(|i| format!("{}.{}T", 1 + i / 10, i % 10)).collect();
        let text = format!("Levels: {}.", longest_legit.join(", "));
        assert_eq!(truncate_numeric_run(&text, cap), (text.clone(), 0), "38 is the longest legitimate run observed");
        let shortest_degen: Vec<String> = (0..166).map(|i| format!("{}.{}", 1 + i / 100, i % 100)).collect();
        let d = format!("Levels: {}.", shortest_degen.join(", "));
        assert_eq!(truncate_numeric_run(&d, cap).1, 166, "the shortest degenerate run observed is caught");
    }

    /// The second degeneration mode: one value repeated rather than a ramp.
    #[test]
    fn test_truncate_numeric_run_catches_repeated_value_mode() {
        let text = format!("levels (e.g., 0.5, 0.618, {}) and a trendline.", vec!["1.738T"; 60].join(", "));
        let (out, run) = truncate_numeric_run(&text, 40);
        assert!(run >= 60, "got {run}");
        assert!(out.starts_with("levels (e.g., 0.5, 0.618"));
        assert!(out.len() < text.len());
    }

    #[test]
    fn test_truncate_numeric_run_handles_units_and_multibyte() {
        let mut v: Vec<String> = (0..40).map(|i| format!("{}.{}T", 1 + i / 10, i % 10)).collect();
        v.insert(0, "2.514T".into());
        let text = format!("Levels: {} \u{b7} done.", v.join(", "));
        let (out, run) = truncate_numeric_run(&text, 24);
        assert!(run >= 40, "got {run}");
        assert!(out.starts_with("Levels: 2.514T"));
        assert!(out.len() < text.len());
    }
}
