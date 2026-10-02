# Talon <-> Handy

Untested starting point (written without access to a Talon install).

Install: copy `handy.py` and `handy.talon` into your Talon user folder
(`~/.talon/user/` on Linux/macOS, `%APPDATA%\talon\user\` on Windows), e.g. in a `handy/` subfolder.

On Windows set the executable path, e.g. in a `.talon` settings block:
`user.handy_path = "C:/Users/<you>/AppData/Local/Handy/handy.exe"`.

Requires a Handy build with `--swap-language`, `--next-prompt`, `--set-language` and `--set-prompt`
(branch `dev/hotkeys-build`).

Extra voice commands (need the newest dev build)
- `handy reply`: select a message you want to answer, say it, then dictate your reply. Talon copies the
  selection to the clipboard, selects Handy's `reply` prompt (which reads `${clipboard}`) and starts
  dictation. Press the dictation key again to finish.
- `handy rerun`: re-process your last dictation with the next prompt. Select the text you pasted earlier
  first if you want it replaced.

Design notes
- `F13` (or whichever key you choose) is owned by Talon. It runs `handy --toggle-post-process` and
  mutes Talon's speech recognition while Handy records, so your dictated sentences are not also
  interpreted as Talon commands. Press it again to stop and wake Talon. If Talon stays muted after a
  failed dictation, use your normal Talon wake command.
- `handy auto styles on` switches Handy's prompt when the focused app changes (see `APP_PROMPTS`).
