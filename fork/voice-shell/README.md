# Voice shell (`hv`)

Natural-language file and folder work by voice, using Hermes Agent as the shell. One short Hermes session per
spoken request, so the context stays small; the agent's own memory carries your habits between requests.

```
hv "copy this file to the reports folder"      PLAN: inspects read-only, prints a numbered plan, ends "Go ahead?"
hv go                                          EXECUTE exactly that plan (same session)
hv again "no, the archive one"                 answer a question or correct the plan (plans again)
hv ask "what is in the folder I used yesterday"   read-only question, fresh session
hv --dry "..."                                 show the prompt and command, call nothing
```

Nothing changes on disk until you say go. The wrapper adds, to every request: the working directory, the
selected files (see below), recent folders (`zoxide`, if installed) and your own places file
(`~/.config/hv/places.md`, plain-language aliases such as `reports = ~/work/reports`), plus the terminal-safety
rules in `preamble.md` (quote paths, never overwrite, never delete, move to trash instead, no sudo, ask when
ambiguous, show exact full paths).

## Install

- `ln -sf <repo>/fork/voice-shell/hv ~/.local/bin/hv` (done on this machine). Needs Hermes (`hermes`) configured.
- Optional: `zoxide` for recent folders (`eval "$(zoxide init bash)"` in `.bashrc`).
- Talon: the commands are in the community fork, `kolaf/community`, folder `kolaf/hv/` (loaded automatically; no copying).
  They are installed on the home machine and load without errors; the spoken behaviour is untried.

## Talon commands

- In a terminal: `hermes <what you want>`, `hermes go` (also yes / go ahead / do it), `hermes again <text>`,
  `hermes ask <text>`, `hermes cancel`. They type the `hv` command into the terminal you are in, so the output
  and the current folder are already right. `hermes` was chosen because "shell" is used by other community files.
- In a file manager: `grab files` copies the selection and remembers the paths. Then switch to the terminal and say
  `hermes copy these to the reports folder`. The selection counts for 3 minutes (`HV_SELECTION_TTL`).
- Talon's recognizer is English-focused. For Norwegian requests, dictate with Handy into the terminal instead and
  press Enter after typing `hv ` first.

## Speed

A request costs about 5-10 s, almost all of it model time (two model calls of ~10k tokens each, and the wait grows with the
number of tokens the model writes). What `hv` does about it:

- **Private voice home** (`~/.config/hv/hermes-home`): a copy of your Hermes config with `reasoning_effort: low` and a 5 s
  approval timeout, refreshed whenever your real `config.yaml` changes. A command that needs approval cannot be approved by
  voice, so waiting the default 60 s only delayed the refusal (this was the cause of 60-90 s runs). Sign-in, memories, skills
  and the Hindsight setup are shared with the normal home through symlinks; your own config is never changed.
  `HV_FAST=0` turns it off; `HV_REASONING` and `HV_APPROVAL_TIMEOUT` tune it.
- **The folder listing is in the context** ("Files here"), so "list the files" needs no tool call (one model call, ~6 s).
- Toolsets are `terminal,file,memory` (no `skills`: that leaves out the skills index and tools, ~10 % of the prompt).
- Read-only requests (list, show, find, count) are answered directly; only requests that change something get a plan
  ending in "Go ahead?".
- `hermes -z` is not used: it auto-approves every command, which would remove the plan-first safety.

## Settings (environment variables)

`HV_TOOLSETS` (default `terminal,file,memory`; `clarify` is deliberately off, see below), `HV_MAX_TURNS`
(12), `HV_SOURCE` (`voice`, a session tag), `HV_SELECTION_FILE`, `HV_SELECTION_TTL`, `HV_HERMES`, `HV_PREAMBLE`,
`HV_STATE_DIR`, `HV_CONFIG_DIR`, `HV_EXTRA_ARGS` (extra flags for Hermes, e.g. `--ignore-rules` for tests).
State: the last session id, and a tab-separated journal of every request, in `~/.local/state/hv/`.

## What was verified (2 October 2026, Hermes 0.18.2, gpt-5.4 via the hosted gateway)

- `test-hv.sh`: 31 checks against a stub Hermes: prompt assembly, session resume, `go` versus `go to ...`, stale
  selection, quoting of `"`, `$()`, backticks and newlines, error exit codes, dry run.
- Real runs in a scratch folder: copy plan (nothing changed) then `go` (copied, source intact); an ambiguous
  request (numbered candidates, no action) answered with `again` then `go`; a delete request (planned as a move to
  trash, not executed); a move plan with exact paths.
- Timing: a trivial call about 7 s; a plan 13 to 25 s; `go` or a follow-up 6 to 10 s. Each call sends about
  10,000 input tokens (Hermes' own tool descriptions, mostly cached after the first call).
- WSL finds a selection written by Windows and converts `C:\...` paths; the PowerShell clipboard command returns
  the exact paths of copied files.

## What was not verified

- The Talon files' spoken behaviour (they load without errors). `hv_grab_selection` assumes `edit.copy`, `sleep` and `app.notify`.
- Executing a trash move, `--checkpoints` with `/rollback`, Hermes' own approval prompts (never triggered),
  long sessions, Norwegian requests, and whether the agent saves habits to memory (tests ran with `--ignore-rules`).

## Design notes and lessons

- `clarify` (Hermes' ask-a-question tool) waits for an answer nobody can type in one-shot mode and times out
  after 120 s, so it is off; the agent lists numbered candidates and stops instead.
- The first version let the agent search the filesystem for a folder that sat next to the working directory
  (62 s). The preamble now says: look nearby first, then places and recent folders, and only then search with a
  depth and time limit.
- The first preamble told the agent to shorten paths with `~`, and it printed `/tmp/x` as `~/tmp/x` in a plan.
  Plans must show exact full paths.
