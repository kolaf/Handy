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
| **Next prompt** | `ctrl+alt+p` or `handy --next-prompt` | Post-processing page |
| **Set exact values** | `handy --set-language no`, `handy --set-prompt email`; combinable with each other and a toggle | n/a (flags only) |
| **Overlay feedback** | A caption under the controls shows the language (and the prompt, if post-processing runs); a short notice appears after a switch; the activity bars scale to your recent volume | Settings: overlay style |
| **Vocabulary by dictation** | Spell a word after saying it, or say "add to vocabulary X" / "legg til i ordlisten X" | Custom Words (Advanced) |
| **Snippets** | Say "insert my signature" and the stored text appears | Snippets (Advanced) |
| **Prompt variables** | `${vocabulary}` `${snippets}` `${clipboard}` `${examples}` in any prompt | Prompt editor |
| **Prompt examples** | Optional examples box per prompt | Prompt editor |
| **Re-run with next prompt** | `ctrl+alt+r` or `handy --rerun` | Post-processing page |
| **Paste last dictation** | `ctrl+alt+v` or `handy --paste-last` | General page |
| **Offline fallback** | Automatic when the language model call fails | n/a |
| **Skip the model for short dictations** | Number of words below which the model is skipped | Post-processing page (0 = off) |
| **Ten default prompts** | Baked in for fresh and portable installs | `fork/prompts/` |
| **Guide page** | Sidebar > Guide: what is new in this build and a quick reference | `src/content/fork-guide.md` (keep it in step with this file) |

Default shortcuts are `ctrl+alt+l`, `ctrl+alt+p`, `ctrl+alt+r`, `ctrl+alt+v`; rebind them in Settings. On Wayland, desktops
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
There is no native `${selection}`: copy the text first (a Talon command can do this, see `fork/talon/`).

**Examples field.** For fixed structures: put the structure with `[placeholders]` in the prompt, then give one or two
`Dictation: ... / Result: ...` pairs separated by `---`. The *Document template* prompt is a working sample.

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
handy --swap-language          handy --next-prompt
handy --rerun                  handy --paste-last
handy --set-language CODE      handy --set-prompt ID                  (combinable, also with a toggle)
handy --set-language no --set-prompt email --toggle-post-process
```

Prompt ids: `simple`, `informal_message`, `email`, `note`, `meeting`, `super`, `reply`, `document`, `informal_text`,
`formal_text`, plus any you create.

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
  The checker itself is verified; **model behaviour is not**, until you run it against your model.

## Talon

`fork/talon/` has a Talon bridge (`handy.py`, `handy.talon`, README): one key that starts/stops a dictation and mutes Talon
meanwhile, voice commands for the flags, a "handy reply" command that copies the selection first, and optional per-app prompt
switching. **Untested against a real Talon install.**

## Building and installing

Windows prerequisites: Visual Studio 2026 with the *Desktop development with C++* workload (MSVC x64/x86 build tools),
Windows SDK 10.0.26100, Rust (MSVC), Bun, CMake, Vulkan SDK. Run builds from a Visual Studio developer prompt.
**Close Handy first**: an open `handy.exe` cannot be replaced and the build fails.

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
