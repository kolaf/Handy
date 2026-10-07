# This fork

This is a fork of [cjpais/Handy](https://github.com/cjpais/Handy) that has deliberately diverged. It keeps the MIT license
(see `LICENSE`). Upstream changes are merged in by hand; patches may go back upstream later if they open up.

- Product branch: `dev/hotkeys-build` (the older `feature/*` branches are stale; ignore or delete them).
- Version label: `0.9.8-hotkeys.1` (set in `package.json`, `src-tauri/Cargo.toml`/`Cargo.lock`, `src-tauri/tauri.conf.json`).
  It sorts below the stock `0.9.8`, so **turn off "Update checks" in the app**, or it will offer to replace this build.
- Everything below is **unverified against a real language model unless it says otherwise**; run the prompt bench (see
  [Prompts](#prompts-and-the-test-bench)).

> **Setting up a new Windows + WSL machine (for example a work computer): see [`fork/SETUP.md`](fork/SETUP.md).**

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
| **Scratch and redo** | `handy --scratch-last`, `--redo-with ID` (Talon: "scratch dictation", "redo as email", "redo raw") | n/a |
| **Paste guard** | Automatic: a dictation is not pasted into another window than where you started; it goes to the clipboard | Advanced: Keep dictation out of the wrong window |
| **Format commands** | End a dictation with "format as email" / "som punktliste" | the `super` prompt only |
| **Speech model switching** | `ctrl+alt+m`, `handy --model-picker`, `handy --set-model NAME` (Talon: "model parakeet") | General page |
| **Model per language** | Changing the language also switches the model | General: Model per language |
| **Language model switching** | `handy --set-llm local\|cloud\|NAME` (Talon: "language model local") | Post-Processing: provider "Local (llama-server)" |
| **Meeting minutes** | Meetings page, `handy --meeting-minutes FILES`, Talon "transcribe meeting" | Meetings page |
| **Latest recording** | "Use the latest recording", `handy --meeting-latest`, Talon "transcribe latest meeting" | Meetings page: recorder folder |
| **Cut silence** | Automatic before local meeting transcription | Meetings page checkbox |
| **Speakers** | Checkbox, `--meeting-speakers`, Talon "... with speakers"; names when the conversation says them | Meetings page |
| **Activity page** | Sidebar, next to History: every notice with its details | n/a |
| **Downloadable builds** | Tag `build-N` runs the GitHub workflow: Windows portable zip and Ubuntu `.deb` | `.github/workflows/fork-build.yml` |
| **Setup guide** | `fork/SETUP.md`: the whole voice setup on a new Windows + WSL machine | n/a |
| **23 default prompts** | Baked in for fresh and portable installs: the dictation prompts, the `t_*` transforms and the two meeting-minutes prompts | `fork/prompts/` |
| **Guide page** | Sidebar > Guide: what is new in this build and a quick reference | `src/content/fork-guide.md` (keep it in step with this file) |

Default shortcuts are `ctrl+alt+l` (swap language), `ctrl+alt+p` (prompt picker), `ctrl+alt+m` (model picker), `ctrl+alt+r` (re-run),
`ctrl+alt+v` (paste last), `ctrl+alt+f` (reformat selection) and `ctrl+alt+k` (learn from correction); rebind them in Settings. Dictation is
post-processed only with the **post-processing** shortcut (`handy --toggle-post-process`); the plain Transcribe shortcut skips the language model
(a common reason for "post-processing is ignored"). On Wayland, desktops
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

**Format commands.** The `super` prompt (only that one) looks at the end of a dictation: an explicit command such as "format as email",
"as a list", "make it formal", "som e-post", "som punktliste" or "gjør det formelt" is applied to the whole text and left out, and it beats
the prompt's own guess about what kind of text it is. Without a command the prompt chooses from the content (a message to a person ->
informal, something for a colleague or institution -> email, things to remember -> bullet list, a recap -> meeting notes, code -> kept
literal). The other prompts do not look for commands. It is a prompt rule, so it depends on the model following it; the bench cases
`format_command` and `format_command_no` check it. To use another prompt for one dictation whatever is selected, use Talon "dictate as ..." or
`--use-prompt-once ID`.

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
`kolaf/` are written for Windows Terminal; Linux copies of the terminal, yazi and Explorer-style commands exist (`terminal_linux.talon`,
`yazi_linux.talon`, `handy/explorer_linux.talon`, matching the community `tag: terminal` and `tag: user.file_manager`). They are untested:
there is no Linux machine with Talon to try them on.

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

**Meeting minutes.** The old standalone `meeting-transcriber` script (`~/dev/openai-transcribe`) is now part of Handy: sidebar
page **Meetings** (choose files or a folder, or drop them on the page; pick the speech model for the job; language; output folder), or
`handy --meeting-minutes "part1.m4a;part2.m4a" [--meeting-language no] [--meeting-model parakeet]`, or Talon: select the files in Explorer and say
"transcribe meeting" (also "transcribe meeting norwegian" / "english"). The audio files (MP3, M4A/AAC,
WAV, FLAC, OGG; several parts of one session in order, or a folder, whose audio files are used in natural name order) are decoded and resampled to 16 kHz mono, cut into 5-minute chunks and
transcribed with a local speech model, so the recording never leaves the computer: the one Handy has loaded, or the one chosen for
meetings (it is loaded for the job and the dictation model is put back afterwards; dictation during the job uses the meeting model). (The script transcribed in the
cloud; that option was left out on purpose, so there are no extra endpoint or key settings.) The transcript goes to the
post-processing model with the minutes prompt (`t_meeting_minutes_en` / `_no`, both general
prompts for meetings and conversations, editable in the prompt list), then a short title is made, and
`<folder>/<time>-<title>.md` plus `...-transcript.txt` are saved (folder: setting, default `meeting_notes` in the home folder) and
the minutes opened. Progress is shown on the page and the result on the Activity page ("Meeting minutes"). Files are never deleted
(the script offered to). One job at a time; Cancel stops after the current chunk. Decoding, mixing down and resampling are
tested on a generated WAV file; a long local run has not been timed. The speech model is shared with dictation, so dictating
while a meeting is being transcribed makes both wait.

**Latest recording (OBS Studio and others).** Set the recorder's output folder on the Meetings page (empty means the `Videos` folder in the
home folder, which is OBS Studio's default on Windows and Linux). `handy --meeting-latest [--meeting-single] [--meeting-folder DIR]`, the page
button "Use the latest recording", and Talon "transcribe latest meeting" / "transcribe latest recording" take the newest file in that folder
(not its sub-folders) together with the files that belong to the same recording: Handy looks at the 40 newest audio/video files (mp3,
m4a, aac, wav, flac, ogg, **mkv, mp4, mov**; the first audio track is used, AAC is what OBS writes by default), reads each file's length
from its header, and a file belongs to the group when it ended within N minutes (default 5, setting) of the moment the group starts.
That is exact for OBS's automatic file splitting, where each part starts when the previous one is closed, and it keeps two separate
meetings apart. If a length cannot be read, only files written within N minutes of each other are grouped. A newest file that was
written less than 20 seconds ago is refused ("the recording may still be running"). MKV files with Opus audio cannot be decoded; set OBS
to AAC. Everything is plain file-system code, so it behaves the same on Windows and Linux; the Talon commands do not touch the clipboard.

**Cutting silence (local meeting transcription, on by default).** Speech models invent text over silence and noise, so before the
local transcription the audio goes through the same Silero voice detector as dictation (sensitivity 0.3, so quiet speakers are
kept). Pauses under 1 s stay, 0.3 s is kept around speech, stretches of speech are packed into chunks of at most 5 minutes so a chunk
boundary is always in a silence, and joined stretches get 0.4 s of silence between them. The Activity entry says how many minutes
were kept. If the detector fails, the audio is chunked at fixed lengths as before; a recording without any speech is refused. The
Meetings page checkbox "Cut silence before transcribing" turns it off (for example for a very quiet speaker). Not used with speaker
identification, where the cloud model does its own detection; the transcript has no timestamps in the local path, so nothing else
depends on the original timing. Not tried on a real recording yet.

**Speakers in meeting minutes (cloud, off by default; untested against the real endpoint).** The Meetings page checkbox "Identify the
speakers", `--meeting-speakers`, and Talon "transcribe latest meeting with speakers" / "transcribe meeting [norwegian|english] with
speakers" send the audio (16 kHz mono WAV, ten-minute pieces under the 25 MB limit) to `POST {post-processing base URL}/audio/transcriptions`
with the post-processing API key, model `diarize_model` (default `gpt-4o-transcribe-diarize`), `response_format=diarized_json` and
`chunking_strategy=auto`. No local speech model is used or loaded. The transcript becomes `[mm:ss] Speaker N: text` (consecutive turns of one
speaker are joined) and the minutes prompts attribute views, decisions and actions to the labels without guessing names. Labels from the API
("A", "B") only hold within one request, so for the following pieces Handy sends an 8 s voice sample of up to 4 known speakers
(`known_speaker_names[]` / `known_speaker_references[]`) to keep "Speaker 1" the same person. Names are not looked up; say them in the
minutes by editing. If the endpoint (a gateway in front of the model, for instance) drops the segments, the transcript is plain text and the Activity entry says so.
Privacy: unlike plain transcription, the recording leaves the computer.

**Meetings in a SilverBullet project (optional).** [SilverBullet](https://silverbullet.md) is a self-hosted note tool whose pages are plain Markdown
files with an HTTP API (`GET|PUT /.fs/<page>.md` with `Authorization: Bearer <token>`; in multi-space mode the space name is part of the address,
for example `http://host:3000/notes`). A **project** is a page with `tags: project` in its frontmatter; its tasks are `* [ ] ...` lines, and tasks on
other pages that link to it (`[[Saga]]`) belong to it (SilverBullet's "Linked Tasks" widget shows them on the project page). Setup: Meetings page,
"SilverBullet project": space address, API token (the account needs write access; keep the token like a password, because write access to a space
can run code there), the folder for meeting pages (default `Meeting Notes`) and the hashtag for proposed tasks (default `fromMeeting`); "Load projects"
lists the pages tagged project. With a project chosen (page dropdown, `--meeting-project NAME`, Talon "transcribe latest meeting for saga"):
1. The project is read first (a wrong name or an unreachable server stops the job before the long transcription). Handy reads the project page,
   its open tasks and the open tasks on other pages that link to it (up to 400 pages, none from `Library/`).
2. The minutes prompt gets that text as context (as data: to spell names right and say which tasks came up).
3. A second step asks the language model for a JSON object with `proposed_tasks`, `possibly_completed` and `new_information`. The reply is checked in
   code: single bounded lines, no links or hashtags, and a "possibly completed" entry must match a task that really is open (the text on the page is
   used, not the model's wording).
4. The local files are saved as before. Then one **new** page is created in the space, create-only (`If-None-Match: *`, a taken name gets "(2)"):
   `Meeting Notes/<date> <title>` (frontmatter `tags: meeting`, `project`, `date`, `createdBy: Handy`; the minutes; "Proposed tasks (from the
   meeting, not reviewed)" as `* [ ] ... [[Saga]] #fromMeeting`, which the project's Linked Tasks widget shows without the project page being edited;
   "Possibly completed (existing tasks; nothing was changed, tick them yourself)", which only quotes the tasks; "Proposed new information for
   [[Saga]] (not added to the project page)"). **The transcript is not put into the space**: it stays in the local `-transcript.txt` next to the
   local minutes. The page that opens afterwards is the SilverBullet one. Existing pages and tasks are never edited, ticked or deleted; a failure
   in SilverBullet does not fail the job (the local files are complete, and the Activity entry says what went wrong). Without a project nothing is
   sent to SilverBullet.
5. **The same layout as your own minutes.** `fork/silverbullet/Meeting Minutes.md` is a SilverBullet page template (tagged `meta/template/page`,
   command "Meeting: New Minutes", suggested name `Meeting Notes/<date> Meeting`; created in the space as `Templates/Meeting Minutes`; after creating
   it run "System: Reload" once). It has the frontmatter Handy writes (`tags: meeting`, `project`, `date`) and the sections Summary, Attendees,
   Decisions, Discussion, Action items (as real `* [ ]` tasks when you write them yourself) and Open questions and risks. The automatic minutes are
   told to use exactly these sections (Norwegian headings for Norwegian meetings: Sammendrag, Deltakere, Beslutninger, Diskusjon, Oppfølgingspunkter,
   Åpne spørsmål og risikoer), with the action items as plain bullets: the checkable tasks of an automatic page are the proposed ones further down.
6. **Safety.** A page of a space can run code (`${...}` expressions, `<!-- #lua -->` directives, space-lua code blocks) in the browser of whoever opens
   it, and the minutes come from a language model reading speech, so every piece of text is neutralised before it is written (`${` becomes `$ {`,
   `<!--` and code fences are broken up); page titles follow SilverBullet's name rules. Privacy: the project text goes to the post-processing model
   like the transcript does.
Tested against the real space with the page `Saga` and two throwaway pages (the pages were created, read back, and `Saga.md` kept the same hash);
the language-model steps were tried with a stand-in model on a transcript that contained an injection attempt (it produced nothing from it). Not yet
tried: a full run from a real recording, and the Windows build talking to the space over Tailscale.

**Testing the SilverBullet features.** Never against a real notes space: `fork/silverbullet/dev-instance.sh start` runs a throwaway SilverBullet
(the server release, in multi-space mode like the real one: space `notes` under `/notes`, a per-account API token, invented notes including a project
`Saga`) on `http://127.0.0.1:3010/notes`; `stop` and `reset` do what they say. The opt-in tests that talk to a space (`live_space`, `live_journal`,
and `SB_WRITE=1` for the meeting pages) take `SB_URL` and `SB_TOKEN` from the environment (see the script's header).

**Dictating into today's SilverBullet journal ("update journal").** The same SilverBullet settings (address, token) plus a journal folder (default
`Journal`, pages named `YYYY-MM-DD`, SilverBullet's own default; a missing page is created with `tags: journal` and `date`). Start it with Talon
"update journal" (`handy --update-journal --toggle-post-process`) or the button "Dictate into today's journal" on the Meetings page: an ordinary
post-processed dictation starts, you ramble about what you did and what should be done tomorrow (or ask for a change: "move the vendor call to
Thursday", "I finished that task"), and you stop with your Handy key. Nothing is pasted. Instead:
1. Today's page is read from SilverBullet together with its version (`ETag`).
2. The language model gets the page body, the dictation, today's and tomorrow's date and the lists of what exists in your space (projects, pages,
   tags in use; read from the space and kept for ten minutes; the journal and meeting folders are left out of the page list, but their 40 most recent pages are
   listed by name so that a meeting or yesterday's entry can be linked), and returns **the complete updated
   page body**. A journal page is short and a new one is made every day, and the pages are backed up, so rewriting the whole entry is the design:
   it can add, regroup, merge duplicates and apply what you asked for. **Every update organises the whole entry under second-level headings**: the
   headings the page already has are kept and used (including ones like `## Tomorrow`), new headings are added only for items that fit none, and a
   flat list is grouped for the first time. The default headings are `## Done` (what happened, in order), `## Notes` (observations, ideas) and
   `## Tasks` (all checkbox tasks, open ones first and by due date, finished ones last); when two or more items concern the same project or person
   that exists in your space, a heading with its link (`## [[Saga]]`) is used instead, and its tasks may stay under it. The order of the headings
   stays stable and an empty heading is left out. Existing items keep their words, order within a heading, indentation and format (a nested bullet
   stays under its parent): they move to the right heading and are not rewritten. The prompt also tells it to match the page's style (bullet
   character, time prefixes, `* [ ]` tasks, `[due: ...]` attributes), to mark a task done only if you say it is done, and never to delete
   something unless you said so or it is an exact duplicate.
3. What the code does with the answer: the **frontmatter is always the old one** (the model's is ignored); **lines the model left unchanged are
   kept byte for byte** (matched one to one), so your own `${...}` expressions survive; every **new or changed line** is cleaned (control
   characters removed, `${`, `<!--` and code fences neutralised, at most 400 characters) and its **references are verified**: a `[[link]]` must
   name a page that exists (written with the page's real name; `[[Page|words]]` and `[[Page#heading]]` keep alias and heading; anything else becomes
   plain text with only the page's own name, without its folder), and a `#tag` must be a tag in use (otherwise the `#` is dropped). **Every page counts as a
   page that exists, including all meeting and journal pages, and a link that the old page already contains is kept even if the model touches its line**
   (an earlier version flattened `[[Meeting Notes/2026-10-08 Title]]` to plain text with the folder and no brackets, because the meeting folder was not in the list of
   pages; `.conflicted` copies are not pages). So what you say about an existing project, page, person or tag comes
   out as a correct link or tag, and a name that does not exist never becomes a phantom page or a new tag. An answer is refused (nothing is written)
   when it is empty, longer than 400 lines, changes nothing, or drops more than half of a page of six or more lines.
4. A copy of the page as it was goes to `<meeting notes folder>/journal_backups/` and the page is written with `If-Match`: if the page changed
   while you were dictating (you edited it in the browser), nothing is written and the dictation is put on the clipboard (and is in History).
   This is the only place where Handy changes an existing SilverBullet page.
5. **Controls.** Talon "update journal", `handy --update-journal --toggle-post-process`, or the button "Dictate into today's journal" on the Meetings page
   (sidebar, in the SilverBullet block, which also holds the address, token and the journal folder) arm journal mode and start the dictation; you
   stop with your normal Handy key, and the shortcut for cancelling (Escape) cancels it and switches journal mode off. Arming lasts 15 seconds and
   is used up by the next *post-processed* recording that starts; every other recording is an ordinary dictation, so a forgotten arming cannot
   divert a later one.
6. The Activity page lists the new or changed lines, the old lines that are gone or changed, and the page address; a notice says "Journal: N added,
   M changed or removed" or "Journal not updated".
Tested against your real space on a throwaway page in `Handy test (delete me)/` (links corrected, unknown page and tag made plain, `${...}` defused,
a replaced line reported as changed, a stale write refused with the page unchanged) and with a stand-in model on a rambling dictation with a
self-correction, an edit request and an injected instruction (the page style, links and tags were right and the injected text became a harmless
reminder line). Not yet tried: a full dictation through the real model, and an entry in your own journal style beyond the sample.

**Speaker names.** With the sub-option "Use names that are said in the conversation" (on by default when speakers are on) the
language model reads the labelled transcript once and returns a name only for a speaker whose name the conversation makes certain
(an introduction, or being addressed by name and answering). Anything else stays "Speaker N". The reply is checked in code: only
known labels, plain letters (no markup or line breaks), at most 40 characters, no name used for two speakers, no placeholders such
as "unknown". The transcript file and the minutes then use the names; the Activity entry lists which names were used. A wrong
name is possible (a name that is mentioned but belongs to a third person); check the minutes.

**Local language model.** The provider "Local (llama-server)" (default address `http://127.0.0.1:8081/v1`) sits next to the others on
the Post-Processing page. `handy --set-llm local|cloud|NAME` (Talon: "language model local" / "language model cloud") switches the
provider in use without opening the page; `cloud` means the `custom` provider, where a hosted gateway or model address is entered. Setup and benchmark
results are in `fork/SETUP.md`. Speaker identification uses the `custom` provider when "local" is selected.

**Azure AI Foundry (Azure OpenAI) as the language model.** Use the Custom provider with the v1 address
`https://<resource>.openai.azure.com/openai/v1` (a resource shown under `cognitiveservices.azure.com` accepts the same path), the resource key
as API key, and the **deployment name** as model. Handy appends `/chat/completions`, so the address must end in `/v1`: without it Azure answers
404. The old form with `/openai/deployments/<name>/...?api-version=` does not fit. Tested with the bench on 2026-10-04 (41 of 41 cases). Speaker
identification on this endpoint needs a diarizing transcription deployment on the same resource.

**Overlay.** The floating bar shows the state (recording, transcribing, processing) and short notices (language, prompt, model switched, learned,
synced ...; the caption under the bar names the language and prompt). A notice never takes over a bar that is recording. The overlay's show
counter is bumped when a show is requested (not when the main thread runs it), and the delayed hide re-checks it on the main thread: this
closes a race where the bar could vanish right after it appeared, most likely at the first dictation after startup. Not confirmed on a
machine after the fix.

**More than lists in the sync file.** `handy --sync-lists FILE` now also carries your own prompts (ids `prompt_...`; the built-in ones
come with the program), the per-app and per-language rules, and four switches ("prompt per app" on, "model per language" on, the
meeting language, speakers on). It still only adds: an entry that exists on both sides with different content stays as it is on
each machine and is reported as a conflict, and a switch from the file is taken only where this machine still has the default (so
nothing you set on purpose is overwritten, and a switch you turn off again does not travel). Paths (recorder and minutes folders), keys and
the model choice are never in the file. Old files without these parts still load.

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

**Edit by instruction.** Talon's "edit this" copies the selected text and runs `handy --use-prompt-once edit --toggle-post-process`: you then speak the change you want, stop with your Handy key, and the `edit` prompt (selected text from the clipboard, your words as the instruction) pastes the result over the still-selected text. **When it does nothing useful**, the places to look: (1) Talon copies with `edit.selected_text()` and says "Handy: N characters copied" or "nothing was copied" (the
editor did not have focus, or nothing was selected); (2) the one-shot prompt `edit` must be in the prompt list: if it is not, the selected prompt is used
and the Activity page shows "Prompt not found" (install the built-in prompts with `fork/scripts/install-prompts.py`, Handy closed); (3) the clipboard is
read when you stop the dictation, so do not copy anything else in between; if it is empty, nothing is sent to the model and the Activity page says "Nothing to
edit"; the Handy log has the line "The prompt works on the clipboard: N characters copied"; (4) a prompt that works on the clipboard is never skipped by the
setting "Skip the model for short dictations" (an instruction such as "translate to Norwegian" is short by nature; before this was fixed, such an instruction was
only cleaned up and pasted as text). The result goes to the cursor where you are when you stop the dictation, so you can select on one page, speak the change
and click into another page before stopping.

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
conflict is logged. Invalid entries in the file are skipped. The built-in prompts are not in the file (they come from the repo:
`fork/scripts/install-prompts.py`); your own prompts, the rules and four switches are (see "More than lists in the sync file"). Keys and other
settings are not.

**Re-run with next prompt.** Takes the raw transcript of your most recent dictation from the history, advances to the next
prompt, processes it and pastes the result. Select the earlier pasted text first to replace it; otherwise the result is
inserted as a second copy.

**Offline fallback and short dictations.** When post-processing is fully configured (provider, model, prompt) but the request
fails, or when a dictation has fewer words than the minimum-words setting, a *local cleanup* runs instead of the model:
spoken punctuation (English and Norwegian: question mark/spørsmålstegn, exclamation mark/utropstegn, comma/komma,
period/full stop/punktum, colon/kolon, semicolon/semikolon, new line/ny linje, new paragraph/nytt avsnitt) becomes the
symbol, and sentences are capitalized. It is deliberately simple: a literal "the period of time" is converted too. The
overlay says "Offline: basic cleanup only" after a failure.

## Weekly upstream sync (on the Linux server, Hermes cron, Claude Code)

`fork/scripts/sync-upstream.sh` brings `cjpais/Handy` main into the fork without touching any working copy, and leaves a pull request for you to
review. It runs unattended on the always-on Linux server: a Hermes cron job `handy-upstream-sync` (schedule `30 7 * * 1`, Monday 07:30 server time,
no-agent mode, delivery to Telegram) runs `~/.hermes/scripts/handy/sync-upstream.sh`, a wrapper that pulls the repository clone `~/dev/Handy`, sets
the PATH, runs the script and prints a short result, which Hermes sends to Telegram: "no new commits", or the pull request link (or the issue link when
it did not finish) followed by Claude's **verdict and summary** (see "The review" below). Requirements on that machine: the clone with `upstream` pointing
at `cjpais/Handy`, a GitHub token with write access to **this repository only** (fine-grained: Contents, Pull requests and Issues read and write)
in `~/.config/handy-sync/token` (mode 600; the wrapper exports it as `GH_TOKEN`, so the general `gh` login is not used), Rust, Bun, the Linux build libraries (the list under "Building and installing"), a signed-in Claude Code, `gh` logged in, and
Hermes with Telegram configured. If the build libraries are missing the script says so and stops before starting Claude.

1. Fetch upstream. Nothing new, or a branch `upstream-sync-<sha>` already on GitHub for that commit: stop.
2. In a separate worktree (`~/dev/Handy-sync`) make `upstream-sync-<sha>` from `dev/hotkeys-build` and merge upstream into it.
3. Run the checks: `bun install`, `bun run build`, `cargo test --lib`, `bun run lint`, `npx tsc --noEmit`.
4. **Clean merge and checks pass:** no AI involved. Push the branch and open a pull request into `dev/hotkeys-build`.
5. **Conflicts or failing checks:** start Claude Code headless (`claude -p`, budget `CLAUDE_BUDGET_USD`, default 10) in the worktree with instructions to
   follow this file ("Merging upstream"), keep the fork's features, adopt upstream's design where it replaced code, fix what the merge breaks and run
   the checks. It can read, edit, commit and run git (not push), cargo, bun and npx; `git push`, `gh`, `curl` and `rm -rf` are denied.
6. **The review (always, whenever there is something new).** A second headless Claude run, read-only (Read, Grep, Glob and `git log|show|diff|merge-base`),
   looks at the upstream commits and writes, for someone who does not know the upstream project: `VERDICT: MERGE|REVIEW|CAREFUL - why`, a one-paragraph
   summary of what the changes do for a user, the changes one by one in plain words, the overlap with the fork's features, what to try by hand after
   merging, and what it could not verify. The script gives it the list of files that both upstream and the fork changed and the dependency, build
   and CI files upstream touched. **MERGE** means documentation, translations, tests, CI or small isolated fixes in code the fork does not touch, no
   dependency, engine or settings-format changes, and good automated checks; **REVIEW** means user-visible changes, code the fork also changes, or
   dependency bumps that look sound; **CAREFUL** means the speech engine or audio, paste or shortcut handling, the overlay, settings migration, many
   files, a conflict Claude had to resolve, a failed state, or anything it could not judge (when unsure it must pick the stricter one). The full review
   goes at the top of the pull request, and the verdict and summary go into the Telegram message. Budget: `REVIEW_BUDGET_USD` (default 3). If the
   review run fails, the verdict is REVIEW with the commit titles instead. The review is Claude's reading of the diff, not a test: the app is never run.
7. **The script does not trust Claude's report:** it checks that the merge is finished, nothing is uncommitted, upstream is in the branch, and runs the
   checks itself again. Pass: pull request whose body says Claude resolved it, lists the conflicted files and includes Claude's summary. Fail: an
   issue with the end of the checks log (and the branch, if it has commits).

Nothing reaches `dev/hotkeys-build` without you merging the pull request. After merging, tag a build (`git tag build-N && git push <repo> build-N`)
and try dictation: the sync proves the code compiles and the unit tests pass, not that the app works.

**Answering from Telegram ("merge it").** Hermes' cron delivery sends the message but does not record it in your Telegram chat session, so the assistant
would not know what "merge the PR" refers to (it asked "which PR?" the first time). So `fork/hermes/sync-upstream-job.sh` (the job's wrapper) also writes
every pull-request or issue message into that session with `fork/hermes/mirror-context.sh` (Hermes' own `gateway.mirror.mirror_to_session`), together
with a note for the assistant: which pull request it is and what to do on a reply. The skill `handy-upstream-sync` (`fork/hermes/SKILL.md`, installed
in `~/.hermes/skills/devops/handy-upstream-sync/`) holds the procedure: "merge it" runs `fork/hermes/merge-and-build.sh <PR>`, which refuses anything but
an open `upstream-sync-*` pull request into `dev/hotkeys-build`, waits for CI (a failing check stops it unless `--ignore-checks`), merges with a
merge commit, deletes the branch, tags the next `build-N` and pushes it (this starts the CI build), and `watch-build.sh` sends a Telegram message
with the download links when the build is done. For a CAREFUL verdict, or when Claude resolved conflicts, the assistant asks for an explicit
confirmation first; "skip" closes the pull request; a question gets answered from the review at the top of the pull request. On the
server the helper scripts and the skill are symlinks into the repository clone, so a pull updates them. **The cron job's own script must be a real file**:
Hermes refuses a script whose path resolves outside `~/.hermes/scripts` ("Blocked: script path resolves outside the scripts directory"; the job failed
that way on 2026-10-06 07:30 after it had been made a symlink). So `~/.hermes/scripts/handy/sync-upstream.sh` is a two-line stub that `exec`s
`~/dev/Handy/fork/hermes/sync-upstream-job.sh`; the job's logic stays in the repository.

**Watching a run.** `tail -f ~/.cache/hv/upstream-sync.log` shows the script's steps (and the Claude session id). `fork/scripts/sync-upstream.sh --watch`
shows what Claude is doing live (its tool calls and text, from `~/.cache/hv/upstream-sync.log.claude.jsonl`). Afterwards `claude --resume <session id>`
opens the same session, from the worktree directory, to ask questions or continue. **There is no remote connection into a running job:** Claude
Code's Remote Control (`claude --remote-control`) is for interactive sessions, not for a headless `-p` run. Watching from another machine
means running the `tail`/`--watch` commands over SSH; the pull request or issue on GitHub is the result you get notified about.

Run it by hand any time: `fork/scripts/sync-upstream.sh` (on a terminal it shows everything; `DRY=1` prints the push and the pull request instead
of doing them; set `UPSTREAM=upstream` where the remote has that name). On the server, `hermes cron run <job id>` runs the job on the next scheduler
tick, `hermes cron list` shows the schedule and last run, `hermes cron edit` changes the time, `hermes cron pause|resume|remove` stop it. The checks
pass on the server (368 tests; first compile about 15 minutes, later runs reuse the build cache). Tested against throwaway repositories (clean merge,
a conflict resolved by a stub, a conflict nobody resolves, and a conflict resolved by the real `claude`) and a no-news run through Hermes;
**not yet run against the real upstream with a real conflict.** The earlier Windows scheduled task was removed.

## Command-line flags

Sent to the running instance (a second `handy` process forwards them and exits). If Handy is not running they just start it.

```
handy --toggle-transcription | --toggle-post-process | --cancel      (upstream)
handy --swap-language          handy --prompt-picker        handy --model-picker
handy --rerun                  handy --paste-last           handy --learn
handy --reformat               handy --transform ID         handy --use-prompt-once ID
handy --update-journal --toggle-post-process      (the dictation goes into today's SilverBullet journal)
handy --scratch-last           handy --redo-with ID         (ID `raw` = the transcript without formatting)
handy --set-language CODE      handy --set-prompt ID        (combinable, also with a toggle)
handy --set-model NAME         handy --set-llm local|cloud|NAME
handy --learn-repo FOLDER      handy --import-words FILE    handy --sync-lists FILE
handy --meeting-minutes "a.m4a;b.m4a" [--meeting-language no] [--meeting-model NAME] [--meeting-speakers] [--meeting-project NAME]
handy --meeting-latest [--meeting-single] [--meeting-folder DIR] [--meeting-speakers] [--meeting-project NAME] [--meeting-language no] [--meeting-model NAME]
handy --set-language no --set-prompt email --toggle-post-process
```

Prompt ids: `simple`, `informal_message`, `email`, `note`, `meeting`, `super`, `reply`, `document`, `informal_text`,
`formal_text`, `edit`, the transforms `t_formal`, `t_informal`, `t_shorter`, `t_longer`, `t_clear`, `t_fix`, `t_to_no`, `t_to_en`,
`t_bullets`, `t_summary`, the meeting prompts `t_meeting_minutes_en` / `t_meeting_minutes_no`, plus any you create.

## Prompts and the test bench

All in `fork/prompts/`.

- `build_prompts.py` is the **source of truth**. It builds each prompt from shared steps: (1) spoken punctuation,
  (2) spelled-out letters including the NATO alphabet, (3) cleanup, self-corrections and repair of misheard words (unclear
  words are marked `[word?]`), (4) vocabulary and snippet tags, (5) the style. `python3 build_prompts.py` writes
  `handy-prompts.json` and `src-tauri/src/dev_prompts.json`.
- `dev_prompts.json` is compiled into the app: **fresh settings and portable installs start with these prompts** and `super`
  selected. Existing installs keep what they have stored (a portable update never touches `Data\`); update the built-in prompts with
  `python3 fork/scripts/install-prompts.py PATH` (close Handy first: the script checks and refuses while it runs; it makes a backup). **Give the path of the
  settings file your Handy really uses**: a portable Handy keeps it in `<folder>/Data/settings_store.json`, an installed one in `%APPDATA%\com.pais.handy`. Without a path the
  script lists the files it finds and stops if there is more than one. **Exception:** at start Handy adds the prompts that features
  depend on if they are missing from the stored list: `edit` ("edit this"), `reply` ("reply to this") and the transforms `t_*` ("make that
  formal" ...). A prompt that exists is never changed, so your edits stay, and an ordinary built-in prompt that you deleted stays deleted.
  (Before this, an old `Data\` without `edit` gave "Prompt not found" for "edit this".) Changed *text* of built-in prompts still needs
  `install-prompts.py`.
- `bench.py` runs `bench_cases.json` (41 cases) the way Handy sends a request and checks the result:
  `python3 bench.py --from-handy` (endpoint, model and key from your Handy settings), `--prompt email`,
  `--case spell_exe -v`, `--dry` (print assembled prompts), `--mock` (check the checker without a model).
  All 41 cases passed against the real model (GPT 5.4 via the hosted gateway on 2 October 2026, and again on an Azure Foundry deployment
  on 4 October; single runs with simple checks, so treat it as strong evidence and not a guarantee). Local models on an RTX 3080
  (llama.cpp, Q4_K_M): Qwen2.5-7B 34, Qwen3-4B 33, Gemma-3-12B 31, Gemma-3-4B 27 of 41, weakest on Norwegian punctuation and format
  commands and on ignoring instructions inside dictated text (see `fork/SETUP.md`). Rerun it after changing a prompt or the model.

## Talon

The Talon files live in the community fork `kolaf/community`, folder `kolaf/` (not in this repo), so a clone of the fork carries them.
`kolaf/README.md` has the setup for a new machine and how to merge upstream; `kolaf/terminal/README.md` lists the terminal commands too (say
"terminal help"). Folders: `personal/` (wake key `Ctrl+PageUp`, spoken wake commands disabled, `drowse`, `shock`), `hv/` (voice shell),
`terminal/` (shell navigation from the state file the shell hook writes, yazi, zoxide/fzf/atuin; Linux copies), `handy/` (the Handy commands
below) and `handy-bridge/` (the old bridge, disabled). **The spoken behaviour has been tried only in part; Talon loads the files without errors.**

### Voice commands for Handy

All of them are in `kolaf/handy/handy.talon` and work in any program, except the two marked. Each one runs `handy ...` (a running Handy
receives it) through `kolaf/handy/handy_integration.py`. Phrases in `<angle brackets>` come from the lists below.

| Say | What happens | Handy flag / prompt |
|---|---|---|
| `make that formal` / `informal` / `shorter` / `fuller` / `clearer` | rewrites the selection, or with nothing selected the last dictation, and replaces it | `--transform t_formal` ... `t_longer`, `t_clear` |
| `fix that up` | spelling and grammar only | `--transform t_fix` |
| `translate that to norwegian` / `english` | translates and replaces | `--transform t_to_no` / `t_to_en` |
| `bullet that` / `summarize that` | bullet list / short summary | `--transform t_bullets` / `t_summary` |
| `dictate as <prompt>` | starts a dictation that uses that prompt for this one dictation; stop with your Handy key | `--use-prompt-once ID --toggle-post-process` |
| `reply to this` | copies the selected message, then dictate the reply (the `reply` prompt, message as context) | `--use-prompt-once reply` |
| `edit this` | copies the selection, then speak the change ("shorter and friendlier, mention Thursday"); the result replaces the selection | `--use-prompt-once edit` |
| `scratch dictation` | deletes the last dictation with Backspace presses (same window, at most 5 minutes old) | `--scratch-last` |
| `redo as <prompt>` | processes the last recording again with that prompt and replaces the text | `--redo-with ID` |
| `redo raw` | replaces the last dictation by the transcript as the speech model produced it | `--redo-with raw` |
| `scratch that` / `nope that` | the community phrase made context-aware (`scratch_that.py`): takes back whichever came last, Talon's own phrase or Handy's dictation | `--scratch-last` when Handy's was newer |
| `model <model>` | switches the speech model (it must be downloaded) | `--set-model NAME` |
| `model picker` | shows the numbered list of downloaded models; say or press a number | `--model-picker` |
| `language model local` / `cloud` | switches the post-processing language model between the "Local (llama-server)" provider and the custom (cloud) one | `--set-llm local\|cloud` |
| `transcribe latest meeting` | meeting minutes from the newest recording in the recorder folder together with the files that belong to it | `--meeting-latest` |
| `transcribe latest recording` | the same for the newest file only | `--meeting-latest --meeting-single` |
| `transcribe latest meeting with speakers` | the same with speaker labels (the audio goes to the post-processing endpoint) | adds `--meeting-speakers` |
| `transcribe meeting` / `norwegian` / `english` *(in Explorer)* | minutes from the audio files or folder selected in Explorer; the language defaults to the Meetings page | `--meeting-minutes FILES [--meeting-language no]` |
| `transcribe meeting [norwegian\|english] with speakers` *(in Explorer)* | the same with speaker labels | adds `--meeting-speakers` |
| `update journal` | starts a dictation that is added to today's SilverBullet journal page, in its style and with links to your pages and tags; stop with your Handy key | `--update-journal --toggle-post-process` |
| `transcribe latest meeting for <project>` / `... with speakers for <project>` | the same in the context of a SilverBullet project (a spoken name; it only has to be unambiguous) | adds `--meeting-project NAME` |
| `transcribe meeting for <project>` *(in Explorer)* | the same for the selected files | adds `--meeting-project NAME` |
| `learn this repo` *(in a terminal, `terminal/`)* | adds the project's names and terms to Custom Words | `--learn-repo FOLDER` |

The commands that copy the selection first (`reply to this`, `edit this`, the transforms with a selection) press Ctrl+C, or Ctrl+Shift+C in
a terminal; see "How the selection is copied". `scratch dictation`, `redo` and a transform on the last dictation press no copy key, so they are
safe in terminals.

### SilverBullet in the browser (`kolaf/silverbullet/`)

About 40 commands that all start with **"silver"** and send SilverBullet's default keyboard shortcuts (Windows and Linux; `Mod` = Ctrl; `kolaf/silverbullet/README.md`
has the whole table with each key). They are active in every browser, because SilverBullet's tab titles are only page names, so Talon cannot tell its tab from another
one; the prefix keeps them from colliding with anything else. Groups: finding and moving ("silver page", "silver open <text>", "silver commands", "silver run <text>",
"silver home", "silver back", "silver tree", "silver graph", "silver tab" with Rango and the setting `user.kolaf_silverbullet_url`), journal ("silver journal today|previous|next",
"silver quick note", "silver from template"), writing ("silver bold|italic|strike through|quote|make list|delete line|indent|outdent|comment|marker"), tasks and outline
("silver task", "silver move up|down|left|right", "silver fold"), pages through the palette ("silver rename|delete|copy page") and system ("silver export", "silver reload").
**Checked** against a real SilverBullet 2.12.0 in a headless Chrome (the throwaway instance, see "Testing the SilverBullet features"): the page picker, meta picker, tag picker, command palette,
journal today and previous, quick note, bold, italic, strike through, delete line and indent. `Ctrl-g h` (home), which the documentation lists, did not work there, so "silver home" runs
"Navigate: Home" through the palette, which did. **Not working or not seen working in the test:** "silver quote" (`Ctrl-Shift-.`) and "silver task" (`Ctrl-. t`); the rest come from
SilverBullet's source and are untried. Not verified by voice at all, and the files could not be loaded into Talon on this machine.

### The lists you can edit

- `kolaf/handy/prompts.talon-list` (what you can say after `dictate as` and `redo as`; the value is a prompt id): simple, plain -> `simple`;
  message, chat -> `informal_message`; email -> `email`; note -> `note`; meeting -> `meeting`; document -> `document`; formal -> `formal_text`;
  informal -> `informal_text`. Add your own prompt's id with a spoken name.
- `kolaf/handy/models.talon-list` (what you can say after `model`; the value only has to be part of the model's id or name, and it must be
  downloaded): norwegian -> `nb_ggml`, parakeet, whisper small|medium|large. If two models match, Handy lists them and changes nothing; add
  an exact name to the list.

### How Talon and Handy talk

- **Talon to Handy:** `run_handy` starts `handy.exe` with the flags; a running Handy picks them up (single-instance forwarding) and the second
  process exits. The program is the Talon setting `user.kolaf_handy_path` (default `D:/Handy/handy.exe`; if the file does not exist, `handy` from
  the PATH). Override it in a `.talon` file of your own: `settings():` `user.kolaf_handy_path = "C:/path/to/handy.exe"`.
- **Handy to Talon:** while recording Handy rewrites `%USERPROFILE%\.cache\hv\handy-state.txt` every 3 seconds ("recording" or "idle" plus a time).
  Talon polls it, switches its speech off while Handy records and on afterwards (only if it was Talon that switched it off; a "recording" older than
  12 seconds counts as a crash leftover). Setting `user.kolaf_mute_during_handy` (default on) turns this off. `handy-paste.txt` carries the time of
  Handy's last undoable paste, which `scratch that` compares with Talon's own phrase history.
- **Linux:** `terminal_linux.talon`, `yazi_linux.talon` and `explorer_linux.talon` repeat the terminal and Explorer-style commands for Linux
  terminals and file managers (community `tag: terminal` and `tag: user.file_manager`). Untested.

### Things that go wrong

- Talon starts asleep after a restart; `Ctrl+PageUp` wakes it. A phrase said while it sleeps is rejected and nothing happens (the rejected
  recording is in `%APPDATA%\talon\recordings\<month>\reject`, named after what Talon heard).
- Talon is quiet while Handy records, so say nothing to Talon until you have stopped the dictation.
- A misheard command does nothing; the recordings folder shows what Talon thought it heard, and the phrase can be changed in the `.talon` file.
- A transform or redo that cannot run (other window, too old, nothing to take back) says why on Handy's Activity page.

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
`bun run tauri build --bundles deb --config src-tauri/tauri.fork.conf.json` (that file only turns off updater artefacts, which need a signing key). Install with
`sudo apt install ./Handy_*_amd64.deb` and `xdotool` (X11). Unverified on a real desktop and on Ubuntu 26.04.

### Downloadable builds (GitHub Actions)

`.github/workflows/fork-build.yml` builds the Windows portable zip (tests, `tauri build --no-bundle`, packaging with the VC++ runtime and
ONNX Runtime DLLs; the job checks that the key files are in the zip) and the Ubuntu 22.04 `.deb` (through upstream's `build.yml`, unsigned).
Start it by pushing a tag: `git tag build-N && git push git@github.com:kolaf/Handy.git build-N` (about 25 minutes). A tag build publishes a
release `fork-<commit>` marked latest, so these addresses always serve the newest build:
`https://github.com/kolaf/Handy/releases/latest/download/Handy_amd64.deb` and `.../Handy-portable.zip` (the versioned names are attached
too, with a `.sha256`). Without a tag, a push to `dev/hotkeys-build` that touches the workflow file builds and keeps the files as workflow
artefacts for 30 days; the manual "Run workflow" button works only once the file is on the default branch. Nothing is signed. The zip is
smaller than a local build and the `.deb` has not been installed on a machine yet.

**Unsigned builds are blocked** by Defender SmartScreen on machines whose policy forbids bypassing it, and signing would not
help quickly (reputation builds with usage, per file). See the discussion in the project history; options are an IT-approved
folder, the official signed Handy with our prompts and `fork/scripts/handy-profile.ps1`, or Linux.

## Syncing between machines

| What | Lives in | How it gets to a new machine |
|---|---|---|
| Handy code, prompts, `hv`, docs, build scripts | `kolaf/Handy`, branch `dev/hotkeys-build` (public) | `git clone`; deploy a build with `fork/scripts/deploy-portable.ps1`, or install the `.deb` |
| Talon user files: wake key, `shock`/`drowse`, `sleep.py`, voice shell commands, disabled Handy bridge | `kolaf/community` (public), folder `kolaf/` | clone the fork into Talon's `user/` folder; see `kolaf/README.md` there |
| Other Talon packages (Cursorless, Rango) | their own upstream repos (old checkouts) | listed in `kolaf/README.md`; not synced from here |
| Handy settings | each machine's own `settings_store.json` | built-in prompts: `fork/scripts/install-prompts.py` (Handy closed); words, snippets, corrections, own prompts, per-app and per-language rules and four switches: `handy --sync-lists` with `handy-lists.json` in the private dotfiles repo; endpoint, key, shortcuts, paths and models by hand |
| Secrets: Handy API key, Hermes auth | never in git | a password manager; not automated |
| `user/settings.talon` (speech timeout) | the machine only | recreate by hand |

`kolaf/dotfiles` (Ansible; now **private**) provisions WSL: terminal tools, Hermes with `hv`, the Talon shell hook,
and optionally Handy, Talon and `op` on a native Linux desktop. It holds no secrets (keys come from a password manager). The whole-machine order is in `fork/SETUP.md`.
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

Our logic is in `src-tauri/src/` (`extras.rs`, `learn.rs`, `context.rs`, `picker.rs`, `model_switch.rs`, `meeting.rs`, `activity.rs`,
`listsync.rs`, `repo_words.rs`); new dependencies are symphonia (audio decoding), base64, x11rb (Linux) and reqwest's `multipart`. The hooks into upstream files that may conflict are:
`actions.rs` (`process_transcription_output`, `SwitchAction`/`announce_setting_change`, vocabulary parsing, `ACTION_MAP`),
`settings.rs` (`Snippet`, `LLMPrompt.examples`, new fields and bindings, `default_post_process_prompts`),
`shortcut/mod.rs` (prompt commands take `examples`; a few new commands), `lib.rs` (CLI handling, command registration),
`cli.rs`, `signal_handle.rs`, `overlay.rs` (notice, caption, show/hide counter, window heights; the Windows geometry tests encode them), and
frontend: `LanguageSelector.tsx`, `ModelSettingsCard.tsx`, `PostProcessingSettings.tsx`, `RecordingOverlay.tsx/.css`,
`Snippets.tsx`, `PostProcessMinWords.tsx`, `settingsStore.ts`, `bindings.ts` (edited by hand here), `en/translation.json`
(other languages fall back to English for the new strings). Run `cargo test --release --lib` after a merge: it covers these.

## What has been tried

Verified: the whole upstream-sync loop with a real change on 2026-10-05 (daily job, clean merge, checks, Claude review with verdict, pull request, "merge it" in Telegram, merge, tag `build-3`, CI build of the zip and deb, release, build-finished message); Norwegian dictation (NB-Whisper) with the CI-built 0.9.8 portable zip on the work laptop (RTX A3000, 2026-10-05), which also covers the `transcribe` 0.3.0 bump; the unit tests (368), the prompt bench (41 of 41 on two real endpoints), local-model benchmarks, meeting decoding on a generated
file, the CI builds (they run), Talon files loading without errors, the Ansible `--check` runs that were done. **Not tried on real data or by
voice:** meeting transcription of a real recording (silence cutting, OBS grouping, local speed), speaker identification and speaker names
against a real endpoint, `--set-llm`, most spoken Talon commands, scratch and redo in VS Code and terminals, the paste guard in daily use, the
overlay fix, the Linux builds and Linux Talon files, the CI-built portable zip and `.deb` on a machine, the Ansible roles on a fresh machine.

## Known limits

- New UI text exists in English only; `bun run check:translations` therefore reports the new keys as missing in the other 24
  languages (expected; the app falls back to English).
- The vocabulary and snippet tags and the format commands depend on the model following the prompt; the bench measures this.
- Wayland: global shortcuts must be bound to the flags in the desktop settings; typing into other apps needs `wtype`,
  `dotool` or `ydotool` (X11: `xdotool`).
- The AppImage format has not been built.
