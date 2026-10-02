"""Talon side of the voice shell (see ../README.md). Types `hv ...` commands into the focused terminal and
remembers which files are selected in a file manager.

Written from memory of Talon's API and NOT run against a real Talon install: check the action names
(actions.insert, actions.edit.copy, actions.sleep, app.notify) against your Talon version.
"""
import os
import platform
import shlex
import subprocess
import urllib.parse
from pathlib import Path

from talon import Module, actions, app

mod = Module()


def _selection_file() -> Path:
    """Where the selected paths are remembered. The hv script finds the same file (also from WSL)."""
    return Path.home() / ".cache" / "hv" / "selection.txt"


def _clipboard_paths() -> list[str]:
    """Paths of the files currently on the clipboard (what Ctrl+C in a file manager puts there)."""
    if platform.system() == "Windows":
        # Explorer puts real file paths on the clipboard as a file-drop list.
        cmd = ["powershell", "-NoProfile", "-Command",
               "Get-Clipboard -Format FileDropList | ForEach-Object { $_.FullName }"]
        out = subprocess.run(cmd, capture_output=True, text=True, creationflags=0x08000000).stdout
        return [line.strip() for line in out.splitlines() if line.strip()]
    for target in ("text/uri-list", "x-special/gnome-copied-files"):
        try:
            out = subprocess.run(["xclip", "-selection", "clipboard", "-t", target, "-o"],
                                 capture_output=True, text=True, timeout=3).stdout
        except (OSError, subprocess.TimeoutExpired):
            continue
        paths = [urllib.parse.unquote(line[7:]) for line in out.splitlines() if line.startswith("file://")]
        if paths:
            return paths
    return []


@mod.action_class
class Actions:
    def hv_grab_selection():
        """Remember the files selected in the file manager so the next hv request can say 'this'."""
        actions.edit.copy()
        actions.sleep("250ms")
        paths = _clipboard_paths()
        target = _selection_file()
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("\n".join(paths) + ("\n" if paths else ""), encoding="utf-8")
        app.notify(f"hv: {len(paths)} path(s) remembered" if paths else "hv: nothing selected")

    def hv_request(text: str):
        """Send a spoken request to the voice shell (plan first)."""
        actions.insert(f"hv {shlex.quote(text)}\n")

    def hv_go():
        """Approve the plan the voice shell just showed."""
        actions.insert("hv go\n")

    def hv_again(text: str):
        """Correct or answer the voice shell (plans again)."""
        actions.insert(f"hv again {shlex.quote(text)}\n")

    def hv_ask(text: str):
        """Ask the voice shell a read-only question."""
        actions.insert(f"hv ask {shlex.quote(text)}\n")

    def hv_cancel():
        """Interrupt the voice shell."""
        actions.key("ctrl-c")
