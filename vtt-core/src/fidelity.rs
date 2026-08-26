//! Visual fidelity diagnostic (PR-023).
//!
//! Checks what a visual segment SAYS against what was ON SCREEN, using OCR of the
//! frames it was generated from (recorded in `Segment::frames` since PR-022).
//! Facts are the OCR-checkable classes of the CHOCOLATE chart-caption error
//! typology (Huang et al., ACL 2024): numeric values, labels (tickers, exchanges,
//! indicator names) and out-of-context mentions. Trend and magnitude claims are
//! not checkable from a screenshot and are deliberately not scored.
//!
//! Precision: stated facts found in the OCR of the frames the model saw.
//! Recall: prominent on-screen facts that were mentioned, scored against either
//! the kept frames (per-job diagnostic) or every candidate frame (study mode).
//! This DIAGNOSES; it never edits a segment.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::{
    format_timestamp, parse_timestamp, FidelityConfig, FidelitySummary, OcrConfig, RecallReference,
    Segment, SegmentType, VttError,
};

// --- Facts ---

/// Unit class of a numeric fact. Percentages never match plain numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NumberUnit {
    Plain,
    Percent,
}

/// A checkable fact, as stated in a description or read from a frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Fact {
    /// `value` is fully scaled (k/M/B/T applied); `precision` is the size of the
    /// last stated digit, also scaled -- "42,000" is 1000, "1.66T" is 1e10,
    /// "70142" is 1. `raw` is the token as written.
    Number { value: f64, unit: NumberUnit, precision: f64, raw: String },
    /// Uppercase on-screen token: ticker, exchange, indicator (BTC, CRYPTOCAP, RSI).
    Label { text: String },
    /// Chart timeframe, normalised: minutes lowercase (`15m`), hours/days/weeks/
    /// months uppercase (`4H`, `1D`, `1W`, `1M`).
    Timeframe { text: String },
}

impl Fact {
    /// Deduplication key. Numbers key on their canonical value and unit, so an
    /// OCR misread of the OHLC prefix ("02.514T" for "O2.514T") and the true
    /// "2.514T" are one fact, as are "42k" and "42,000". `raw` is kept for display.
    pub fn key(&self) -> String {
        match self {
            Fact::Number { value, unit, .. } => format!(
                "n:{}{}",
                canonical_number(*value),
                if *unit == NumberUnit::Percent { "%" } else { "" }
            ),
            Fact::Label { text } => format!("l:{text}"),
            Fact::Timeframe { text } => format!("t:{text}"),
        }
    }
}

/// Shortest plain decimal that identifies the value: 2.514e12 -> "2514000000000",
/// 0.0001671 -> "0.0001671", -0.25 -> "-0.25".
fn canonical_number(v: f64) -> String {
    let s = format!("{:.10}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".to_string() } else { s }
}

fn strip_punct(tok: &str) -> &str {
    tok.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | '"' | '\'' | ',' | '.' | ';' | ':' | '!' | '?' | '*' | '`' | '\u{b7}' | '\u{2022}'))
}

/// Parse a chart timeframe token: `15m`, `4H`, `1D`, `1W`, `1M`/`3M`/`6M`/`12M`.
/// `M` with any other value is a million, not a month.
fn parse_timeframe(tok: &str) -> Option<Fact> {
    let last = tok.chars().last()?;
    let digits = &tok[..tok.len() - last.len_utf8()];
    if digits.is_empty() || digits.len() > 3 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = digits.parse().ok()?;
    let text = match last.to_string().as_str() {
        "m" => format!("{n}m"),
        "h" | "H" => format!("{n}H"),
        "d" | "D" => format!("{n}D"),
        "w" | "W" => format!("{n}W"),
        "M" if matches!(n, 1 | 3 | 6 | 12) => format!("{n}M"),
        _ => return None,
    };
    Some(Fact::Timeframe { text })
}

/// Parse a numeric token: optional sign, optional `$`, digits with commas,
/// optional decimals, optional `%` or k/M/B/T multiplier.
fn parse_number(tok: &str) -> Option<Fact> {
    let raw = tok.to_string();
    let mut s = tok;
    let mut sign = 1.0;
    if let Some(rest) = s.strip_prefix('-').or_else(|| s.strip_prefix('\u{2212}')) {
        sign = -1.0;
        s = rest;
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest;
    }
    if let Some(rest) = s.strip_prefix('$') {
        s = rest;
    }
    let mut unit = NumberUnit::Plain;
    let mut mult = 1.0;
    if let Some(rest) = s.strip_suffix('%') {
        unit = NumberUnit::Percent;
        s = rest;
    } else if let Some(last) = s.chars().last() {
        match last {
            'k' | 'K' => mult = 1e3,
            'M' => mult = 1e6,
            'b' | 'B' => mult = 1e9,
            't' | 'T' => mult = 1e12,
            _ => {}
        }
        if mult != 1.0 {
            s = &s[..s.len() - 1];
        }
    }
    if s.is_empty() || !s.bytes().next().map_or(false, |b| b.is_ascii_digit()) {
        return None;
    }
    let cleaned: String = s.chars().filter(|c| *c != ',').collect();
    let (int_part, dec_part) = match cleaned.split_once('.') {
        Some((i, d)) => (i, Some(d)),
        None => (cleaned.as_str(), None),
    };
    if int_part.is_empty() || !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if let Some(d) = dec_part {
        if d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    let value: f64 = cleaned.parse().ok()?;
    // Precision = size of the last stated digit. Decimals: 10^-d. Grouped or
    // suffixed integers ("42,000", "44k"): the trailing zeros are round-off,
    // so 42,000 means the thousand. Plain integers ("2020", "70142", "1") are
    // exact -- a year must not match a neighbouring year, and a Fibonacci "1"
    // must not match 0.883.
    let grouped = tok.contains(',') || mult != 1.0;
    let precision = match dec_part {
        Some(d) => 10f64.powi(-(d.len() as i32)),
        None if grouped => {
            let zeros = int_part.trim_end_matches('0').len();
            let trailing = if int_part.chars().all(|c| c == '0') { 0 } else { int_part.len() - zeros };
            10f64.powi(trailing as i32)
        }
        None => 0.0,
    };
    Some(Fact::Number {
        value: sign * value * mult,
        unit,
        precision: precision * mult,
        raw,
    })
}

fn parse_label(tok: &str) -> Option<Fact> {
    if tok.len() < 2
        || !tok.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        || !tok.bytes().any(|b| b.is_ascii_uppercase())
    {
        return None;
    }
    Some(Fact::Label { text: tok.to_string() })
}

/// Extract checkable facts from free text (a description or an OCR line).
///
/// Supported forms: `42,000` `$42,000` `42k` `1.66T` `-0.25%` `+10.95%`, chart-header
/// OHLC values `O2.514T` `C2.594T`; timeframes `15m` `4H` `1D` `1W` `1M`; uppercase
/// labels of 2+ characters.
/// Not supported (documented, deferred): dates ("Feb 12, 2024"), spelled-out
/// numbers, mixed-case names ("Bitcoin"), ranges ("42-44k").
pub fn extract_facts(text: &str) -> Vec<Fact> {
    extract_facts_with(text, &[])
}

/// `extract_facts` with a stoplist of uppercase prose tokens to ignore as labels.
pub fn extract_facts_with(text: &str, label_stoplist: &[String]) -> Vec<Fact> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut prev = "";
    for word in text.split(|c: char| c.is_whitespace() || c == '/' || c == '|') {
        let tok = strip_punct(word);
        if tok.is_empty() {
            continue;
        }
        // The model numbers frames itself ("Frame 3 (00:00:27.000):"); that
        // ordinal is prompt structure, not an on-screen fact.
        let after_frame = prev.trim_start_matches('*').eq_ignore_ascii_case("frame");
        prev = tok;
        if after_frame && tok.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        // Chart headers print OHLC values as `O2.514T H2.632T L2.492T C2.594T`;
        // strip that single-letter prefix so the value is checkable.
        let ohlc = tok
            .strip_prefix(['O', 'H', 'L', 'C'])
            .filter(|rest| rest.bytes().next().map_or(false, |b| b.is_ascii_digit()))
            .and_then(parse_number);
        let fact = parse_timeframe(tok)
            .or_else(|| parse_number(tok))
            .or(ohlc)
            .or_else(|| parse_label(tok))
            .filter(|f| !matches!(f, Fact::Label { text } if label_stoplist.iter().any(|w| w == text)));
        if let Some(f) = fact {
            if seen.insert(f.key()) {
                out.push(f);
            }
        }
    }
    out
}

/// The precision rule: a stated number matches an on-screen number when the
/// difference is within half of the stated number's own last digit (so "1.66T"
/// matches 1.661T and "42,000" matches 41,958), plus an optional relative
/// tolerance. Percentages only match percentages.
pub fn facts_match(stated: &Fact, screen: &Fact, tolerance: f64) -> bool {
    match (stated, screen) {
        (
            Fact::Number { value: a, unit: ua, precision, .. },
            Fact::Number { value: b, unit: ub, .. },
        ) => ua == ub && (a - b).abs() <= 0.5 * precision + tolerance * b.abs() + 1e-12,
        (Fact::Label { text: a }, Fact::Label { text: b }) => a == b,
        (Fact::Timeframe { text: a }, Fact::Timeframe { text: b }) => a == b,
        _ => false,
    }
}

// --- OCR ---

#[derive(Debug, Clone, Deserialize)]
struct OcrItemRaw {
    text: String,
    #[serde(default)]
    score: f64,
    #[serde(default)]
    #[serde(rename = "box")]
    bbox: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
struct OcrRecordRaw {
    path: String,
    #[serde(default)]
    items: Vec<OcrItemRaw>,
    #[serde(default)]
    error: Option<String>,
}

/// One text region read from a frame. (PR-024 keeps the raw text so the vision
/// prompt can quote it; PR-023 derives facts from it.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrItem {
    pub text: String,
    pub score: f64,
    /// Top-left of the OCR box, in source pixels; reading order is (y, x).
    pub x: f64,
    pub y: f64,
    pub height_px: u32,
}

/// One OCR'd frame: its capture time and the text regions read from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrFrame {
    pub timestamp: f64,
    pub path: String,
    pub items: Vec<OcrItem>,
}

impl OcrFrame {
    /// Checkable facts read from this frame's text.
    pub fn facts(&self) -> Vec<OcrFact> {
        self.items
            .iter()
            .flat_map(|it| {
                extract_facts(&it.text)
                    .into_iter()
                    .map(|fact| OcrFact { fact, height_px: it.height_px, score: it.score })
            })
            .collect()
    }

    /// Text quoted for an OCR-grounded prompt: items above `min_score`, in
    /// reading order (top-to-bottom, then left-to-right), capped at `max_items`.
    /// The cap keeps the highest-confidence items but restores reading order.
    pub fn prompt_items(&self, min_score: f64, max_items: usize) -> Vec<&OcrItem> {
        let mut kept: Vec<&OcrItem> = self
            .items
            .iter()
            .filter(|it| it.score >= min_score && !it.text.trim().is_empty())
            .collect();
        if kept.len() > max_items {
            kept.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            kept.truncate(max_items);
        }
        kept.sort_by(|a, b| {
            a.y.partial_cmp(&b.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });
        kept
    }
}

/// A fact read from a frame, with its OCR box height in source pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrFact {
    pub fact: Fact,
    pub height_px: u32,
    pub score: f64,
}

/// Parse the wrapper's JSON-lines output. Frames with an `error` field yield no
/// facts (logged by the caller); malformed lines are an error.
pub fn parse_ocr_output(stdout: &str) -> Result<Vec<(String, Vec<OcrItem>, Option<String>)>, VttError> {
    let mut out = Vec::new();
    for (i, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let rec: OcrRecordRaw = serde_json::from_str(line)
            .map_err(|e| VttError::Config(format!("fidelity: bad OCR output line {}: {e}", i + 1)))?;
        let items = rec
            .items
            .into_iter()
            .map(|item| {
                let xs: Vec<f64> = item.bbox.iter().filter_map(|p| p.first().copied()).collect();
                let ys: Vec<f64> = item.bbox.iter().filter_map(|p| p.get(1).copied()).collect();
                let lo = ys.iter().cloned().reduce(f64::min);
                let hi = ys.iter().cloned().reduce(f64::max);
                OcrItem {
                    text: item.text,
                    score: item.score,
                    x: xs.iter().cloned().reduce(f64::min).unwrap_or(0.0),
                    y: lo.unwrap_or(0.0),
                    height_px: match (lo, hi) {
                        (Some(a), Some(b)) => (b - a).round().max(0.0) as u32,
                        _ => 0,
                    },
                }
            })
            .collect();
        out.push((rec.path, items, rec.error));
    }
    Ok(out)
}

/// Run the configured OCR command over `frames` (timestamp, path) and return one
/// `OcrFrame` per input in the same order.
pub async fn run_ocr(config: &OcrConfig, frames: &[(f64, PathBuf)]) -> Result<Vec<OcrFrame>, VttError> {
    if frames.is_empty() {
        return Ok(Vec::new());
    }
    let (bin, prefix) = config
        .command
        .split_first()
        .ok_or_else(|| VttError::Config("ocr.command is empty".into()))?;
    let output = Command::new(bin)
        .args(prefix)
        .arg("--workers")
        .arg(config.workers.to_string())
        .arg("--threads")
        .arg(config.threads.to_string())
        .args(frames.iter().map(|(_, p)| p.as_os_str()))
        .output()
        .await
        .map_err(|e| VttError::Config(format!("fidelity: failed to run OCR command {bin}: {e}")))?;
    if !output.status.success() {
        return Err(VttError::Config(format!(
            "fidelity: OCR command exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let parsed = parse_ocr_output(&String::from_utf8_lossy(&output.stdout))?;
    let by_path: HashMap<String, (Vec<OcrItem>, Option<String>)> =
        parsed.into_iter().map(|(p, f, e)| (p, (f, e))).collect();
    let mut out = Vec::with_capacity(frames.len());
    for (ts, path) in frames {
        let key = path.to_string_lossy().to_string();
        let (items, err) = by_path.get(&key).cloned().ok_or_else(|| {
            VttError::Config(format!("ocr: output missing frame {key}"))
        })?;
        if let Some(e) = err {
            eprintln!("[ocr] failed for {key}: {e}");
        }
        out.push(OcrFrame { timestamp: *ts, path: key, items });
    }
    Ok(out)
}

/// Build `OcrFrame`s from the wrapper's raw JSON-lines output for uniformly
/// spaced candidate frames (study mode): the i-th record is at
/// `start + i / fps`. Candidates come from `ffmpeg fps=N`, so this arithmetic is
/// exact for them (unlike kept frames, whose times are recorded).
pub fn candidate_frames_from_output(stdout: &str, start_secs: f64, fps: f64) -> Result<Vec<OcrFrame>, VttError> {
    if fps <= 0.0 {
        return Err(VttError::Config("candidate fps must be greater than 0".into()));
    }
    Ok(parse_ocr_output(stdout)?
        .into_iter()
        .enumerate()
        .map(|(i, (path, items, _))| OcrFrame { timestamp: start_secs + i as f64 / fps, path, items })
        .collect())
}

/// `--check` the OCR command; returns its version line.
pub async fn check_ocr(config: &OcrConfig) -> Result<String, VttError> {
    let (bin, prefix) = config
        .command
        .split_first()
        .ok_or_else(|| VttError::Config("ocr.command is empty".into()))?;
    let output = Command::new(bin)
        .args(prefix)
        .arg("--check")
        .output()
        .await
        .map_err(|e| VttError::Config(format!("failed to run OCR command {bin}: {e}")))?;
    if !output.status.success() {
        return Err(VttError::Config(format!(
            "OCR command not usable: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// --- Scoring ---

/// One stated fact and how it fared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatedFact {
    pub key: String,
    pub fact: Fact,
    pub supported: bool,
    /// The on-screen token it matched, if any.
    pub matched: Option<String>,
}

/// One prominent on-screen fact and whether the description mentioned it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceFact {
    pub key: String,
    pub fact: Fact,
    pub mentioned: bool,
    pub persist_secs: f64,
    pub height_px: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentFidelity {
    pub start: String,
    pub end: String,
    pub frames: Vec<String>,
    pub stated: Vec<StatedFact>,
    pub prominent: Vec<ReferenceFact>,
}

/// Full report: summary plus per-segment detail (written to `fidelity.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FidelityReport {
    pub summary: FidelitySummary,
    pub number_tolerance: f64,
    pub min_persist_secs: f64,
    pub min_text_height_px: u32,
    pub segments: Vec<SegmentFidelity>,
}

/// F-beta with beta = 0.5: precision weighted double (van Rijsbergen).
pub fn f05(precision: f64, recall: f64) -> f64 {
    let denom = 0.25 * precision + recall;
    if denom <= 0.0 {
        0.0
    } else {
        1.25 * precision * recall / denom
    }
}

fn ts_key(t: f64) -> String {
    format_timestamp(t)
}

/// Version of the scoring rules themselves. Bump whenever a change would move a
/// figure that was already recorded, so old and new scores never compare silently.
pub const FIDELITY_METRIC_VERSION: u32 = 1;

fn median(sorted: &[f64]) -> f64 {
    match sorted.len() {
        0 => 0.0,
        n if n % 2 == 1 => sorted[n / 2],
        n => (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0,
    }
}

/// Median of `vals` weighted by `wts`: the first value at which cumulative
/// weight reaches half the total. With character counts as weights this lands
/// where the *text* is, not where the segments are.
fn weighted_median(pairs: &[(f64, f64)]) -> f64 {
    if pairs.is_empty() {
        return 0.0;
    }
    let total: f64 = pairs.iter().map(|(_, w)| *w).sum();
    let mut acc = 0.0;
    for (v, w) in pairs {
        acc += *w;
        if acc >= total / 2.0 {
            return *v;
        }
    }
    pairs[pairs.len() - 1].0
}

/// Order-independent digest of the stoplist, so two runs with different prose
/// stoplists do not share a signature.
fn stoplist_digest(stoplist: &[String]) -> u64 {
    let mut acc: u64 = 0;
    for w in stoplist {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in w.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        acc ^= h; // XOR: commutative, so declaration order cannot change it
    }
    acc
}

/// Comparability signature, after sacreBLEU (Post, WMT 2018): every setting that
/// can move a score, in one string, built where the score is built. Two reports
/// with identical signatures are comparable; different signatures are not.
fn signature(config: &FidelityConfig, reference: &str) -> String {
    format!(
        "vtt-fidelity|v:{}|ref:{}|tol:{}|persist:{}|height:{}|stop:{}-{:x}",
        FIDELITY_METRIC_VERSION,
        reference,
        config.number_tolerance,
        config.min_persist_secs,
        config.min_text_height_px,
        config.label_stoplist.len(),
        stoplist_digest(&config.label_stoplist),
    )
}

/// Score every visual segment. `kept` must contain an OCR'd frame for each
/// timestamp listed in the segments' `frames`; `reference` (optional) is the
/// candidate-frame OCR for study mode. Pure apart from the inputs.
pub fn score_segments(
    segments: &[Segment],
    kept: &[OcrFrame],
    reference: Option<&[OcrFrame]>,
    config: &FidelityConfig,
) -> FidelityReport {
    let kept_by_ts: HashMap<String, &OcrFrame> = kept.iter().map(|f| (ts_key(f.timestamp), f)).collect();
    let facts_of: HashMap<String, Vec<OcrFact>> =
        kept.iter().map(|f| (ts_key(f.timestamp), f.facts())).collect();
    let mode = if reference.is_some() { RecallReference::Candidates } else { RecallReference::Kept };
    let mut out = Vec::new();
    let (mut stated_n, mut supported_n, mut prominent_n, mut mentioned_n) = (0, 0, 0, 0);
    let mut chars_total = 0usize;
    let mut yield_pairs: Vec<(f64, f64)> = Vec::new();

    for seg in segments.iter().filter(|s| s.segment_type == SegmentType::Visual) {
        let start = parse_timestamp(&seg.start).unwrap_or(0.0);
        let end = parse_timestamp(&seg.end).unwrap_or(start);
        // What the model saw.
        let seen: Vec<&OcrFrame> = seg.frames.iter().filter_map(|t| kept_by_ts.get(t).copied()).collect();
        let seen_facts: Vec<Fact> = seg
            .frames
            .iter()
            .filter_map(|t| facts_of.get(t))
            .flat_map(|v| v.iter().map(|x| x.fact.clone()))
            .collect();

        let mut stated = Vec::new();
        for fact in extract_facts_with(&seg.content, &config.label_stoplist) {
            let matched = seen_facts
                .iter()
                .find(|s| facts_match(&fact, s, config.number_tolerance))
                .map(|s| match s {
                    Fact::Number { raw, .. } => raw.clone(),
                    Fact::Label { text } | Fact::Timeframe { text } => text.clone(),
                });
            stated.push(StatedFact { key: fact.key(), supported: matched.is_some(), matched, fact });
        }

        // What was on screen: the reference frames in this segment's window.
        let window: Vec<&OcrFrame> = match reference {
            Some(r) => r.iter().filter(|f| f.timestamp >= start && f.timestamp < end).collect(),
            None => seen.clone(),
        };
        let window_span = match (window.first(), window.last()) {
            (Some(a), Some(b)) => (b.timestamp - a.timestamp).max(0.0),
            _ => 0.0,
        };
        // Group occurrences by key.
        let mut occ: BTreeMap<String, (Fact, Vec<f64>, u32)> = BTreeMap::new();
        for f in &window {
            let mut in_frame: HashSet<String> = HashSet::new();
            for of in &f.facts() {
                let k = of.fact.key();
                if !in_frame.insert(k.clone()) {
                    continue;
                }
                let e = occ.entry(k).or_insert_with(|| (of.fact.clone(), Vec::new(), 0));
                e.1.push(f.timestamp);
                e.2 = e.2.max(of.height_px);
            }
        }
        let stated_facts: Vec<&Fact> = stated.iter().map(|s| &s.fact).collect();
        let mut prominent = Vec::new();
        for (key, (fact, times, height)) in occ {
            let span = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                - times.iter().cloned().fold(f64::INFINITY, f64::min);
            let persists = span >= config.min_persist_secs
                || (window_span < config.min_persist_secs && times.len() == window.len());
            if !persists || height < config.min_text_height_px {
                continue;
            }
            let mentioned = stated_facts.iter().any(|s| facts_match(s, &fact, config.number_tolerance));
            prominent.push(ReferenceFact { key, fact, mentioned, persist_secs: span.max(0.0), height_px: height });
        }

        chars_total += seg.content.chars().count();
        if stated.len() >= config.min_facts_for_yield.max(1) {
            yield_pairs.push((
                seg.content.chars().count() as f64 / stated.len() as f64,
                seg.content.chars().count() as f64,
            ));
        }
        stated_n += stated.len();
        supported_n += stated.iter().filter(|s| s.supported).count();
        prominent_n += prominent.len();
        mentioned_n += prominent.iter().filter(|p| p.mentioned).count();
        out.push(SegmentFidelity {
            start: seg.start.clone(),
            end: seg.end.clone(),
            frames: seg.frames.clone(),
            stated,
            prominent,
        });
    }

    let precision = if stated_n > 0 { supported_n as f64 / stated_n as f64 } else { 0.0 };
    let recall = if prominent_n > 0 { mentioned_n as f64 / prominent_n as f64 } else { 0.0 };
    // Yield concentration: how far the text-weighted median chars-per-fact sits
    // above the plain median. Verbose-but-honest output raises both equally and
    // scores ~1.0; text piled into segments that state nothing checkable raises
    // only the weighted one.
    yield_pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let plain: Vec<f64> = yield_pairs.iter().map(|(v, _)| *v).collect();
    let cpf_median = median(&plain);
    let cpf_weighted = weighted_median(&yield_pairs);
    let concentration = if cpf_median > 0.0 { Some(cpf_weighted / cpf_median) } else { None };
    let reference_name = match mode {
        RecallReference::Kept => "kept".to_string(),
        RecallReference::Candidates => "candidates".to_string(),
    };
    let sig = signature(config, &reference_name);
    FidelityReport {
        summary: FidelitySummary {
            reference: reference_name,
            segments: out.len(),
            stated: stated_n,
            supported: supported_n,
            prominent: prominent_n,
            mentioned: mentioned_n,
            precision,
            recall,
            f05: f05(precision, recall),
            ocr_grounded: false,
            signature: sig,
            visual_chars: chars_total,
            chars_per_fact_median: cpf_median,
            chars_per_fact_weighted: cpf_weighted,
            yield_concentration: concentration,
        },
        number_tolerance: config.number_tolerance,
        min_persist_secs: config.min_persist_secs,
        min_text_height_px: config.min_text_height_px,
        segments: out,
    }
}

// --- Thumbnails ---

/// File name of a kept-frame thumbnail: the capture timestamp with `:` as `-`.
pub fn thumbnail_name(timestamp: f64) -> String {
    format!("{}.jpg", format_timestamp(timestamp).replace(':', "-"))
}

/// Write scaled JPEG thumbnails of kept frames beside the results so the
/// diagnostic and the review sheet remain reproducible after the job dir is
/// cleaned. Returns the written paths in input order.
pub async fn write_thumbnails(
    ffmpeg_path: &str,
    frames: &[(f64, PathBuf)],
    out_dir: &Path,
    width: u32,
    quality: u32,
) -> Result<Vec<PathBuf>, VttError> {
    tokio::fs::create_dir_all(out_dir)
        .await
        .map_err(|e| VttError::Ffmpeg(format!("failed to create thumbnail dir: {e}")))?;
    let mut out = Vec::with_capacity(frames.len());
    for (ts, src) in frames {
        let dst = out_dir.join(thumbnail_name(*ts));
        let status = Command::new(ffmpeg_path)
            .args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
            .arg(src)
            .args(["-vf", &format!("scale={width}:-1"), "-q:v", &quality.to_string()])
            .arg(&dst)
            .status()
            .await
            .map_err(|e| VttError::Ffmpeg(format!("failed to run ffmpeg for thumbnail: {e}")))?;
        if !status.success() {
            return Err(VttError::Ffmpeg(format!("ffmpeg thumbnail exited with {status} for {}", src.display())));
        }
        out.push(dst);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(f: &Fact) -> (f64, f64) {
        match f {
            Fact::Number { value, precision, .. } => (*value, *precision),
            _ => panic!("not a number: {f:?}"),
        }
    }

    #[test]
    fn test_extract_numbers_with_units_signs_and_precision() {
        let facts = extract_facts("Market cap fell to $1.66T (-0.25%) from 42,000; volume 706B, 44k, 0.0001671.");
        let by_raw: HashMap<String, &Fact> = facts
            .iter()
            .filter_map(|f| match f { Fact::Number { raw, .. } => Some((raw.clone(), f)), _ => None })
            .collect();
        assert_eq!(num(by_raw["$1.66T"]), (1.66e12, 1e10));
        assert_eq!(num(by_raw["-0.25%"]), (-0.25, 0.01));
        assert!(matches!(by_raw["-0.25%"], Fact::Number { unit: NumberUnit::Percent, .. }));
        assert_eq!(num(by_raw["42,000"]), (42000.0, 1000.0));
        assert_eq!(num(by_raw["706B"]), (706e9, 1e9));
        assert_eq!(num(by_raw["44k"]), (44000.0, 1000.0));
        let (v, p) = num(by_raw["0.0001671"]);
        assert!((v - 0.0001671).abs() < 1e-12 && (p - 1e-7).abs() < 1e-18);
    }

    #[test]
    fn test_frame_ordinals_skipped_and_numeric_keys_canonical() {
        let keys: Vec<String> = extract_facts("**Frame 3 (00:00:27.000):** price 2.614T; frame 4 shows 2,614,000,000,000 and 02.514T").iter().map(|f| f.key()).collect();
        assert!(!keys.contains(&"n:3".to_string()) && !keys.contains(&"n:4".to_string()), "{keys:?}");
        assert_eq!(keys.iter().filter(|k| *k == "n:2614000000000").count(), 1, "2.614T and 2,614,000,000,000 are one fact: {keys:?}");
        assert!(keys.contains(&"n:2514000000000".to_string()), "leading-zero OCR form keys as the true value: {keys:?}");
        assert_eq!(extract_facts("-0.25%")[0].key(), "n:-0.25%");
        assert_eq!(extract_facts("0.0001671")[0].key(), "n:0.0001671");
    }

    #[test]
    fn test_label_stoplist_drops_prose_capitals_only() {
        let stop = vec!["US".to_string(), "THE".to_string()];
        let keys: Vec<String> = extract_facts_with("THE chart in US dollars shows BTC on BITSTAMP", &stop).iter().map(|f| f.key()).collect();
        assert_eq!(keys, vec!["l:BTC", "l:BITSTAMP"]);
        assert!(extract_facts("THE chart").iter().any(|f| f.key() == "l:THE"), "no stoplist by default");
    }

    /// Real OCR line from a TradingView header on this corpus.
    #[test]
    fn test_extract_ohlc_header_values() {
        let facts = extract_facts("Crypto Total Market Cap, $\u{b7} 1W \u{b7}CRYPTOCAP O2.514T H2.632T L2.492T C2.594T +80.784B (+3.21%)");
        let keys: Vec<String> = facts.iter().map(|f| f.key()).collect();
        for k in ["t:1W", "l:CRYPTOCAP", "n:2514000000000", "n:2632000000000", "n:2492000000000", "n:2594000000000", "n:80784000000", "n:3.21%"] {
            assert!(keys.contains(&k.to_string()), "missing {k} in {keys:?}");
        }
        assert!(!keys.iter().any(|k| k.starts_with("l:O2") || k.starts_with("l:C2")), "{keys:?}");
    }

    #[test]
    fn test_extract_timeframes_labels_and_month_rule() {
        let facts = extract_facts("BTC/USD on the 4H and 1D; the 1M and 12M views; 5M volume; RSI at 63.");
        let keys: Vec<String> = facts.iter().map(|f| f.key()).collect();
        assert!(keys.contains(&"l:BTC".to_string()) && keys.contains(&"l:USD".to_string()));
        assert!(keys.contains(&"t:4H".to_string()) && keys.contains(&"t:1D".to_string()));
        assert!(keys.contains(&"t:1M".to_string()) && keys.contains(&"t:12M".to_string()));
        assert!(keys.contains(&"n:5000000".to_string()), "5M is a million, not a month: {keys:?}");
        assert!(keys.contains(&"l:RSI".to_string()));
        assert!(keys.contains(&"n:63".to_string()));
        // lowercase words and single letters are not labels; dates are not extracted
        assert!(extract_facts("Bitcoin rose on Feb 12, 2024 a lot").iter().all(|f| matches!(f, Fact::Number { .. })));
    }

    #[test]
    fn test_precision_rule() {
        let stated = |s: &str| extract_facts(s).remove(0);
        assert!(facts_match(&stated("1.66T"), &stated("1.661T"), 0.0));
        assert!(!facts_match(&stated("1.66T"), &stated("1.67T"), 0.0));
        assert!(facts_match(&stated("42,000"), &stated("41958"), 0.0), "42,000 is stated to the thousand");
        assert!(!facts_match(&stated("42,000"), &stated("43000"), 0.0));
        assert!(!facts_match(&stated("70142"), &stated("70143"), 0.0), "exact integers match exactly");
        assert!(!facts_match(&stated("2020"), &stated("2024"), 0.0), "a year is exact, not 'to the ten'");
        assert!(!facts_match(&stated("1"), &stated("0.883"), 0.0), "a plain 1 is exact");
        assert!(facts_match(&stated("44k"), &stated("44,200"), 0.0), "suffixed integers round to their last digit");
        assert!(facts_match(&stated("\u{2212}6.36%"), &stated("-6.36%"), 0.0), "unicode minus is a sign");
        assert!(facts_match(&stated("70142"), &stated("70143"), 0.001), "unless a tolerance is configured");
        assert!(!facts_match(&stated("10.95%"), &stated("10.95"), 0.0), "percent never matches plain");
        assert!(facts_match(&stated("4H"), &stated("4h"), 0.0));
        assert!(!facts_match(&stated("BTC"), &stated("ETH"), 0.0));
    }

    #[test]
    fn test_parse_ocr_output_heights_and_errors() {
        let out = "{\"path\":\"/f/a.jpg\",\"items\":[{\"text\":\"2.614T\",\"score\":0.98,\"box\":[[10,20],[60,20],[60,38],[10,38]]},{\"text\":\"USD\",\"score\":0.9,\"box\":[[0,0],[5,0],[5,12],[0,12]]}]}\n\
                   {\"path\":\"/f/b.jpg\",\"items\":[],\"error\":\"boom\"}\n";
        let parsed = parse_ocr_output(out).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].1.len(), 2, "raw text regions are kept (PR-024)");
        assert_eq!(parsed[0].1[0].text, "2.614T");
        assert_eq!(parsed[0].1[0].height_px, 18);
        assert_eq!(parsed[0].1[0].x, 10.0);
        assert_eq!(parsed[0].1[0].y, 20.0);
        let f = OcrFrame { timestamp: 0.0, path: "p".into(), items: parsed[0].1.clone() };
        assert_eq!(f.facts()[0].fact.key(), "n:2614000000000");
        assert_eq!(parsed[1].2.as_deref(), Some("boom"));
        assert!(parse_ocr_output("not json\n").is_err());
    }

    fn frame(ts: f64, texts: &[(&str, u32)]) -> OcrFrame {
        OcrFrame {
            timestamp: ts,
            path: format!("/f/{ts}.jpg"),
            items: texts
                .iter()
                .enumerate()
                .map(|(i, (t, h))| OcrItem {
                    text: t.to_string(),
                    score: 0.9,
                    x: 0.0,
                    y: i as f64 * 20.0,
                    height_px: *h,
                })
                .collect(),
        }
    }
    fn visual(start: f64, end: f64, frames: &[f64], content: &str) -> Segment {
        Segment {
            segment_type: SegmentType::Visual,
            start: format_timestamp(start),
            end: format_timestamp(end),
            content: content.to_string(),
            frames: frames.iter().map(|t| format_timestamp(*t)).collect(),
        }
    }

    /// PR-029. Fabricated bulk that states nothing checkable is invisible to
    /// precision by construction; the yield term is what makes it visible.
    /// Both segments below state exactly the facts on screen -- precision is a
    /// perfect 1.0 either way -- but one buries them in 20x the text.
    #[test]
    fn test_yield_concentration_sees_bulk_that_precision_cannot() {
        let cfg = FidelityConfig::default();
        let kept = vec![frame(0.0, &[("BTC", 16), ("42,000", 16)]), frame(15.0, &[("BTC", 16), ("42,000", 16)])];
        let terse = "BTC trades at 42,000.";
        let padded = format!("BTC trades at 42,000.{}", " The presenter draws a line from a point to a point.".repeat(20));
        let lean = score_segments(
            &[visual(0.0, 30.0, &[0.0, 15.0], terse), visual(30.0, 60.0, &[0.0, 15.0], terse)],
            &kept, None, &cfg,
        );
        let bloated = score_segments(
            &[visual(0.0, 30.0, &[0.0, 15.0], terse), visual(30.0, 60.0, &[0.0, 15.0], &padded)],
            &kept, None, &cfg,
        );
        assert_eq!(lean.summary.precision, 1.0);
        assert_eq!(bloated.summary.precision, 1.0, "precision cannot see the padding -- that is the whole premise");
        assert_eq!(bloated.summary.stated, lean.summary.stated, "nor can the fact count");
        assert!(
            bloated.summary.yield_concentration.unwrap() > 1.5
                && lean.summary.yield_concentration.unwrap() <= 1.05,
            "the yield term must: lean={:?} bloated={:?}",
            lean.summary.yield_concentration, bloated.summary.yield_concentration
        );
        assert!(bloated.summary.visual_chars > lean.summary.visual_chars);
    }

    /// PR-029. The trap this statistic exists to avoid: verbose-but-honest output
    /// must NOT be flagged. Measured on the corpus, the general prompt has the
    /// worst absolute chars-per-fact of any arm (310) and a concentration of 0.85,
    /// because its text is spread evenly rather than piled into one dead segment.
    /// A mean-based statistic fails this case; a concentration ratio does not.
    #[test]
    fn test_uniformly_verbose_output_is_not_flagged() {
        let cfg = FidelityConfig::default();
        let kept = vec![frame(0.0, &[("BTC", 16), ("42,000", 16)]), frame(15.0, &[("BTC", 16), ("42,000", 16)])];
        let wordy = format!("BTC trades at 42,000.{}", " The chart is rendered on a dark background with a grid.".repeat(20));
        let r = score_segments(
            &[
                visual(0.0, 30.0, &[0.0, 15.0], &wordy),
                visual(30.0, 60.0, &[0.0, 15.0], &wordy),
                visual(60.0, 90.0, &[0.0, 15.0], &wordy),
            ],
            &kept, None, &cfg,
        );
        assert!(r.summary.chars_per_fact_median > 300.0, "this arm IS verbose: {}", r.summary.chars_per_fact_median);
        assert!(
            r.summary.yield_concentration.unwrap() <= 1.05,
            "uniform verbosity must not be flagged, got {:?}",
            r.summary.yield_concentration
        );
    }

    /// PR-029. What a *text-weighted* median is and is not robust to, pinned
    /// because the distinction decides how the figure may be read.
    ///
    /// Not robust to share of text, deliberately: a segment holding half the
    /// output IS the weighted median, however few segments there are. That is
    /// the property the statistic is built on -- a segment that is most of the
    /// text and states almost nothing is Mode B, and must score high.
    ///
    /// Robust to a minority outlier: the real case is `95f4bc52` vseg 5 -- 3
    /// stated facts at 1,019 chars each, legitimate, and 9.4% of its job's text.
    /// The measured arm scores 0.85, unflagged.
    #[test]
    fn test_yield_term_follows_text_share_not_segment_count() {
        let cfg = FidelityConfig::default();
        let kept = vec![frame(0.0, &[("BTC", 16), ("42,000", 16)]), frame(15.0, &[("BTC", 16), ("42,000", 16)])];
        let terse = "BTC trades at 42,000.";
        let low_yield = format!("BTC at 42,000 on 1D.{}", " Prose with no checkable content at all.".repeat(30));

        // Minority outlier: the low-yield segment is a small share of the text.
        let bulk: String = format!("BTC trades at 42,000.{}", " It moves up and it moves down and it rests.".repeat(30));
        let minority = score_segments(
            &[
                visual(0.0, 30.0, &[0.0, 15.0], &bulk),
                visual(30.0, 60.0, &[0.0, 15.0], &bulk),
                visual(60.0, 90.0, &[0.0, 15.0], &bulk),
                visual(90.0, 120.0, &[0.0, 15.0], &low_yield),
            ],
            &kept, None, &cfg,
        );
        assert!(
            minority.summary.yield_concentration.unwrap() <= 1.05,
            "a low-yield MINORITY of the text must not flag the job, got {:?}",
            minority.summary.yield_concentration
        );

        // Dominant: the same segment, now holding nearly all the text.
        let dominant = score_segments(
            &[
                visual(0.0, 30.0, &[0.0, 15.0], terse),
                visual(30.0, 60.0, &[0.0, 15.0], terse),
                visual(60.0, 90.0, &[0.0, 15.0], terse),
                visual(90.0, 120.0, &[0.0, 15.0], &low_yield),
            ],
            &kept, None, &cfg,
        );
        assert!(
            dominant.summary.yield_concentration.unwrap() > 5.0,
            "a low-yield segment that IS most of the text must flag -- that is Mode B, got {:?}",
            dominant.summary.yield_concentration
        );

        // The denominator gate is configurable, not hardcoded. Raising it drops
        // sub-threshold segments from the statistic and keeps the rest...
        let rich = "BTC at 42,000 on 1D from BITSTAMP with RSI at 55.";
        let kept2 = vec![
            frame(0.0, &[("BTC", 16), ("42,000", 16), ("1D", 14), ("BITSTAMP", 14), ("RSI", 14), ("55", 14)]),
            frame(15.0, &[("BTC", 16), ("42,000", 16), ("1D", 14), ("BITSTAMP", 14), ("RSI", 14), ("55", 14)]),
        ];
        let gated = FidelityConfig { min_facts_for_yield: 4, ..Default::default() };
        let r2 = score_segments(
            &[visual(0.0, 30.0, &[0.0, 15.0], rich), visual(30.0, 60.0, &[0.0, 15.0], &low_yield)],
            &kept2, None, &gated,
        );
        assert!(r2.summary.chars_per_fact_median > 0.0, "the qualifying segment still counts");
        assert!(
            r2.summary.chars_per_fact_median < 100.0,
            "the sub-threshold segment was excluded, so the median is the rich segment's: {}",
            r2.summary.chars_per_fact_median
        );

        // ...and when NO segment qualifies the statistic is empty rather than
        // invented. A reader must be able to tell "not measured" from "1.0".
        let none = score_segments(
            &[visual(0.0, 30.0, &[0.0, 15.0], terse)],
            &kept, None, &FidelityConfig { min_facts_for_yield: 99, ..Default::default() },
        );
        assert_eq!(none.summary.chars_per_fact_median, 0.0);
        assert_eq!(
            none.summary.yield_concentration, None,
            "no qualifying segment means NO figure -- not 0.0, which reads as the best possible score"
        );
        let json = serde_json::to_string(&none.summary).unwrap();
        assert!(!json.contains("yield_concentration"), "unmeasured must be omitted, not serialised: {json}");
    }

    /// PR-029. Two scores are comparable only if their signatures match -- the
    /// sacreBLEU rule. Every setting that can move a figure must change it.
    #[test]
    fn test_signature_changes_with_every_scoring_setting() {
        let base = FidelityConfig::default();
        let kept = vec![frame(0.0, &[("BTC", 16)])];
        let segs = [visual(0.0, 30.0, &[0.0], "BTC at 42,000.")];
        let sig = |c: &FidelityConfig| score_segments(&segs, &kept, None, c).summary.signature;
        let b = sig(&base);
        assert!(b.starts_with("vtt-fidelity|v:"), "{b}");
        assert_eq!(b, sig(&base), "same settings must give the same signature");
        assert_ne!(b, sig(&FidelityConfig { number_tolerance: 0.01, ..base.clone() }));
        assert_ne!(b, sig(&FidelityConfig { min_persist_secs: 9.0, ..base.clone() }));
        assert_ne!(b, sig(&FidelityConfig { min_text_height_px: 22, ..base.clone() }));
        assert_ne!(b, sig(&FidelityConfig { label_stoplist: vec!["ZZZ".into()], ..base.clone() }));
        // Stoplist order is not a setting; declaring the same words differently
        // must not fork the signature and strand otherwise-comparable scores.
        let a1 = FidelityConfig { label_stoplist: vec!["THE".into(), "US".into()], ..base.clone() };
        let a2 = FidelityConfig { label_stoplist: vec!["US".into(), "THE".into()], ..base.clone() };
        assert_eq!(sig(&a1), sig(&a2));
    }

    #[test]
    fn test_score_segments_precision_recall_and_prominence() {
        let cfg = FidelityConfig { min_persist_secs: 5.0, min_text_height_px: 10, ..Default::default() };
        // Two kept frames 15 s apart; TOTAL/USD/1.66T persist; 706B appears once; tiny text ignored.
        let kept = vec![
            frame(0.0, &[("TOTAL", 14), ("USD", 14), ("1.661T", 16), ("706B", 16), ("tiny 99", 6)]),
            frame(15.0, &[("TOTAL", 14), ("USD", 14), ("1.661T", 16), ("1W", 12)]),
        ];
        let seg = visual(0.0, 30.0, &[0.0, 15.0], "The TOTAL market cap chart shows 1.66T in USD, a drop of 3.21%, with ETH listed.");
        let report = score_segments(&[seg], &kept, None, &cfg);
        let s = &report.segments[0];
        let supported: Vec<&str> = s.stated.iter().filter(|f| f.supported).map(|f| f.key.as_str()).collect();
        let unsupported: Vec<&str> = s.stated.iter().filter(|f| !f.supported).map(|f| f.key.as_str()).collect();
        assert_eq!(supported, vec!["l:TOTAL", "n:1660000000000", "l:USD"]);
        assert_eq!(unsupported, vec!["n:3.21%", "l:ETH"], "hallucinated percent and ticker");
        // prominent: TOTAL, USD, 1.661T persist 15 s; 706B (once, 0 s span) and 1W do not; 99 too small
        let prom: Vec<&str> = s.prominent.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(prom, vec!["l:TOTAL", "l:USD", "n:1661000000000"]);
        assert!(s.prominent.iter().all(|p| p.mentioned));
        assert_eq!(report.summary.stated, 5);
        assert_eq!(report.summary.supported, 3);
        assert!((report.summary.precision - 0.6).abs() < 1e-9);
        assert!((report.summary.recall - 1.0).abs() < 1e-9);
        assert_eq!(report.summary.reference, "kept");
    }

    #[test]
    fn test_score_segments_candidates_reference_is_window_not_kept() {
        let cfg = FidelityConfig { min_persist_secs: 5.0, min_text_height_px: 10, ..Default::default() };
        let kept = vec![frame(0.0, &[("BTC", 14)])];
        // Candidate frames every 0.5 s show a level the sampler never kept.
        let cands: Vec<OcrFrame> = (0..20).map(|i| frame(i as f64 * 0.5, &[("BTC", 14), ("65250", 12)])).collect();
        let seg = visual(0.0, 10.0, &[0.0], "BTC chart.");
        let report = score_segments(&[seg], &kept, Some(&cands), &cfg);
        let s = &report.segments[0];
        let missed: Vec<&str> = s.prominent.iter().filter(|p| !p.mentioned).map(|p| p.key.as_str()).collect();
        assert_eq!(missed, vec!["n:65250"], "on-screen level the description omitted");
        assert_eq!(report.summary.reference, "candidates");
        assert!((report.summary.recall - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_candidate_frames_from_output_uniform_times() {
        let out = "{\"path\":\"/c/1.jpg\",\"items\":[]}\n{\"path\":\"/c/2.jpg\",\"items\":[]}\n{\"path\":\"/c/3.jpg\",\"items\":[]}\n";
        let frames = candidate_frames_from_output(out, 300.0, 2.0).unwrap();
        let ts: Vec<f64> = frames.iter().map(|f| f.timestamp).collect();
        assert_eq!(ts, vec![300.0, 300.5, 301.0]);
        assert!(candidate_frames_from_output(out, 0.0, 0.0).is_err());
    }

    #[test]
    fn test_f05_and_thumbnail_name() {
        assert!((f05(1.0, 1.0) - 1.0).abs() < 1e-12);
        assert_eq!(f05(0.0, 0.0), 0.0);
        assert!(f05(0.9, 0.3) > f05(0.3, 0.9), "precision weighs more than recall");
        assert_eq!(thumbnail_name(72.5), "00-01-12.500.jpg");
    }
    #[test]
    fn test_prompt_items_reading_order_confidence_filter_and_cap() {
        let f = OcrFrame {
            timestamp: 0.0,
            path: "p".into(),
            items: vec![
                OcrItem { text: "axis".into(), score: 0.99, x: 900.0, y: 300.0, height_px: 12 },
                OcrItem { text: "header".into(), score: 0.98, x: 10.0, y: 50.0, height_px: 14 },
                OcrItem { text: "left".into(), score: 0.97, x: 5.0, y: 300.0, height_px: 12 },
                OcrItem { text: "junk".into(), score: 0.20, x: 0.0, y: 0.0, height_px: 8 },
                OcrItem { text: "  ".into(), score: 0.99, x: 0.0, y: 10.0, height_px: 8 },
            ],
        };
        let picked: Vec<&str> = f.prompt_items(0.5, 10).iter().map(|i| i.text.as_str()).collect();
        assert_eq!(picked, vec!["header", "left", "axis"], "top-to-bottom then left-to-right; low score and blanks dropped");
        // the cap keeps the most confident, then restores reading order
        let picked: Vec<&str> = f.prompt_items(0.5, 2).iter().map(|i| i.text.as_str()).collect();
        assert_eq!(picked, vec!["header", "axis"]);
        assert!(f.prompt_items(0.995, 10).is_empty());
    }
}
