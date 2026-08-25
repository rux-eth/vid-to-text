#!/usr/bin/env python3
"""Before/after comparison for a vision-prompt change. (PR-026)

Reports, per arm: the boilerplate rate, verbosity, and the fidelity figures --
the last as a GUARDRAIL ("did anything collapse"), never as an objective. PR-026's
research found the fidelity metric has never been validated against human judgment,
so it may not be used to rank prompts; and precision alone is improvable by
describing less, which is exactly what a terser prompt does. Both are why this
script always prints `stated` and words-per-segment beside any rate.

The BOILERPLATE_PATTERNS below were fixed BEFORE the first comparison run, so the
regex cannot be tuned to flatter a result. They transcribe the categories measured
on 13 corpus videos on 2026-08-25 and recorded in the PR's Motivation table.

Usage:
  prompt_ab.py --results <server results dir> --manifest <clip,profile,job tsv>
"""
import argparse, json, os, re, sys
from collections import defaultdict

# --- FIXED BEFORE THE FIRST RUN. Do not tune these to a result. ---
BOILERPLATE_PATTERNS = {
    "absent_humans": re.compile(
        r"\bno\s+(?:visible\s+)?(?:characters|people|persons|human figures|humans|faces)\b"
        r"|\bthere are no (?:characters|people|human)\b"
        r"|\bno human (?:figures|presence|subjects)\b", re.I),
    "nothing_changed": re.compile(
        r"\bremains? (?:unchanged|consistent|the same|in place|static|identical)\b"
        r"|\bno (?:significant |notable )?changes?\b"
        r"|\bunchanged (?:from|across|between)\b", re.I),
    "no_transitions": re.compile(
        r"\bno (?:scene )?(?:transitions|cuts|fades)\b"
        r"|\bno camera (?:angles?|movements?|work)\b"
        r"|\bthe (?:shot|camera) (?:holds|remains) steady\b", re.I),
    "no_expressions": re.compile(
        r"\bno (?:facial )?expressions?\b|\bno body language\b|\bno gestures\b", re.I),
    "summary_filler": re.compile(r"(?:^|\n)\s*(?:in summary|to summari[sz]e|overall,)", re.I),
}

def visual_segments(timeline):
    return [s for s in timeline["segments"] if s["type"] == "visual"]

def arm_stats(job_dir):
    tl = json.load(open(os.path.join(job_dir, "timeline.json")))
    segs = visual_segments(tl)
    n = len(segs)
    words = [len(s["content"].split()) for s in segs]
    hits = {k: 0 for k in BOILERPLATE_PATTERNS}
    for s in segs:
        for k, rx in BOILERPLATE_PATTERNS.items():
            if rx.search(s["content"]):
                hits[k] += 1
    any_hit = sum(1 for s in segs
                  if any(rx.search(s["content"]) for rx in BOILERPLATE_PATTERNS.values()))
    cap = (tl.get("capture") or {})
    out = dict(
        segments=n,
        words=sum(words),
        words_per_seg=(sum(words) / n if n else 0.0),
        boilerplate_rate=(any_hit / n if n else 0.0),
        per_pattern={k: (v / n if n else 0.0) for k, v in hits.items()},
        prompt=cap.get("vision_prompt"),
        prompt_sha=(cap.get("vision_prompt_sha256") or "")[:16],
    )
    fid_path = os.path.join(job_dir, "fidelity.json")
    if os.path.exists(fid_path):
        f = json.load(open(fid_path))["summary"]
        out.update(stated=f["stated"], supported=f["supported"],
                   precision=f["precision"], recall=f["recall"],
                   prominent=f["prominent"], mentioned=f["mentioned"])
    return out

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--manifest", required=True)
    a = ap.parse_args()
    rows = [l.rstrip("\n").split("\t") for l in open(a.manifest) if "\t" in l]
    rows = [r for r in rows if len(r) >= 5 and r[4] == "completed"]
    by = {}
    for clip, prof, job, wall, _ in rows:
        d = os.path.join(a.results, job)
        if not os.path.isdir(d):
            print(f"  (missing results dir for {clip}/{prof}: {job})", file=sys.stderr); continue
        st = arm_stats(d); st["wall"] = int(wall); by[(clip, prof)] = st

    clips = sorted({k[0] for k in by}); profs = sorted({k[1] for k in by})
    print(f"{'clip':<12}{'profile':<18}{'segs':>5}{'w/seg':>8}{'boiler':>8}"
          f"{'stated':>8}{'prec':>7}{'recall':>8}{'wall':>7}  prompt")
    for clip in clips:
        for prof in profs:
            s = by.get((clip, prof))
            if not s: continue
            print(f"{clip:<12}{prof:<18}{s['segments']:>5}{s['words_per_seg']:>8.0f}"
                  f"{s['boilerplate_rate']:>8.1%}{s.get('stated',0):>8}"
                  f"{s.get('precision',0):>7.3f}{s.get('recall',0):>8.3f}{s['wall']:>7}"
                  f"  {os.path.basename(s['prompt'] or '?')} {s['prompt_sha']}")
        print()

    # Pooled, and the honesty check the research requires.
    print("pooled by arm:")
    for prof in profs:
        ss = [v for k, v in by.items() if k[1] == prof]
        if not ss: continue
        n = sum(s["segments"] for s in ss); w = sum(s["words"] for s in ss)
        st = sum(s.get("stated", 0) for s in ss); su = sum(s.get("supported", 0) for s in ss)
        pr = sum(s.get("prominent", 0) for s in ss); me = sum(s.get("mentioned", 0) for s in ss)
        boiler = sum(s["boilerplate_rate"] * s["segments"] for s in ss) / n if n else 0
        print(f"  {prof:<18} segs {n:>4}  w/seg {w/n:>6.0f}  boilerplate {boiler:>6.1%}  "
              f"stated {st:>5}  precision {(su/st if st else 0):.3f}  recall {(me/pr if pr else 0):.3f}")
    print()
    print("per-pattern boilerplate rate (share of segments matching each):")
    for prof in profs:
        ss = [v for k, v in by.items() if k[1] == prof]
        if not ss: continue
        n = sum(s["segments"] for s in ss)
        agg = {k: sum(s["per_pattern"][k] * s["segments"] for s in ss) / n for k in BOILERPLATE_PATTERNS}
        print(f"  {prof:<18}" + "  ".join(f"{k}={v:.1%}" for k, v in agg.items()))
    print()
    print("NOTE: precision/recall are a GUARDRAIL, not an objective. The metric has no")
    print("human-agreement calibration (docs/ARCHITECTURE.md), and precision is improvable")
    print("by stating fewer numbers -- so read it next to `stated`, never alone.")

if __name__ == "__main__":
    main()
