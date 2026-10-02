"""Talon <-> Handy bridge.

Drives a running Handy through its command-line flags. Written from memory of Talon's API
and NOT run against a real Talon install -- check action names against your version.
"""
import subprocess

from talon import Context, Module, actions, app, settings, ui

mod = Module()
ctx = Context()

mod.setting(
    "handy_path",
    type=str,
    default="handy",
    desc="Handy executable. On Windows use the full path to handy.exe.",
)
mod.list("handy_style", desc="Spoken names for Handy post-processing prompts")

# Spoken name -> prompt id (ids are the ones in handy-prompts.json).
ctx.lists["user.handy_style"] = {
    "simple": "simple",
    "message": "informal_message",
    "email": "email",
    "note": "note",
    "meeting": "meeting",
    "super": "super",
    "reply": "reply",
    "informal": "informal_text",
    "formal": "formal_text",
}

# Focused app (lower-case substring of the app name) -> prompt id. Edit to taste.
APP_PROMPTS = {
    "outlook": "email",
    "thunderbird": "email",
    "teams": "informal_message",
    "slack": "informal_message",
    "obsidian": "note",
    "code": "simple",
    "terminal": "simple",
}

_state = {"handy_dictating": False, "last_prompt": None, "auto_prompt": False}


def _handy(*args: str) -> None:
    """Fire and forget: the call forwards the request to the running Handy and exits."""
    exe = settings.get("user.handy_path")
    kwargs = {"stdout": subprocess.DEVNULL, "stderr": subprocess.DEVNULL}
    if app.platform == "windows":
        kwargs["creationflags"] = 0x08000000  # CREATE_NO_WINDOW
    subprocess.Popen([exe, *args], **kwargs)


@mod.action_class
class Actions:
    def handy_toggle():
        """Start or stop a Handy dictation (with post-processing), muting Talon meanwhile."""
        if not _state["handy_dictating"]:
            # Mute Talon first so it does not act on the sentences meant for Handy.
            actions.speech.disable()
            _state["handy_dictating"] = True
            _handy("--toggle-post-process")
        else:
            _state["handy_dictating"] = False
            _handy("--toggle-post-process")
            actions.speech.enable()

    def handy_reply():
        """Copy the selected message and start a dictated reply to it (uses the 'reply' prompt)."""
        if _state["handy_dictating"]:
            return
        # The 'reply' prompt reads the clipboard, so put the selected message there first.
        actions.edit.copy()
        actions.sleep("150ms")
        actions.speech.disable()
        _state["handy_dictating"] = True
        # Several settings in one call. The prompt stays on 'reply' afterwards; switch back with
        # 'handy style <name>'. Do not reset it while Handy is still processing: the prompt is
        # read after you stop speaking.
        _handy("--set-prompt", "reply", "--toggle-post-process")

    def handy_rerun():
        """Re-run the last dictation with the next prompt (select the old text first to replace it)."""
        _handy("--rerun")

    def handy_cancel():
        """Cancel the current Handy dictation and wake Talon."""
        _handy("--cancel")
        if _state["handy_dictating"]:
            _state["handy_dictating"] = False
            actions.speech.enable()

    def handy_language(code: str):
        """Set Handy's language ("no", "en", "auto")."""
        _handy("--set-language", code)

    def handy_swap_language():
        """Swap Handy's language with its alternate language."""
        _handy("--swap-language")

    def handy_next_prompt():
        """Switch to Handy's next post-processing prompt."""
        _handy("--next-prompt")

    def handy_prompt(prompt_id: str):
        """Set Handy's post-processing prompt by id."""
        _handy("--set-prompt", prompt_id)
        _state["last_prompt"] = prompt_id

    def handy_auto_prompt_set(enabled: bool):
        """Turn per-app prompt switching on or off."""
        _state["auto_prompt"] = enabled
        app.notify(f"Handy auto styles: {'on' if enabled else 'off'}")


def _on_app_activate(active_app) -> None:
    if not _state["auto_prompt"]:
        return
    name = (active_app.name or "").lower()
    for needle, prompt_id in APP_PROMPTS.items():
        if needle in name:
            if prompt_id != _state["last_prompt"]:
                actions.user.handy_prompt(prompt_id)
            return


ui.register("app_activate", _on_app_activate)
