# What's new in this build

This build (`0.9.7-hotkeys.1`) adds features on top of Handy. Details for every item are in `FORK.md` in the repository; this page is the short version. Shortcuts are the defaults; change them on the General and Post-Processing pages. **Dictation is only post-processed with the post-processing shortcut** (or `handy --toggle-post-process`); the plain Transcribe shortcut skips the language model.

## Switching quickly

- **Swap language** (`Ctrl+Alt+L`): swaps the language with the _Alternate Language_ (General page).
- **Prompt picker** (`Ctrl+Alt+P`): numbered list of your prompts; press a number or click. The window never takes focus.
- **Model picker** (`Ctrl+Alt+M`) and `handy --set-model NAME`: switch the speech model among the downloaded ones. **Model per language** (General, optional) switches the model when the language changes.
- **Language model**: the provider "Local (llama-server)" runs a model on this computer. `handy --set-llm local` or `cloud` (the custom provider) switches between them.
- **Prompt per app** (Post-Processing): choose the prompt by program, for example Slack → _Informal message_. The app is the one where you **started** speaking. Windows, and Linux with X11.
- **Format at the end** (_Super_ prompt only): end a dictation with "format as email", "as a list", "make it formal", "som punktliste" and the text is formatted that way; the command is left out.
- **Keeps dictation out of the wrong window** (Advanced, on): if you changed window while the text was being prepared, it goes to the clipboard and a notice says so.
- **Paste last dictation** (`Ctrl+Alt+V`).

## Working on text you already have

- **Reformat selection** (`Ctrl+Alt+F`): run the selection through the selected prompt and replace it.
- **Transform** (`handy --transform ID`, Talon "make that formal"): the selection, or the last dictation, through a one-purpose prompt (`t_*`).
- **Scratch and redo** (Talon "scratch dictation", "redo as email"): take the last dictation back with Backspace presses, or process the same recording again with another prompt. Same window, at most 5 minutes old; use it right after dictating.
- **Re-run with next prompt** (`Ctrl+Alt+R`). **Edit by instruction** (Talon "edit this") and **reply with context** (Talon "reply to this") use the `edit` and `reply` prompts.

## Meetings (sidebar)

- Choose or drop audio files or a folder, pick the speech model and language; Handy writes minutes and saves them with the transcript. `handy --meeting-minutes FILES`, or Talon "transcribe meeting" in Explorer.
- **Latest recording** (button, `--meeting-latest`, Talon "transcribe latest meeting"): the newest file in your recorder folder (OBS Studio...) with the files that belong to it.
- **Long silences are cut** first so the speech model has nothing to invent text over (checkbox; local transcription only).
- **Identify the speakers** (checkbox, `--meeting-speakers`, Talon "... with speakers"): the audio goes to the post-processing endpoint's diarizing model, the transcript gets `Speaker 1:` labels, and the minutes attribute views to them. A name is used only if the conversation makes it certain. **The recording leaves the computer.**

## Teaching Handy your words

- **By dictation**: spell a word after saying it, or say "add to vocabulary X".
- **Learn from correction** (`Ctrl+Alt+K`): fix a dictation by hand, select the fixed text, press the shortcut. **Learned Corrections** (Advanced) are _Always_ or _Hint_ rules; click to switch.
- **Snippets** (Advanced): "insert my signature"; the text is inserted locally.
- **Learn a repo** (`--learn-repo FOLDER`, Talon "learn this repo"), `--import-words FILE`.
- **Sync between computers** (`--sync-lists FILE`): words, snippets, corrections, your own prompts, the per-app and per-language rules and a few switches, merged with a JSON file you keep in git. It only adds.

## The Activity page

Next to History. Every notice (learned, synced, model or language-model switched, meeting finished or failed, reformat problems...) is kept here with the details behind it. Click an entry.

## Writing prompts

- Variables: `${output}` `${vocabulary}` `${corrections}` `${snippets}` `${clipboard}` `${examples}` `${app}` `${title}` `${language}` `${date}` `${time}` `${weekday}`. Window titles are untrusted text.
- The **Examples** box is for fixed structures. A prompt whose id starts with `t_` is a transform and is left out of the picker and of "re-run".

## When the model is not available

If the language model cannot be reached, Handy cleans the text locally (spoken punctuation, capitals) and says "Offline: basic cleanup only". **Skip the model for short dictations** (Post-Processing) does the same for dictations under a number of words.

## Talon commands

Handy is driven by voice through the community fork (`kolaf/handy/`); Talon switches its speech off while Handy records. Wake Talon with `Ctrl+PageUp` after a restart. The full table is in `FORK.md`.

- **Rewrite:** "make that formal|informal|shorter|fuller|clearer", "fix that up", "translate that to norwegian|english", "bullet that", "summarize that" (the selection, or the last dictation).
- **Dictate in a mode:** "dictate as email|message|note|meeting|document|formal|informal|simple", "reply to this", "edit this" (select first, then speak).
- **Undo and redo:** "scratch dictation", "redo as <mode>", "redo raw", and the context-aware "scratch that".
- **Models:** "model parakeet|norwegian|whisper small...", "model picker", "language model local|cloud".
- **Meetings:** "transcribe latest meeting|recording" (add "with speakers"), and in Explorer "transcribe meeting [norwegian|english]".
- **Terminal:** "learn this repo"; say "terminal help" for the rest.

## Command line

`handy --swap-language`, `--prompt-picker`, `--model-picker`, `--set-model NAME`, `--set-llm local|cloud`, `--set-language no`, `--set-prompt email`, `--meeting-minutes FILES`, `--meeting-latest`, `--meeting-speakers`, `--reformat`, `--transform ID`, `--redo-with ID`, `--scratch-last`, `--use-prompt-once ID`, `--rerun`, `--paste-last`, `--learn`, `--learn-repo FOLDER`, `--import-words FILE`, `--sync-lists FILE`. They work with a running Handy and can be combined with `--toggle-post-process`, for example from a window-manager key binding or Talon.
