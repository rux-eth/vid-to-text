//! `vid-to-text review` (PR-023): render a self-contained HTML review sheet for a
//! job's fidelity diagnostic, and score a labels file against the metric.
//!
//! The sheet shows, per visual segment, a few of its source-frame thumbnails, the
//! description, and the facts the metric judged -- sampled disagreement-first so
//! an hour of human judgment lands where the metric is least certain. Judgments
//! are copied out as JSON and fed back with `--labels` to compute agreement.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};
use vtt_core::{Fact, FidelityReport, Timeline};

/// One item put in front of the reviewer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: String,
    pub segment: usize,
    /// "stated" (the description said it) or "missed" (on screen, not mentioned).
    pub kind: String,
    pub key: String,
    pub display: String,
    /// "supported" | "unsupported" | "missed"
    pub metric: String,
    pub matched: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LabelFile {
    pub items: Vec<Label>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Label {
    pub id: String,
    pub kind: String,
    pub metric: String,
    /// "yes" | "no" | "unsure" -- for stated facts: on screen?; for missed
    /// facts: should it have been mentioned?
    pub human: String,
}

fn fact_display(f: &Fact) -> String {
    match f {
        Fact::Number { raw, .. } => raw.clone(),
        Fact::Label { text } | Fact::Timeframe { text } => text.clone(),
    }
}

/// Deterministic shuffle (LCG) so the same job yields the same sheet.
fn shuffle<T>(v: &mut Vec<T>, seed: u64) {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    for i in (1..v.len()).rev() {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = (x >> 33) as usize % (i + 1);
        v.swap(i, j);
    }
}

/// Disagreement-first sample. Composition, in priority order and each shuffled
/// deterministically: (1) every stated fact the metric called unsupported — the
/// hallucination calls, which precision (the gate) depends on; (2) supported
/// stated facts until two thirds of the sample is filled, so κ has both classes;
/// (3) missed prominent facts to fill the rest. `per_segment` caps any one
/// segment so a degenerate one (a 568-number counting loop on this corpus)
/// cannot fill the sheet.
pub fn select_items(report: &FidelityReport, sample: usize, seed: u64) -> Vec<ReviewItem> {
    select_items_capped(report, sample, seed, usize::MAX)
}

pub fn select_items_capped(report: &FidelityReport, sample: usize, seed: u64, per_segment: usize) -> Vec<ReviewItem> {
    let mut unsupported = Vec::new();
    let mut supported = Vec::new();
    let mut missed = Vec::new();
    for (si, seg) in report.segments.iter().enumerate() {
        for f in &seg.stated {
            let item = ReviewItem {
                id: format!("s{si}:{}", f.key),
                segment: si,
                kind: "stated".into(),
                key: f.key.clone(),
                display: fact_display(&f.fact),
                metric: if f.supported { "supported".into() } else { "unsupported".into() },
                matched: f.matched.clone(),
            };
            if f.supported { supported.push(item) } else { unsupported.push(item) }
        }
        for p in seg.prominent.iter().filter(|p| !p.mentioned) {
            missed.push(ReviewItem {
                id: format!("m{si}:{}", p.key),
                segment: si,
                kind: "missed".into(),
                key: p.key.clone(),
                display: fact_display(&p.fact),
                metric: "missed".into(),
                matched: None,
            });
        }
    }
    shuffle(&mut unsupported, seed);
    shuffle(&mut supported, seed ^ 0x9e37_79b9);
    shuffle(&mut missed, seed ^ 0x7f4a_7c15);
    let mut out: Vec<ReviewItem> = Vec::new();
    let mut per_seg: HashMap<usize, usize> = HashMap::new();
    let mut take = |items: Vec<ReviewItem>, limit: usize, out: &mut Vec<ReviewItem>| {
        for item in items {
            if out.len() >= limit {
                break;
            }
            let n = per_seg.entry(item.segment).or_insert(0);
            if *n < per_segment {
                *n += 1;
                out.push(item);
            }
        }
    };
    take(unsupported, sample, &mut out);
    take(supported, sample * 2 / 3, &mut out);
    take(missed, sample, &mut out);
    out
}

/// Cohen's kappa between the metric's verdict on stated facts (supported /
/// unsupported) and the human's (on screen: yes / no). "unsure" is excluded.
pub fn cohen_kappa(pairs: &[(bool, bool)]) -> Option<f64> {
    let n = pairs.len();
    if n == 0 {
        return None;
    }
    let both_yes = pairs.iter().filter(|(a, b)| *a && *b).count() as f64;
    let both_no = pairs.iter().filter(|(a, b)| !*a && !*b).count() as f64;
    let a_yes = pairs.iter().filter(|(a, _)| *a).count() as f64;
    let b_yes = pairs.iter().filter(|(_, b)| *b).count() as f64;
    let n = n as f64;
    let po = (both_yes + both_no) / n;
    let pe = (a_yes / n) * (b_yes / n) + ((n - a_yes) / n) * ((n - b_yes) / n);
    if (1.0 - pe).abs() < 1e-12 {
        return Some(1.0);
    }
    Some((po - pe) / (1.0 - pe))
}

/// Agreement summary from a labels file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Agreement {
    pub labelled_stated: usize,
    pub kappa: Option<f64>,
    pub metric_precision: f64,
    pub human_precision: f64,
    /// metric said unsupported, human said on screen: OCR or matching misses.
    pub false_hallucinations: usize,
    /// metric said supported, human said not on screen: matches to the wrong number.
    pub missed_hallucinations: usize,
    pub labelled_missed: usize,
    /// share of "missed" facts the human says should have been mentioned.
    pub missed_that_matter: f64,
    /// Item ids the human overruled -- the cases that calibrate the matching rules.
    pub disagreements: Vec<String>,
}

pub fn score_labels(labels: &LabelFile) -> Agreement {
    let mut pairs = Vec::new();
    let (mut fh, mut mh) = (0, 0);
    let (mut lm, mut matter) = (0, 0);
    let mut disagreements = Vec::new();
    for l in &labels.items {
        if l.human == "unsure" {
            continue;
        }
        let human_yes = l.human == "yes";
        match l.kind.as_str() {
            "stated" => {
                let metric_yes = l.metric == "supported";
                pairs.push((metric_yes, human_yes));
                if metric_yes != human_yes {
                    disagreements.push(l.id.clone());
                }
                if !metric_yes && human_yes { fh += 1 }
                if metric_yes && !human_yes { mh += 1 }
            }
            "missed" => {
                lm += 1;
                if human_yes { matter += 1 }
            }
            _ => {}
        }
    }
    let n = pairs.len().max(1) as f64;
    Agreement {
        labelled_stated: pairs.len(),
        kappa: cohen_kappa(&pairs),
        metric_precision: pairs.iter().filter(|(m, _)| *m).count() as f64 / n,
        human_precision: pairs.iter().filter(|(_, h)| *h).count() as f64 / n,
        false_hallucinations: fh,
        missed_hallucinations: mh,
        labelled_missed: lm,
        missed_that_matter: if lm > 0 { matter as f64 / lm as f64 } else { 0.0 },
        disagreements,
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Pick up to `n` thumbnails spread across the segment's frames.
fn pick_thumbs(frames: &[String], n: usize) -> Vec<&String> {
    if frames.len() <= n {
        return frames.iter().collect();
    }
    (0..n).map(|i| &frames[i * (frames.len() - 1) / (n - 1).max(1)]).collect()
}

/// Render the sheet. `thumbs` maps thumbnail file name to JPEG bytes.
pub fn render_html(
    job: &str,
    timeline: &Timeline,
    report: &FidelityReport,
    items: &[ReviewItem],
    thumbs: &HashMap<String, Vec<u8>>,
    thumbs_per_segment: usize,
) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut by_seg: HashMap<usize, Vec<&ReviewItem>> = HashMap::new();
    for it in items {
        by_seg.entry(it.segment).or_default().push(it);
    }
    let s = &report.summary;
    let mut h = String::new();
    h.push_str(&format!("<!doctype html><html><head><meta charset=\"utf-8\"><title>Fidelity review {}</title>\
<style>body{{font-family:system-ui,sans-serif;margin:24px;max-width:1100px;background:#fafafa;color:#111}}\
.seg{{border:1px solid #ccc;border-radius:8px;padding:14px;margin:18px 0;background:#fff}}\
.thumbs img{{height:150px;margin:2px;border:1px solid #ddd}} .desc{{white-space:pre-wrap;font-size:13px;color:#333;max-height:14em;overflow:auto;border-left:3px solid #ddd;padding-left:8px}}\
table{{border-collapse:collapse;width:100%;margin-top:8px}} td,th{{border-bottom:1px solid #eee;padding:4px 6px;font-size:13px;text-align:left}}\
.m-supported{{color:#1a7f37}} .m-unsupported{{color:#b3261e;font-weight:600}} .m-missed{{color:#8a5a00;font-weight:600}}\
textarea{{width:100%;height:120px}} .sticky{{position:sticky;top:0;background:#fafafa;padding:8px 0;border-bottom:1px solid #ddd}}</style></head><body>\
<div class=\"sticky\"><b>Fidelity review</b> job {} &middot; {} &middot; {} items ({} stated, {} missed) &middot; metric: precision {:.3} recall {:.3} F0.5 {:.3} ({})\
 &nbsp; <button onclick=\"collect()\">Copy labels JSON</button> <span id=\"done\"></span></div>\
<p>For each <b>stated</b> fact: is it on screen in the frames shown? For each <b>missed</b> fact: should the description have mentioned it? Pick <i>unsure</i> freely.</p>",
        esc(job), esc(job), esc(&timeline.source), items.len(),
        items.iter().filter(|i| i.kind == "stated").count(), items.iter().filter(|i| i.kind == "missed").count(),
        s.precision, s.recall, s.f05, s.reference));
    let mut segs: Vec<usize> = by_seg.keys().copied().collect();
    segs.sort();
    for si in segs {
        let seg = &report.segments[si];
        h.push_str(&format!("<div class=\"seg\"><h3>Segment {} &middot; {} &rarr; {} &middot; {} frames</h3><div class=\"thumbs\">", si, esc(&seg.start), esc(&seg.end), seg.frames.len()));
        for t in pick_thumbs(&seg.frames, thumbs_per_segment) {
            let name = format!("{}.jpg", t.replace(':', "-"));
            if let Some(bytes) = thumbs.get(&name) {
                h.push_str(&format!("<img title=\"{}\" src=\"data:image/jpeg;base64,{}\">", esc(t), b64.encode(bytes)));
            }
        }
        h.push_str("</div>");
        if let Some(desc) = timeline.segments.iter().find(|x| x.start == seg.start && x.end == seg.end && !x.frames.is_empty()) {
            h.push_str(&format!("<div class=\"desc\">{}</div>", esc(&desc.content)));
        }
        h.push_str("<table><tr><th>fact</th><th>kind</th><th>metric</th><th>matched</th><th>your judgment</th></tr>");
        for it in &by_seg[&si] {
            let q = if it.kind == "stated" { "on screen?" } else { "should be mentioned?" };
            h.push_str(&format!("<tr data-id=\"{id}\" data-kind=\"{kind}\" data-metric=\"{metric}\"><td><code>{disp}</code></td><td>{kind}</td><td class=\"m-{metric}\">{metric}</td><td>{matched}</td>\
<td>{q} <label><input type=\"radio\" name=\"{id}\" value=\"yes\">yes</label> <label><input type=\"radio\" name=\"{id}\" value=\"no\">no</label> <label><input type=\"radio\" name=\"{id}\" value=\"unsure\">unsure</label></td></tr>",
                id = esc(&it.id), kind = it.kind, metric = it.metric, disp = esc(&it.display),
                matched = esc(it.matched.as_deref().unwrap_or("")), q = q));
        }
        h.push_str("</table></div>");
    }
    h.push_str(&format!("<h3>Labels</h3><textarea id=\"out\" placeholder=\"Click 'Copy labels JSON' when done\"></textarea>\
<script>function collect(){{const items=[];document.querySelectorAll('tr[data-id]').forEach(tr=>{{const c=tr.querySelector('input:checked');if(c)items.push({{id:tr.dataset.id,kind:tr.dataset.kind,metric:tr.dataset.metric,human:c.value}});}});\
const j=JSON.stringify({{job:{job:?},items}},null,1);document.getElementById('out').value=j;document.getElementById('done').textContent=items.length+' labelled';\
if(navigator.clipboard)navigator.clipboard.writeText(j).catch(()=>{{}});}}</script></body></html>", job = job));
    h
}

/// `vid-to-text rescore`: recompute a job's fidelity report offline from its
/// persisted kept-frame OCR (`ocr.json`), optionally against a candidates
/// reference (raw wrapper output for uniformly spaced frames) and with
/// different matching parameters. Prints the summary as JSON; with `--write`
/// stores the full report beside the job's results.
pub fn run_rescore(
    job_dir: &Path,
    candidates: Option<&Path>,
    candidate_start: f64,
    candidate_fps: f64,
    tolerance: Option<f64>,
    min_persist: Option<f64>,
    min_height: Option<u32>,
    write: Option<&Path>,
) -> Result<(), String> {
    let timeline: Timeline = serde_json::from_str(
        &std::fs::read_to_string(job_dir.join("timeline.json")).map_err(|e| format!("timeline.json: {e}"))?,
    )
    .map_err(|e| format!("timeline.json: {e}"))?;
    let kept: Vec<vtt_core::OcrFrame> = serde_json::from_str(
        &std::fs::read_to_string(job_dir.join("ocr.json")).map_err(|e| format!("ocr.json (written by servers with the fidelity diagnostic enabled): {e}"))?,
    )
    .map_err(|e| format!("ocr.json: {e}"))?;
    let reference = match candidates {
        Some(p) => {
            let raw = std::fs::read_to_string(p).map_err(|e| format!("candidates: {e}"))?;
            Some(vtt_core::candidate_frames_from_output(&raw, candidate_start, candidate_fps).map_err(|e| e.to_string())?)
        }
        None => None,
    };
    let mut cfg = vtt_core::FidelityConfig::default();
    if let Some(t) = tolerance { cfg.number_tolerance = t; }
    if let Some(m) = min_persist { cfg.min_persist_secs = m; }
    if let Some(h) = min_height { cfg.min_text_height_px = h; }
    let mut report = vtt_core::score_segments(&timeline.segments, &kept, reference.as_deref(), &cfg);
    // score_segments cannot know whether the vision prompt was OCR-grounded, and a
    // re-score that silently reported `false` would drop the circularity warning
    // the original run recorded. Carry it forward from the stored timeline.
    if let Some(prior) = timeline.fidelity.as_ref() {
        report.summary.ocr_grounded = prior.ocr_grounded;
    }
    println!("{}", serde_json::to_string_pretty(&report.summary).map_err(|e| e.to_string())?);
    if let Some(out) = write {
        std::fs::write(out, serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?)
            .map_err(|e| format!("failed to write {}: {e}", out.display()))?;
    }
    Ok(())
}

/// Entry point. With `labels`, prints agreement; otherwise writes the sheet.
pub fn run_review(job_dir: &Path, output: Option<&Path>, sample: usize, per_segment: usize, thumbs_per_segment: usize, labels: Option<&Path>) -> Result<(), String> {
    let job = std::fs::canonicalize(job_dir)
        .ok()
        .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
        .unwrap_or_else(|| "job".into());
    if let Some(lp) = labels {
        let text = std::fs::read_to_string(lp).map_err(|e| format!("failed to read labels: {e}"))?;
        let labels: LabelFile = serde_json::from_str(&text).map_err(|e| format!("bad labels file: {e}"))?;
        let a = score_labels(&labels);
        println!("{}", serde_json::to_string_pretty(&a).map_err(|e| e.to_string())?);
        return Ok(());
    }
    let timeline: Timeline = serde_json::from_str(
        &std::fs::read_to_string(job_dir.join("timeline.json")).map_err(|e| format!("timeline.json: {e}"))?,
    )
    .map_err(|e| format!("timeline.json: {e}"))?;
    let report: FidelityReport = serde_json::from_str(
        &std::fs::read_to_string(job_dir.join("fidelity.json")).map_err(|e| format!("fidelity.json (is the fidelity diagnostic enabled on the server?): {e}"))?,
    )
    .map_err(|e| format!("fidelity.json: {e}"))?;
    let mut thumbs = HashMap::new();
    if let Ok(rd) = std::fs::read_dir(job_dir.join("frames")) {
        for e in rd.flatten() {
            if let Ok(bytes) = std::fs::read(e.path()) {
                thumbs.insert(e.file_name().to_string_lossy().to_string(), bytes);
            }
        }
    }
    let seed = job.bytes().fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
    let items = select_items_capped(&report, sample, seed, per_segment);
    let html = render_html(&job, &timeline, &report, &items, &thumbs, thumbs_per_segment);
    let out: PathBuf = output.map(|p| p.to_path_buf()).unwrap_or_else(|| job_dir.join("review.html"));
    std::fs::write(&out, html).map_err(|e| format!("failed to write {}: {e}", out.display()))?;
    eprintln!("wrote {} ({} items, {} thumbnails available)", out.display(), items.len(), thumbs.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vtt_core::{FidelitySummary, ReferenceFact, SegmentFidelity, StatedFact};

    fn report() -> FidelityReport {
        let n = |raw: &str| Fact::Number { value: 1.0, unit: vtt_core::NumberUnit::Plain, precision: 1.0, raw: raw.into() };
        let stated = |raw: &str, ok: bool| StatedFact { key: format!("n:{raw}"), fact: n(raw), supported: ok, matched: ok.then(|| raw.to_string()) };
        FidelityReport {
            summary: FidelitySummary { reference: "kept".into(), segments: 2, stated: 4, supported: 2, prominent: 1, mentioned: 0, precision: 0.5, recall: 0.0, f05: 0.0, ocr_grounded: false, ..Default::default() },
            number_tolerance: 0.0, min_persist_secs: 5.0, min_text_height_px: 10,
            segments: vec![
                SegmentFidelity { start: "00:00:00.000".into(), end: "00:01:00.000".into(), frames: vec!["00:00:00.000".into()],
                    stated: vec![stated("100", true), stated("200", false)],
                    prominent: vec![ReferenceFact { key: "n:300".into(), fact: n("300"), mentioned: false, persist_secs: 10.0, height_px: 12 }] },
                SegmentFidelity { start: "00:01:00.000".into(), end: "00:02:00.000".into(), frames: vec!["00:01:00.000".into()],
                    stated: vec![stated("400", true), stated("500", false)], prominent: vec![] },
            ],
        }
    }

    #[test]
    fn test_select_items_disagreement_first_and_deterministic() {
        let r = report();
        let a = select_items(&r, 3, 7);
        let b = select_items(&r, 3, 7);
        assert_eq!(a, b, "same seed, same sheet");
        assert_eq!(a.iter().filter(|i| i.metric == "unsupported").count(), 2, "all hallucination calls first: {a:?}");
        let all = select_items(&r, 100, 7);
        assert_eq!(all.len(), 5);
        assert_eq!(all.iter().filter(|i| i.kind == "missed").count(), 1);
        // supported facts stop at two thirds of the sample so missed ones get a slice
        let six = select_items(&r, 6, 7);
        assert!(six.iter().any(|i| i.kind == "missed"), "{six:?}");
        let capped = select_items_capped(&r, 100, 7, 1);
        assert_eq!(capped.len(), 2, "one item per segment: {capped:?}");
    }

    #[test]
    fn test_cohen_kappa_and_label_scoring() {
        assert_eq!(cohen_kappa(&[]), None);
        assert!((cohen_kappa(&[(true, true), (false, false)]).unwrap() - 1.0).abs() < 1e-9);
        assert!(cohen_kappa(&[(true, false), (false, true)]).unwrap() < 0.0);
        let labels = LabelFile { items: vec![
            Label { id: "a".into(), kind: "stated".into(), metric: "supported".into(), human: "yes".into() },
            Label { id: "b".into(), kind: "stated".into(), metric: "unsupported".into(), human: "yes".into() },
            Label { id: "c".into(), kind: "stated".into(), metric: "supported".into(), human: "no".into() },
            Label { id: "d".into(), kind: "stated".into(), metric: "unsupported".into(), human: "unsure".into() },
            Label { id: "e".into(), kind: "missed".into(), metric: "missed".into(), human: "yes".into() },
            Label { id: "f".into(), kind: "missed".into(), metric: "missed".into(), human: "no".into() },
        ]};
        let a = score_labels(&labels);
        assert_eq!(a.labelled_stated, 3);
        assert_eq!(a.false_hallucinations, 1);
        assert_eq!(a.missed_hallucinations, 1);
        assert_eq!(a.labelled_missed, 2);
        assert!((a.missed_that_matter - 0.5).abs() < 1e-9);
        assert_eq!(a.disagreements, vec!["b".to_string(), "c".to_string()]);
    }

    #[test]
    fn test_render_html_contains_items_and_thumbnails() {
        let r = report();
        let t = Timeline { source: "v.mp4".into(), duration_seconds: 120.0, segments: vec![], capture: None, fidelity: None };
        let items = select_items(&r, 10, 1);
        let mut thumbs = HashMap::new();
        thumbs.insert("00-00-00.000.jpg".to_string(), vec![0xFF, 0xD8, 0xFF]);
        let html = render_html("job1", &t, &r, &items, &thumbs, 4);
        assert!(html.contains("data-id=\"s0:n:200\""));
        assert!(html.contains("data:image/jpeg;base64,/9j/"));
        assert!(html.contains("Copy labels JSON"));
        assert_eq!(pick_thumbs(&["a".into(), "b".into(), "c".into(), "d".into(), "e".into()], 3).len(), 3);
    }
}
