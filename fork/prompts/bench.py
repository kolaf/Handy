#!/usr/bin/env python3
"""Prompt test bench for the Handy post-processing prompts.

Runs test dictations through the prompts the way Handy does and checks the final text:
  1. fill in ${vocabulary} / ${snippets} / ${clipboard} / ${examples}, then ${output}
  2. send ONE user message to /chat/completions (no extra parameters)
  3. strip <think> blocks, collect [[vocab: X]] tags, expand [[snippet: NAME]]
  4. evaluate the checks of each case against the resulting text

Usage:
  python3 bench.py --from-handy                 # use endpoint/model/key from Handy's settings
  BENCH_BASE_URL=... BENCH_API_KEY=... BENCH_MODEL=... python3 bench.py
  python3 bench.py --prompt email --case spell_exe -v
  python3 bench.py --dry            # print assembled prompts, no network
  python3 bench.py --mock           # use each case's "mock" output (tests the checks only)

Standard library only. The key is read from the environment or Handy's settings file and is
never printed.
"""
import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).parent
def default_settings_path() -> str:
    """Where Handy keeps settings_store.json on this platform (portable installs: Data/ next to the exe)."""
    if os.name == "nt":
        return os.path.join(os.environ.get("APPDATA", ""), "com.pais.handy", "settings_store.json")
    wsl = [p for p in Path("/mnt/c/Users").glob("*/AppData/Roaming/com.pais.handy/settings_store.json")] \
        if Path("/mnt/c/Users").is_dir() else []
    if wsl:
        return str(wsl[0])
    return str(Path.home() / ".local/share/com.pais.handy/settings_store.json")


DEFAULT_HANDY_SETTINGS = default_settings_path()
MAX_CLIPBOARD = 6000
VAR = re.compile(r"\$\{(vocabulary|snippets|clipboard|examples)\}")
VOCAB_TAG = re.compile(r"\[\[\s*vocab\s*:([^\]\n]*)\]\]")
SNIPPET_TAG = re.compile(r"\[\[\s*snippet\s*:([^\]\n]*)\]\]")
THINK = re.compile(r"^\s*<think>.*?</think>\s*", re.S)


def examples_block(examples: str) -> str:
    ex = examples.strip()
    return "(no examples)" if not ex else "<examples>\n" + ex.replace("${output}", "$ {output}") + "\n</examples>"


def clipboard_for_prompt(text: str) -> str:
    t = text.strip()
    if not t:
        return "(the clipboard is empty)"
    return t[:MAX_CLIPBOARD].replace("${output}", "$ {output}")


def assemble(prompt: dict, case: dict) -> str:
    """Build the text Handy sends: variables, appended examples, then ${output}."""
    template = prompt["prompt"]
    examples = prompt.get("examples", "")
    vocab = case.get("vocabulary", [])
    snippet_names = list(case.get("snippets", {}).keys())

    def sub(m):
        name = m.group(1)
        if name == "vocabulary":
            return ", ".join(vocab) or "(none yet)"
        if name == "snippets":
            return ", ".join(snippet_names) or "(none)"
        if name == "examples":
            return examples_block(examples)
        return clipboard_for_prompt(case.get("clipboard", ""))

    expanded = VAR.sub(sub, template)
    if examples.strip() and "${examples}" not in template:
        expanded += (
            "\n\nExamples of the expected result (a dictation, then what the output should be). "
            "Follow their structure and style:\n" + examples_block(examples)
        )
    return expanded.replace("${output}", case["input"])


def finish(raw: str, case: dict):
    """Handy's steps after the model: strip think block, vocab tags, snippet expansion."""
    text = THINK.sub("", raw)
    vocab = []
    for m in VOCAB_TAG.finditer(text):
        w = m.group(1).strip().strip("\"'").strip()
        if w and len(w) <= 64 and not re.search(r"[\[\]\x00-\x1f]", w) and \
                not any(v.lower() == w.lower() for v in vocab) and len(vocab) < 5:
            vocab.append(w)
    text = VOCAB_TAG.sub("", text).rstrip()
    snippets = case.get("snippets", {})

    def expand(m):
        name = m.group(1).strip().lower()
        for k, v in snippets.items():
            if k.lower() == name:
                return v
        return ""

    text = SNIPPET_TAG.sub(expand, text)
    return text.strip(), vocab


def run_checks(text: str, vocab: list, checks: list):
    failures = []
    for check in checks:
        (kind, arg), = check.items()
        low = text.lower()
        ok = {
            "equals": lambda: text == arg,
            "contains": lambda: arg in text,
            "icontains": lambda: arg.lower() in low,
            "not_contains": lambda: arg not in text,
            "not_icontains": lambda: arg.lower() not in low,
            "regex": lambda: re.search(arg, text, re.M | re.I) is not None,
            "not_regex": lambda: re.search(arg, text, re.M | re.I) is None,
            "vocab": lambda: sorted(v.lower() for v in vocab) == sorted(a.lower() for a in arg),
            "max_words": lambda: len(text.split()) <= arg,
            "min_words": lambda: len(text.split()) >= arg,
        }.get(kind)
        if ok is None:
            failures.append(f"unknown check '{kind}'")
        elif not ok():
            failures.append(f"{kind}: {arg!r}")
    return failures


def call_model(base_url: str, key: str, model: str, content: str) -> str:
    url = base_url.rstrip("/") + "/chat/completions"
    body = json.dumps({"model": model, "messages": [{"role": "user", "content": content}], "stream": False}).encode()
    req = urllib.request.Request(url, body, {"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            data = json.load(resp)
    except urllib.error.HTTPError as err:
        raise SystemExit(f"HTTP {err.code} from {url}: {err.read()[:300].decode(errors='replace')}")
    except urllib.error.URLError as err:
        raise SystemExit(f"Could not reach {url}: {err.reason}")
    return data["choices"][0]["message"]["content"] or ""


def load_endpoint(args):
    if args.from_handy:
        s = json.load(open(args.handy_settings, encoding="utf-8"))
        s = s.get("settings", s)
        pid = s["post_process_provider_id"]
        provider = next(p for p in s["post_process_providers"] if p["id"] == pid)
        return provider["base_url"], s["post_process_api_keys"].get(pid, ""), s["post_process_models"].get(pid, "")
    return (os.environ.get("BENCH_BASE_URL", ""), os.environ.get("BENCH_API_KEY", ""),
            os.environ.get("BENCH_MODEL", ""))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--prompts", default=str(HERE / "handy-prompts.json"))
    ap.add_argument("--cases", default=str(HERE / "bench_cases.json"))
    ap.add_argument("--prompt", help="only cases for this prompt id")
    ap.add_argument("--case", help="only this case id")
    ap.add_argument("--from-handy", action="store_true", help="read endpoint, model and key from Handy's settings")
    ap.add_argument("--handy-settings", default=DEFAULT_HANDY_SETTINGS)
    ap.add_argument("--dry", action="store_true", help="print assembled prompts, no network")
    ap.add_argument("--mock", action="store_true", help="use each case's 'mock' output instead of a model")
    ap.add_argument("-v", "--verbose", action="store_true", help="show model output for every case")
    args = ap.parse_args()

    prompts = {p["id"]: p for p in json.load(open(args.prompts, encoding="utf-8"))}
    cases = json.load(open(args.cases, encoding="utf-8"))
    cases = [c for c in cases if (not args.prompt or c["prompt"] == args.prompt) and (not args.case or c["id"] == args.case)]
    if args.mock:
        cases = [c for c in cases if "mock" in c]
    if not cases:
        raise SystemExit("No matching cases.")
    base_url = key = model = ""
    if not (args.dry or args.mock):
        base_url, key, model = load_endpoint(args)
        if not (base_url and key and model):
            raise SystemExit("Set BENCH_BASE_URL, BENCH_API_KEY and BENCH_MODEL, or use --from-handy.")
        print(f"Endpoint: {base_url}   model: {model}\n")

    passed = failed = 0
    for case in cases:
        prompt = prompts[case["prompt"]]
        content = assemble(prompt, case)
        leftovers = [v for v in VAR.findall(content)]
        if args.dry:
            print(f"=== {case['id']} ({case['prompt']}) -- {len(content)} chars, unexpanded vars: {leftovers or 'none'}\n{content}\n")
            continue
        raw = case["mock"] if args.mock else call_model(base_url, key, model, content)
        text, vocab = finish(raw, case)
        failures = run_checks(text, vocab, case.get("checks", []))
        status = "PASS" if not failures else "FAIL"
        passed, failed = passed + (not failures), failed + bool(failures)
        print(f"{status}  {case['id']:<26} [{case['prompt']}]  {case.get('note', '')}")
        for f in failures:
            print(f"        - failed {f}")
        if failures or args.verbose:
            print("        input : " + case["input"].replace("\n", " ")[:160])
            print("        output: " + text.replace("\n", "\\n")[:300] + (f"   vocab={vocab}" if vocab else ""))
    if not args.dry:
        print(f"\n{passed} passed, {failed} failed, {passed + failed} total")
        sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
