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

## Personal overrides (`personal/`)

Installed in the Talon user folder as `user\personal\`, outside the community folder so community can be updated
without conflicts (community was reset to upstream on 2 October 2026; the old local edits are in the backup branch
`backup/pre-upstream-sync-20261002` of `kolaf/community`).

- `wake_key_and_tag.talon`: `Ctrl+PageUp` toggles speech (the way to wake Talon now), and switches on the tag below.
  If voice wake ever needs to come back, delete this file's `tag()` line or the `disable_wake_*.talon` files.
- `disable_wake_*.talon` + `personal.py`: override community's spoken wake commands ("wake up", "talon wake",
  "welcome back") with `skip()`. Each has community's context plus the tag `user.disable_voice_wake`, so it is more
  specific and wins. "wake up and listen" (leaving deep sleep) is left alone on purpose.
- `special_key_extra.talon-list`: adds the spoken key name `shock` (presses Enter) to community's `special_key` list.
  Lists with the same name from matching contexts merge, so community's own entries stay. A `.talon-list` file must
  start with its `list:` header on line 1; comments go below the `-` line.
- `drowse.talon`: `drowse` puts Talon to sleep, like community's "go to sleep" (wake with `Ctrl+PageUp`).
- Not restored from the old setup: `junk` (Delete), `mixed mode`, and the Python word-case tweaks.

- Verified: Talon loaded all files without errors and its registry lists both the community command and the `skip()`
  override for each phrase. Not verified: that the override wins in practice; test it by putting Talon to sleep
  with `Ctrl+PageUp`, saying "wake up" (nothing should happen), then `Ctrl+PageUp` again (Talon wakes).
- If locked out: the Talon tray icon menu can re-enable speech.
