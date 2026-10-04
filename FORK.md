# This fork

This is a fork of [cjpais/Handy](https://github.com/cjpais/Handy) that has deliberately diverged. It keeps the MIT license
(see `LICENSE`). Upstream changes are merged in by hand; patches may go back upstream later if they open up.

- Product branch: `dev/hotkeys-build` (the older `feature/*` branches are stale; ignore or delete them).
- Version label: `0.9.7-hotkeys.1` (set in `package.json`, `src-tauri/Cargo.toml`/`Cargo.lock`, `src-tauri/tauri.conf.json`).
  It sorts below the stock `0.9.7`, so **turn off "Update checks" in the app**, or it will offer to replace this build.
- Everything below is **unverified against a real language model unless it says otherwise**; run the prompt bench (see
  [Prompts](#prompts-and-the-test-bench)).

## What was added

| Feature | How to use it | Where to configure |
|---|---|---|
| **Swap language** | `ctrl+alt+l` or `handy --swap-language` swaps the language with the *alternate language* | General: Language and Alternate Language pickers |
| **Set exact values** | `handy --set-language no`, `handy --set-prompt email`; combinable with each other and a toggle | n/a (flags only) |
| **Overlay feedback** | A caption under the controls shows the language (and the prompt, if post-processing runs); a short notice appears after a switch; the activity bars scale to your recent volume | Settings: overlay style |
| **Vocabulary by dictation** | Spell a word after saying it, or say "add to vocabulary X" / "legg til i ordlisten X" | Custom Words (Advanced) |
| **Snippets** | Say "insert my signature" and the stored text appears | Snippets (Advanced) |
| **Prompt variables** | `${vocabulary}` `${snippets}` `${clipboard}` `${examples}` in any prompt | Prompt editor |
| **Prompt examples** | Optional examples box per prompt | Prompt editor |
| **Re-run with next prompt** | `ctrl+alt+r` or `handy --rerun` | Post-processing page |
| **Prompt picker** (replaces cycling) | `ctrl+alt+p` or `handy --prompt-picker` | Post-processing page |
| **Reformat selection** | `ctrl+alt+f` or `handy --reformat` | Post-processing page |
| **Prompt per app** | Post-processing page | rules "program (+ title) → prompt", Windows and Linux/X11 |
| **Transform** | `handy --transform ID` (Talon: "make that formal" ...) | selection or last dictation → prompt `t_*` → replaces it |
| **Reply with context** | `handy --use-prompt-once reply --toggle-post-process` (Talon: "reply to this") | |
| **Learn a repo** | `handy --learn-repo FOLDER` (Talon: "learn this repo"), `handy --import-words FILE` | adds project words |
| **Sync lists** | `handy --sync-lists FILE` | merges word list, snippets and learned corrections with a JSON file kept in git |
| **Learn from correction** | `ctrl+alt+k` or `handy --learn` | Post-processing page; list under Advanced → Learned Corrections |
| **Paste last dictation** | `ctrl+alt+v` or `handy --paste-last` | General page |
| **Offline fallback** | Automatic when the language model call fails | n/a |
| **Skip the model for short dictations** | Number of words below which the model is skipped | Post-processing page (0 = off) |
| **Edit by voice** | Copy text, press the post-processing key, say the change ("make it shorter", "translate to English"); the `edit` prompt pastes the result | `edit` prompt |
| **Eleven default prompts** | Baked in for fresh and portable installs | `fork/prompts/` |
| **Guide page** | Sidebar > Guide: what is new in this build and a quick reference | `src/content/fork-guide.md` (keep it in step with this file) |

Default shortcuts are `ctrl+alt+l`, `ctrl+alt+p` (prompt picker), `ctrl+alt+r`, `ctrl+alt+v`; rebind them in Settings. On Wayland, desktops
own global shortcuts, so bind the command-line flags instead (see below).

### Details

**Vocabulary by dictation.** The post-processing prompt returns `[[vocab: WORD]]` on its own line when you spell a word or
ask to add one. Handy removes the tag from the pasted text and appends the word to Custom Words (skipping duplicates,
ignoring case), then shows "Added to vocabulary". Words are validated because they come from model output: one line,
at most 64 characters, no brackets or control characters, at most 5 per dictation. Works only with the post-processing
hotkey and a prompt that contains the vocabulary step (all of ours do).

**Snippets.** Settings > Advanced > Snippets: a name and a block of text (names up to 50 characters, text up to 5000,
at most 100, unique ignoring case). The model only sees the *names* (via `${snippets}`) and writes `[[snippet: NAME]]`
where you asked for one; Handy replaces the tag with your text verbatim and locally, so signatures, addresses and URLs are
never sent to the model and are never altered. Unknown names are dropped. A prompt must contain `${snippets}` for the
model to know the names.

**Prompt variables.** `${output}` is the transcript (as upstream). `${vocabulary}` is your Custom Words, `${snippets}` the
snippet names. `${clipboard}` is the clipboard text (read **only** when the prompt contains it; capped at 6000
characters; never stored in history). `${examples}` is the prompt's examples. If a prompt has examples but does not place
`${examples}`, they are appended after the instructions. Substituted text is never scanned for variables again.
There is no native `${selection}`: copy the text first (a Talon command can do this: the `hermes`/`grab files` commands in the community fork, `kolaf/`).

**Examples field.** For fixed structures: put the structure with `[placeholders]` in the prompt, then give one or two
`Dictation: ... / Result: ...` pairs separated by `---`. The *Document template* prompt is a working sample.

**Learn from correction.** After fixing a dictation by hand, select the corrected passage and press the shortcut. Handy copies
the selection (your clipboard is restored), finds the matching dictation in the history, and asks the post-processing model what
was a recognition mistake (`fork/prompts/learn_prompt.md`). Names and terms go into the custom words; recurring mishearings become
rules applied to future transcripts. Proposals are validated locally (the word must really appear in your text, ordinary-word
swaps and rewordings are refused). Benchmark: `python3 fork/prompts/learn_bench.py --from-handy`.

How the selection is copied (learn, reformat, transform, "edit this"/"reply to this"): Handy first releases any Alt, Shift or
Meta key that your hotkey may still be holding (otherwise the app would receive Ctrl+Alt+C), then sends Ctrl+C, or Ctrl+Shift+C
when the active window is a terminal (a plain Ctrl+C there would interrupt the running program). If the clipboard does not change
it tries once more. The Activity page entry says what was sent to which window and whether it worked. If nothing could be copied
and the clipboard text does not look like your last dictation (less than 40 % shared words), learn stops instead of comparing
unrelated texts.

**Prompt picker.** `Ctrl+Alt+P` opens a small floating window listing the first nine prompts with a number each (the
current one is highlighted). Press a number key or click a row to switch to that prompt; Escape, the shortcut again, or 12
seconds of nothing closes it. The window never takes focus, so the app you are dictating into keeps its selection. While it is
open the digits 1-9 and Escape are temporary global shortcuts, swallowed so no digit is typed into the document (if the
keyboard hook cannot block keys on your system, the digit would also reach the document). More than nine prompts: only the
first nine are listed; use `--set-prompt ID` for the rest. The old cycle-prompt shortcut is gone (its stored binding is
removed on first start and `Ctrl+Alt+P` now opens the picker); "re-run with next prompt" still steps through the list.

**Reformat selection.** `Ctrl+Alt+F` (or `handy --reformat`) copies the selected text (your clipboard is restored), runs it
through the currently selected prompt exactly like a dictation would be, and pastes the result over the selection. Needs real
selected text and a working post-processing setup; the `edit` prompt is refused because it expects a spoken instruction.
Spoken-punctuation and filler rules of the prompt apply to the selected text too, so choose a prompt that fits (formal,
informal, email, ...). Undo in the target app (Ctrl+Z) reverts it.

**Corrections: always or hint.** A learned correction is either *always* (replaced literally in every transcript, before
the formatter sees it) or a *hint* (only listed to the formatter, which applies it when it fits the sentence). Learn stores a rule as *always*
only when the model vouches that the heard text is not a real word ("Superwisper", "Hermia"); everything else ("fart" for
"prompt", "carry" for "Kari") is a hint, because the speaker may really mean that word. Switch any rule with the button
in Advanced → Learned Corrections. Prompts
get the rules through the `${corrections}` variable (the built-in prompts already have it: "X is Y" for always rules, "X may be Y
(only if it fits the sentence)" for hints). Hints need post-processing to be on and reachable; without it nothing happens.

**Activity log.** Every on-screen notice except language and prompt switches ("Learned: ...", "Nothing new to learn", "Lists
synced", reformat and transform problems, "Offline: basic cleanup only") is also written to `activity.jsonl` in Handy's data
folder (the last 500 entries) and shown on the **Activity** page next to History. An entry has the full notice text and, for
learning, the details: which dictation was compared (heard / pasted / corrected), what the model said about it, what it
proposed, what was added and every item that was left out with the reason ("does not occur in the corrected text", "already in
your custom words", "a common word" ...). For `--learn-repo` it lists the folder, how many files and terms were read, the
terms the model chose and which of them were new.

**Where you started.** When recording starts Handy remembers the foreground program and window (`context.rs`,
`begin_recording_context`). That is what the prompt rules and the `${app}` / `${title}` variables use, and what the paste guard
compares with: if the program or window is a different one when the text is ready (Windows; the title is ignored because it
changes as you type), the text is put on the clipboard instead of pasted, with a notice and an Activity entry ("Not pasted:
window changed"). Setting: Advanced → "Keep dictation out of the wrong window" (on by default). Actions that do not record
(re-run, redo) use the window that has focus when they run.

**Linux (X11).** The active window is read through the standard window-manager property `_NET_ACTIVE_WINDOW` (title from
`_NET_WM_NAME`, program from the window's process `/proc/<pid>/exe`, falling back to the window class for sandboxed apps), using
the `x11rb` crate (no extra system libraries). Everything that depends on the active window works the same: prompt rules (rule
program names are the process names, e.g. `slack`, `firefox`, `gnome-terminal-server`), `${app}` / `${title}`, the paste
guard, and terminal-aware copying (Ctrl+Shift+C in terminals). Under Wayland applications cannot ask which window is active, so
there the app is unknown: rules do not match, and the paste guard and the terminal check do nothing. The Talon files in
`kolaf/` are written for Windows Terminal; Linux Talon needs the terminal contexts adjusted.

**More prompt variables.** `${app}`, `${title}` (program and window title where you started speaking; the title is bounded and
defused because it is untrusted text), `${language}` (the dictation language setting), `${date}` (2026-10-03), `${time}`
(21:48) and `${weekday}`. Unknown values read "(unknown)". `${selection}` was left out on purpose: capturing a selection at
the start of every dictation would send Ctrl+C while you hold your dictation hotkey and delay the recording; "reply to this" and
"edit this" copy the selection explicitly instead.

**Scratch and redo.** `handy --scratch-last` deletes the last dictation and `handy --redo-with ID` replaces it by the same
recording (the raw transcript from the history) processed with prompt `ID`. Both work like Talon's "scratch that": Handy
remembers what it last pasted as a dictation (the text, the window and the time) and takes it back by pressing Backspace once
per character (plus one for the trailing space). Nothing is selected or copied, so it works in terminals and in VS Code's
terminal panel without risking a Ctrl+C. It only runs if the same window still has focus (the program and the window; the title is
ignored), the paste is at most 5 minutes old and at most 1500 characters long; otherwise it does nothing and the Activity page says
why. It cannot see whether you moved the cursor inside that window, so use it right after dictating. After a scratch the memory
is empty, so a second "scratch" does not eat more text. Paste-last and redo count as dictations; reformat results do not.

Handy writes the time of its last undoable paste to `%USERPROFILE%\\.cache\\hv\\handy-paste.txt` (`none` once taken back). Talon's
"scratch that" / "nope that" is overridden (`kolaf/handy/scratch_that.py`): Talon stamps each phrase it types, and "scratch that"
asks Handy to scratch when Handy's paste is newer than Talon's last phrase and at most 5 minutes old; otherwise the normal
community behaviour runs. It does not matter how a dictation was started (hotkey, tray, Talon command). Text typed by hand is
known to neither side. Talon: "scratch dictation", "redo as email|message|
note|meeting|document|formal|informal|simple".

**Model per language (optional).** Settings → General → "Model per language": turn it on and add rules "language code →
downloaded model" (for example `en` → Parakeet, `no` → NB-Whisper). Whenever the dictation language changes through the swap
shortcut, `--set-language`, Talon, or the language setting, Handy also switches to that language's model (toast and Activity entry;
a rule for a model that is not downloaded says so and changes nothing). Languages without a rule leave the model alone, and with
the switch off nothing changes. Combined with `--swap-language` this makes "English ↔ Norwegian" a single action.

**Model switching.** `handy --set-model NAME` switches the speech-to-text model by (part of) its id or name, ignoring case:
an exact id wins, otherwise the words must occur in exactly one downloaded model (ambiguous or unknown names say so and list the
downloaded ones). It uses the same switch as the settings page and tray menu (the model is loaded right away unless unloading is
set to "Immediately"), shows a toast, and writes the result to the Activity page, with a note if the model does not list the
current language. `handy --model-picker` (shortcut `ctrl+alt+m`, setting on the General page) opens the same numbered window as
the prompt picker, listing the downloaded models. Only downloaded models can be chosen; download more on the Models page. Talon:
"model parakeet", "model norwegian", "model whisper small", "model picker" (names in `kolaf/handy/models.talon-list`).

**Prompt per app.** Post-processing page → "Prompt per app": turn it on and add rules (program name such as `slack.exe`, an
optional window-title part, and a prompt). When you dictate with post-processing, the first matching rule decides the prompt;
a rule with a title part wins over a rule for the whole app; no match means the selected prompt. Windows and Linux with X11 (it reads the
foreground window). The log shows the decision ("Prompt for this dictation: ... (app rule for slack.exe ...)"), which is also
the easiest way to find a program's name. A prompt set with `--use-prompt-once` (below) wins over a rule.

**Edit by instruction.** Talon's "edit this" copies the selected text and runs `handy --use-prompt-once edit --toggle-post-process`: you then speak the change you want, stop with your Handy key, and the `edit` prompt (selected text from the clipboard, your words as the instruction) pastes the result over the still-selected text.

**One-shot prompt.** `handy --use-prompt-once ID` makes the next dictation that uses post-processing use prompt `ID` and then
forgets it (it expires after 3 minutes). Talon's "reply to this" copies the selected message, sets `reply` this way and starts
a dictation, so the reply is written with the message as context.

**Transform.** `handy --transform ID` runs the selected text through prompt `ID` (without changing the selected prompt) and
replaces it. With nothing selected it takes the last dictation Handy pasted (see "Scratch and redo": same
window, at most 5 minutes old): the model works on that text, then the old text is removed with Backspace presses and the result
is pasted. A selection is detected by copying it (Ctrl+C, or Ctrl+Shift+C in a recognised terminal). The built-in transform prompts have ids starting with `t_` (formal, informal, shorter, longer, clearer,
fix, to_no, to_en, bullets, summary) and are left out of the prompt picker and "re-run with next prompt". Talon: "make that
formal", "make that shorter", "fix that up", "translate that to norwegian", "bullet that", "summarize that" ...

**Recording state.** While running, Handy keeps `%USERPROFILE%\.cache\hv\handy-state.txt` ("recording" or "idle", then a Unix
time; refreshed every 3 s while recording). Talon uses it to switch its speech off while Handy records and on afterwards, so a
dictation is not taken for voice commands (Talon setting `user.kolaf_mute_during_handy`).

**Learn a repo.** `handy --learn-repo FOLDER` scans the folder (skipping `.git`, `node_modules`, `target` ... and capped at 4000
files / 4 MB), takes the 400 best-scoring terms from file names and sources, asks the post-processing model to choose (a) names a speech
recognizer would likely get wrong (people, places, airport and product names, codes, tool names with unusual spelling) and (b) the
project's own domain vocabulary, including everyday words that are central to it (an air sports app: scorecard, contestant,
waypoint, gate), but not generic programming or interface words; keeps only answers that
were really candidates, and adds the new ones to the custom words. `handy --import-words FILE` adds the words of a text file
(one per line) with no model involved.

**Sync lists.** `handy --sync-lists FILE` (Handy must be running) merges your custom words, snippets and learned corrections
with a JSON file, both ways, and rewrites the file in a stable sorted order so it diffs well in git. Keep the file in a
private repo (it holds your snippet texts) and run the command on each machine after pulling, then commit the result. It only
adds: deleting an entry on one machine does not delete it elsewhere (it would come back), so remove it everywhere or edit the
file. If both sides have a snippet or correction with the same key but different content, this machine's version wins and the
conflict is logged. Invalid entries in the file are skipped. Prompts are not in the file (they come from the repo:
`fork/scripts/install-prompts.py`), and neither are keys or other settings.

**Re-run with next prompt.** Takes the raw transcript of your most recent dictation from the history, advances to the next
prompt, processes it and pastes the result. Select the earlier pasted text first to replace it; otherwise the result is
inserted as a second copy.

**Offline fallback and short dictations.** When post-processing is fully configured (provider, model, prompt) but the request
fails, or when a dictation has fewer words than the minimum-words setting, a *local cleanup* runs instead of the model:
spoken punctuation (English and Norwegian: question mark/spørsmålstegn, exclamation mark/utropstegn, comma/komma,
period/full stop/punktum, colon/kolon, semicolon/semikolon, new line/ny linje, new paragraph/nytt avsnitt) becomes the
symbol, and sentences are capitalized. It is deliberately simple: a literal "the period of time" is converted too. The
overlay says "Offline: basic cleanup only" after a failure.

## Command-line flags

Sent to the running instance (a second `handy` process forwards them and exits). If Handy is not running they just start it.

```
handy --toggle-transcription | --toggle-post-process | --cancel      (upstream)
handy --swap-language          handy --prompt-picker
handy --rerun                  handy --paste-last
handy --learn                  handy --sync-lists FILE
handy --set-language CODE      handy --set-prompt ID                  (combinable, also with a toggle)
handy --set-language no --set-prompt email --toggle-post-process
```

Prompt ids: `simple`, `informal_message`, `email`, `note`, `meeting`, `super`, `reply`, `document`, `informal_text`,
`formal_text`, `edit`, plus any you create.

## Prompts and the test bench

All in `fork/prompts/`.

- `build_prompts.py` is the **source of truth**. It builds each prompt from shared steps: (1) spoken punctuation,
  (2) spelled-out letters including the NATO alphabet, (3) cleanup, self-corrections and repair of misheard words (unclear
  words are marked `[word?]`), (4) vocabulary and snippet tags, (5) the style. `python3 build_prompts.py` writes
  `handy-prompts.json` and `src-tauri/src/dev_prompts.json`.
- `dev_prompts.json` is compiled into the app: **fresh settings and portable installs start with these prompts** and `super`
  selected. Existing installs keep what they have stored; update them with
  `python3 fork/scripts/install-prompts.py` (close Handy first; it makes a backup).
- `bench.py` runs `bench_cases.json` (24 cases) the way Handy sends a request and checks the result:
  `python3 bench.py --from-handy` (endpoint, model and key from your Handy settings), `--prompt email`,
  `--case spell_exe -v`, `--dry` (print assembled prompts), `--mock` (check the checker without a model).
  On 2 October 2026 all 29 cases passed against the real model (GPT 5.4 via the hosted gateway; one run, simple
  checks, so treat it as strong evidence and not a guarantee). Rerun it after changing a prompt or the model.

## Talon

The Talon files live in the community fork `kolaf/community`, folder `kolaf/` (not in this repo), so a clone of the fork
carries them: a Talon bridge for Handy (disabled until you enable it), personal overrides (wake key `Ctrl+PageUp`,
spoken wake commands disabled, `drowse`, `shock`), and the voice shell commands. Its `kolaf/README.md` has the setup for a
new machine and how to merge upstream. **The spoken behaviour has not been tried; Talon loads the files without errors.**

## Voice shell

`fork/voice-shell/` is separate from Handy: `hv`, a wrapper that turns a spoken request into one short Hermes Agent
session (plan first, then `hv go`), with Talon commands (`hermes <request>`, `grab files`) that live in the community fork. Safety rules, measured
behaviour, what is unverified and how to install: `fork/voice-shell/README.md`.

## Building and installing

Windows prerequisites: Visual Studio 2026 with the *Desktop development with C++* workload (MSVC x64/x86 build tools),
Windows SDK 10.0.26100, Rust (MSVC), Bun, CMake, Vulkan SDK. Run builds from a Visual Studio developer prompt.
**Close Handy first** if you run it from `target\release`: an open `handy.exe` cannot be replaced and the build fails. If you run a
*deployed copy in its own folder* (see below), builds never collide with it.

```
bun install
bun run tauri build --no-bundle                                  # target/release/handy.exe
bun run tauri build --bundles nsis --config fork/scripts/dev-bundle.json   # unsigned installer
```

`dev-bundle.json` turns off update-artifact signing and upstream's code-signing command (both need upstream's private
keys). `fork/scripts/build-portable.ps1` is the script used for portable builds (full test run, build, zip; stops on any failure);
its paths assume `C:\dev\Handy`. **Portable layout**: `handy.exe`, the DLLs from `target/release`, the `resources` folder, the
VC runtime DLLs, and a file named `portable` containing `Handy Portable Mode`. Data then lives in `Data\` next to the exe
(custom Whisper models: `Data\models`).

### Day-to-day local use (no installer)

Handy allows **one running instance per app identity**: starting a second copy (installed, portable, or `target\release\handy.exe`)
while another is open just hands over to the first and exits. So pick one copy to run at a time.

- **Simplest, nothing to copy:** run `src-tauri\target\release\handy.exe` after each build. With no `portable` file next to it, it
  uses the normal profile in `%APPDATA%\com.pais.handy` (the same settings and models as an installed Handy).
- **A stable portable folder you refresh after each build:** `build-portable.ps1` stages the build in `C:\dev\Handy-dev-portable`;
  then `fork\scripts\deploy-portable.ps1 -Target D:\Handy` copies it over the folder and **never touches `Target\Data`**
  (settings, models, history). The first time, add `-SeedFromInstalled` to copy your existing settings (endpoint, key, prompts) and
  models from the normal profile into `Data`; it refuses to seed if `Data` already exists. Close the portable copy first (the script
  checks). Do not delete and recreate the folder, or `Data` goes with it, and unzipping a release zip *over* the folder is safe.
  Add `-Restart` to stop a running copy, deploy, and start it again hidden in the tray (a dictation in progress is lost).
  The intended loop: keep running `D:\Handy\handy.exe`, build as often as you like (Handy can stay open), then
  `deploy-portable.ps1 -Target D:\Handy -Restart`. Run from a folder that is not the staging folder
  (`C:\dev\Handy-dev-portable`), which the build script recreates each time.
- A portable copy's autostart entry lives in the registry and points at the exe path; see the notes in the project history.
  Autostart entries use the same name for every copy of Handy, so enable it in only one.

Linux (Ubuntu 24.04): `sudo apt install build-essential clang libclang-dev libevdev-dev libasound2-dev pkg-config libssl-dev
libvulkan-dev vulkan-tools glslc spirv-headers glslang-tools libgtk-3-dev libwebkit2gtk-4.1-dev
libayatana-appindicator3-dev librsvg2-dev libgtk-layer-shell0 libgtk-layer-shell-dev patchelf cmake file xdg-utils rpm`, then
`bun run tauri build --bundles deb --config <file with {"bundle":{"createUpdaterArtifacts":false}}>`. Install with
`sudo apt install ./Handy_*_amd64.deb` and `xdotool` (X11). Unverified on a real desktop and on Ubuntu 26.04.

**Unsigned builds are blocked** by Defender SmartScreen on machines whose policy forbids bypassing it, and signing would not
help quickly (reputation builds with usage, per file). See the discussion in the project history; options are an IT-approved
folder, the official signed Handy with our prompts and `fork/scripts/handy-profile.ps1`, or Linux.

## Syncing between machines

| What | Lives in | How it gets to a new machine |
|---|---|---|
| Handy code, prompts, `hv`, docs, build scripts | `kolaf/Handy`, branch `dev/hotkeys-build` (public) | `git clone`; deploy a build with `fork/scripts/deploy-portable.ps1`, or install the `.deb` |
| Talon user files: wake key, `shock`/`drowse`, `sleep.py`, voice shell commands, disabled Handy bridge | `kolaf/community` (public), folder `kolaf/` | clone the fork into Talon's `user/` folder; see `kolaf/README.md` there |
| Other Talon packages (Cursorless, Rango) | their own upstream repos (old checkouts) | listed in `kolaf/README.md`; not synced from here |
| Hermes custom skills (15, private project notes) | `kolaf/hermes-skills` (**private**) | `hermes-skills-sync` (per-file three-way sync, conflicts reported, deletions opt-in) |
| Hermes memories | the Hindsight server (`hermes-hindsight-api.kolaf.net` (the API; `hermes-hindsight.kolaf.net` is only the web UI), bank `hermes`) | nothing to sync: point `~/.hermes/hindsight/config.json` at the same server |
| Handy settings (endpoint, key, prompts in use, custom words, snippets, language) | each machine's own `settings_store.json` | prompts: `fork/scripts/install-prompts.py`; the rest by hand (no export tool yet) |
| Secrets: Handy API key, Hindsight key, Hermes auth | never in git | 1Password (`op` is installed on the home WSL); not automated yet |
| `user/settings.talon` (speech timeout) | the machine only | recreate by hand |

Not decided yet: whether `kolaf/dotfiles` (public Ansible setup for WSL) becomes the provisioning hub. It must not hold anything private.
Dropped on purpose: `talon-ai-tools`, replaced by the Handy `edit` prompt, so the GPT key is no longer in Talon at all. The old
copy and its key file were moved (not deleted) to `%APPDATA%\talon\disabled\`.

## Where data lives

| | Installed (Windows) | Portable | Linux |
|---|---|---|---|
| Settings, history, models, logs | `%APPDATA%\com.pais.handy\` (logs in `%LOCALAPPDATA%\com.pais.handy\logs`) | `Data\` next to the exe | `~/.local/share/com.pais.handy/` (expected) |

Back up `settings_store.json` before experimenting; `install-prompts.py` does it for you.

## Merging upstream

```
git remote add upstream https://github.com/cjpais/Handy.git     # once
git fetch upstream && git merge upstream/main
```

Our logic is in `src-tauri/src/extras.rs`; the hooks into upstream files that may conflict are:
`actions.rs` (`process_transcription_output`, `SwitchAction`/`announce_setting_change`, vocabulary parsing, `ACTION_MAP`),
`settings.rs` (`Snippet`, `LLMPrompt.examples`, new fields and bindings, `default_post_process_prompts`),
`shortcut/mod.rs` (prompt commands take `examples`; a few new commands), `lib.rs` (CLI handling, command registration),
`cli.rs`, `signal_handle.rs`, `overlay.rs` (notice, caption, window heights; the Windows geometry tests encode them), and
frontend: `LanguageSelector.tsx`, `ModelSettingsCard.tsx`, `PostProcessingSettings.tsx`, `RecordingOverlay.tsx/.css`,
`Snippets.tsx`, `PostProcessMinWords.tsx`, `settingsStore.ts`, `bindings.ts` (edited by hand here), `en/translation.json`
(other languages fall back to English for the new strings). Run `cargo test --release --lib` after a merge: it covers these.

## Known limits

- New UI text exists in English only; `bun run check:translations` therefore reports the new keys as missing in the other 24
  languages (expected; the app falls back to English).
- The vocabulary and snippet tags and the format commands depend on the model following the prompt; the bench measures this.
- Wayland: global shortcuts must be bound to the flags in the desktop settings; typing into other apps needs `wtype`,
  `dotool` or `ydotool` (X11: `xdotool`).
- The AppImage format has not been built.
