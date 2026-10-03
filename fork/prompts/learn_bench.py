#!/usr/bin/env python3
"""Bench for learn_prompt.md: does the model propose the right vocabulary/corrections?

  python3 learn_bench.py --from-handy      # endpoint/model/key from Handy's settings
  BENCH_BASE_URL=... BENCH_API_KEY=... BENCH_MODEL=... python3 learn_bench.py

Applies the same acceptance rules as learn.rs (vocabulary must occur in the corrected text; a correction's
"wrong" must occur in the raw text and "right" in the corrected text) before judging each case.
"""
import argparse, json, re, sys
from pathlib import Path
import bench

HERE = Path(__file__).parent


def build(case):
    t = (HERE.parent / "prompts" / "learn_prompt.md").read_text(encoding="utf-8")
    fence = lambda s: s.replace("</", "<​/")
    vals = {"vocabulary": "(none)", "corrections": "(none)", "raw": fence(case["raw"]),
            "pasted": fence(case["pasted"]), "corrected": fence(case["corrected"])}
    return re.sub(r"\{\{(\w+)\}\}", lambda m: vals.get(m.group(1), m.group(0)), t)


def has(hay, needle):
    return re.search(r"(?<!\w)" + re.escape(needle) + r"(?!\w)", hay, re.I) is not None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--from-handy", action="store_true")
    ap.add_argument("--handy-settings", default=bench.DEFAULT_HANDY_SETTINGS)
    ap.add_argument("-v", action="store_true")
    args = ap.parse_args()
    base, key, model = bench.load_endpoint(args)
    cases = json.load(open(HERE / "learn_cases.json", encoding="utf-8"))
    bad = 0
    for c in cases:
        out = bench.call_model(base, key, model, build(c))
        out = re.sub(r"^\s*<think>.*?</think>\s*", "", out, flags=re.S)
        m = re.search(r"\{.*\}", out, re.S)
        try:
            p = json.loads(m.group(0))
        except Exception:
            print(f"FAIL {c['id']}: not JSON: {out[:120]!r}"); bad += 1; continue
        vocab = [v for v in p.get("vocabulary", []) if has(c["corrected"], v)]
        corr = [(x["wrong"], x["right"], bool(x.get("hint"))) for x in p.get("corrections", [])
                if has(c["raw"], x["wrong"]) and has(c["corrected"], x["right"])]
        errs = []
        for v in c["expect_vocab"]:
            if not any(v.lower() == w.lower() for w in vocab): errs.append(f"missing vocab {v}")
        for w, r in c["expect_corr"]:
            if not any(w.lower() == a.lower() and r.lower() == b.lower() for a, b, _ in corr): errs.append(f"missing correction {w}->{r}")
        if not c["expect_vocab"] and vocab: errs.append(f"unexpected vocab {vocab}")
        allowed = {(w.lower(), r.lower()) for w, r in c.get("allow_hint_corr", [])}
        for a, b, hint in corr:
            if not c["expect_corr"] and not (hint and (a.lower(), b.lower()) in allowed):
                errs.append(f"unexpected correction {a}->{b} hint={hint}")
        for f in c.get("forbid", []):
            if any(f.lower() in str(x).lower() for x in vocab + corr): errs.append(f"forbidden {f}")
        print(("FAIL " if errs else "ok   ") + c["id"] + (": " + "; ".join(errs) if errs else ""))
        if args.v: print("     ", json.dumps(p, ensure_ascii=False))
        bad += bool(errs)
    print(f"{len(cases)-bad}/{len(cases)} passed")
    sys.exit(1 if bad else 0)


main()
