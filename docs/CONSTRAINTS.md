# Constraints

Hard rules that must never be violated. These are enforced by Claude at all times.

---

## Structural Constraints

These apply to every project built with this template.

### No Phantom Implementations (NON-NEGOTIABLE)

A step is NOT complete if:
- A function exists but returns a default, stub, or placeholder value
- A module is declared but never called from the main flow
- Tests only verify something exists, not that it works correctly

Every PR must include:
1. A test that exercises the **actual behavior**
2. Explicit listing of any stubs or TODOs in the PR description
3. Proof of end-to-end data flow where applicable

### Documentation Accuracy (NON-NEGOTIABLE)

Every code change must include corresponding doc updates in the same commit. Before writing docs, check the actual diff — base doc updates on what changed, not memory. Docs must never describe behavior that doesn't exist in code.

### One PR, One Thing

Each PR is a single, reviewable change. No "while I'm here I'll also add..." — that's scope creep. Each PR references the specific section of the architecture it implements.

### Config Over Hardcoding

All configurable values come from config files. Zero hardcoded parameters for behavior that might change. If a value could reasonably vary between environments or over time, it belongs in config.

---

## Domain Constraints

### No Audio Data on GPU (v1)

Whisper runs on CPU only. The full 24GB of GPU VRAM is reserved for Qwen3-VL. This prevents OOM conditions during parallel processing.

### mp4 Input Only (v1)

Only mp4 files are accepted as input. No format conversion, no container sniffing. If a user has a different format, they convert with ffmpeg themselves.

### Segments Are Immutable After Merge

Once segments from all chunks are merged and sorted into the final timeline, no post-processing modifies their content. The output JSON is a faithful representation of what the models produced. Any transformation (human-readable formatting, SRT export, etc.) happens in a separate layer.

### Checkpoint Integrity

A chunk checkpoint file is only written after the chunk is fully processed by both pipelines. Partial results are never checkpointed. This ensures resumability is always safe — a checkpoint either has complete data or doesn't exist.

### No Network Calls From Client to Models

The client never communicates directly with Ollama or Whisper. All model interaction goes through the server. This keeps the client thin and the server as the single point of control for processing.
