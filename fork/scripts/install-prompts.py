#!/usr/bin/env python3
"""Copy this fork's prompts (fork/prompts/handy-prompts.json) into a Handy settings file.

Close Handy first: it keeps settings in memory and would overwrite the change.
Prompts with the same id are replaced, new ones are appended, anything else you made yourself
is left alone. A timestamped backup is written next to the settings file.

  python3 install-prompts.py                  # default settings path for this platform
  python3 install-prompts.py PATH/settings_store.json
  python3 install-prompts.py --dry-run
"""
import argparse
import json
import os
import shutil
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent


def default_settings_path() -> Path:
    if os.name == "nt":
        return Path(os.environ.get("APPDATA", "")) / "com.pais.handy" / "settings_store.json"
    wsl = sorted(Path("/mnt/c/Users").glob("*/AppData/Roaming/com.pais.handy/settings_store.json")) \
        if Path("/mnt/c/Users").is_dir() else []
    return wsl[0] if wsl else Path.home() / ".local/share/com.pais.handy/settings_store.json"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("settings", nargs="?", type=Path, default=default_settings_path())
    ap.add_argument("--prompts", type=Path, default=HERE.parent / "prompts" / "handy-prompts.json")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    raw = args.settings.read_text(encoding="utf-8")
    doc = json.loads(raw)
    settings = doc.get("settings", doc)
    wanted = {p["id"]: p for p in json.loads(args.prompts.read_text(encoding="utf-8"))}
    existing = settings["post_process_prompts"]
    replaced = [p["id"] for p in existing if p["id"] in wanted]
    merged = [wanted.pop(p["id"]) if p["id"] in wanted else p for p in existing]
    added = list(wanted)
    merged += wanted.values()
    settings["post_process_prompts"] = merged
    print(f"settings: {args.settings}\n  replaced: {replaced}\n  added:    {added}\n  kept:     "
          f"{[p['id'] for p in merged if p['id'] not in replaced and p['id'] not in added]}")
    print(f"  selected prompt stays: {settings.get('post_process_selected_prompt_id')}")
    if args.dry_run:
        print("(dry run, nothing written)")
        return
    backup = args.settings.with_name(args.settings.name + time.strftime(".pre-prompts-%Y%m%d-%H%M%S-backup"))
    shutil.copy2(args.settings, backup)
    indent = 2 if raw.startswith("{\n  ") else None
    args.settings.write_text(json.dumps(doc, ensure_ascii=False, indent=indent), encoding="utf-8")
    json.loads(args.settings.read_text(encoding="utf-8"))  # must still parse
    print(f"written; backup: {backup.name}")


if __name__ == "__main__":
    sys.exit(main())
