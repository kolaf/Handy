#!/usr/bin/env python3
"""Copy this fork's prompts (fork/prompts/handy-prompts.json) into a Handy settings file.

Close Handy first: it keeps settings in memory and would overwrite the change.
Prompts with the same id are replaced, new ones are appended, anything else you made yourself
is left alone. A timestamped backup is written next to the settings file.

  python3 install-prompts.py                  # the settings file, if there is exactly one; otherwise it lists them and stops
  python3 install-prompts.py PATH/settings_store.json      # a portable Handy: <folder>/Data/settings_store.json
  python3 install-prompts.py --dry-run

A portable Handy keeps its settings in Data/ next to handy.exe, not in %APPDATA%: say which file you mean when there are several.
It refuses to run while Handy is running (--force overrides).
"""
import argparse
import json
import os
import shutil
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent


def candidate_settings_files() -> list:
    """Every Handy settings file that can be found: the installed profile and portable folders (Data/ next to handy.exe)."""
    found = []
    if os.name == "nt":
        found.append(Path(os.environ.get("APPDATA", "")) / "com.pais.handy" / "settings_store.json")
        roots = [Path(f"{d}:/") for d in "CDEF"]
    else:
        mnt = Path("/mnt")
        found += sorted(mnt.glob("c/Users/*/AppData/Roaming/com.pais.handy/settings_store.json")) if mnt.is_dir() else []
        found.append(Path.home() / ".local/share/com.pais.handy/settings_store.json")
        roots = sorted(mnt.glob("[a-z]")) if mnt.is_dir() else []
    for root in roots:
        for pattern in ("*/Data/settings_store.json", "Data/settings_store.json", "Users/*/*/Data/settings_store.json"):
            try:
                found += sorted(root.glob(pattern))
            except OSError:
                pass
    unique = []
    for path in found:
        if path.is_file() and path not in unique:
            unique.append(path)
    return unique


def handy_is_running() -> bool:
    """Is a handy process running? (From WSL the Windows process list is used.)"""
    import subprocess
    commands = []
    if os.name == "nt" or Path("/mnt/c/Windows/System32/tasklist.exe").exists():
        exe = "tasklist" if os.name == "nt" else "/mnt/c/Windows/System32/tasklist.exe"
        commands.append([exe, "/FI", "IMAGENAME eq handy.exe", "/NH"])
    commands.append(["pgrep", "-x", "handy"])
    for command in commands:
        try:
            out = subprocess.run(command, capture_output=True, text=True, timeout=10).stdout.lower()
        except (OSError, subprocess.SubprocessError):
            continue
        if "handy.exe" in out or (command[0] == "pgrep" and out.strip()):
            return True
    return False


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("settings", nargs="?", type=Path, default=None)
    ap.add_argument("--prompts", type=Path, default=HERE.parent / "prompts" / "handy-prompts.json")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--force", action="store_true", help="also when Handy seems to be running")
    args = ap.parse_args()

    if args.settings is None:
        candidates = candidate_settings_files()
        if len(candidates) != 1:
            print("Which settings file? " + ("None was found." if not candidates else "Several were found:"))
            for c in candidates:
                print(f"  {c}")
            print("Run again with the path of the one that your Handy uses (a portable Handy: its Data/settings_store.json).")
            sys.exit(2)
        args.settings = candidates[0]
    if not args.dry_run and not args.force and handy_is_running():
        print("Handy is running: it keeps its settings in memory and would overwrite the change. Close it first (or use --force).")
        sys.exit(3)

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
