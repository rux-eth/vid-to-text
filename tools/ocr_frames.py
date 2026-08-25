#!/usr/bin/env python3
"""OCR wrapper for the vid-to-text fidelity diagnostic (PR-023).

Runs RapidOCR (PP-OCR models on ONNX Runtime, CPU) over image files and prints one
JSON object per line: {"path": ..., "items": [{"text": ..., "score": ..., "box": [[x,y],...]}]}.
Chosen by measurement on the market-research corpus: 76/76 legible price-axis values
recovered vs 42/76 for tesseract 4.1.1 (docs/0.0/DESIGN-log.md, 2026-08-25).

Usage:
  ocr_frames.py --check                 # exit 0 and print the engine version if usable
  ocr_frames.py [--workers N] IMG...    # OCR each image, JSON lines on stdout
"""
import argparse, json, sys
from multiprocessing import Pool

_engine = None
_threads = 2

def _init(threads=None):
    """Load the engine once per worker process. `threads` bounds onnxruntime's
    intra-op pool per session; the default (-1) grabs every core, so parallel
    workers only serialise (measured: 1.85 s/frame flat from 4 to 16 workers)."""
    global _engine
    import logging
    logging.disable(logging.INFO)
    from rapidocr import RapidOCR
    t = threads if threads is not None else _threads
    _engine = RapidOCR(params={
        "EngineConfig.onnxruntime.intra_op_num_threads": int(t),
        "EngineConfig.onnxruntime.inter_op_num_threads": 1,
    })

def _ocr(path):
    try:
        res = _engine(path)
        if hasattr(res, "txts"):  # rapidocr >= 3: RapidOCROutput (fields may be None or numpy arrays)
            txts = res.txts
            if txts is None:
                items = []
            else:
                boxes = [] if res.boxes is None else list(res.boxes)
                scores = [] if res.scores is None else list(res.scores)
                items = []
                for i, t in enumerate(txts):
                    b = boxes[i] if i < len(boxes) else []
                    sc = scores[i] if i < len(scores) else 0.0
                    items.append({"text": str(t), "score": float(sc),
                                  "box": [[float(x), float(y)] for x, y in b]})
        else:  # rapidocr 1.x: (list of [box, text, score], elapse)
            res = res[0] if isinstance(res, tuple) else res
            items = [{"text": r[1], "score": float(r[2]), "box": [[float(x), float(y)] for x, y in r[0]]}
                     for r in (res or [])]
        return {"path": path, "items": items}
    except Exception as e:  # never let one frame kill the batch
        return {"path": path, "items": [], "error": str(e)}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--workers", type=int, default=1)
    ap.add_argument("--threads", type=int, default=2, help="onnxruntime intra-op threads per worker")
    ap.add_argument("images", nargs="*")
    a = ap.parse_args()
    global _threads
    _threads = a.threads
    if a.check:
        try:
            import rapidocr, onnxruntime
            _init()
            print(f"rapidocr {getattr(rapidocr, '__version__', 'ok')} onnxruntime {onnxruntime.__version__}")
            return 0
        except Exception as e:
            print(f"rapidocr unavailable: {e}", file=sys.stderr)
            return 1
    if not a.images:
        return 0
    if a.workers > 1 and len(a.images) > 1:
        with Pool(a.workers, initializer=_init, initargs=(a.threads,)) as pool:
            for rec in pool.imap(_ocr, a.images):
                print(json.dumps(rec), flush=True)
    else:
        _init()
        for p in a.images:
            print(json.dumps(_ocr(p)), flush=True)
    return 0

if __name__ == "__main__":
    sys.exit(main())
