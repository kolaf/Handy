# What's new in this build

This build (`0.9.7-hotkeys.1`) adds features on top of Handy. The full reference is `FORK.md` in the repository. The shortcuts below are the defaults; change them on the General and Post-Processing pages.

## Switching quickly

- **Swap language** (`Ctrl+Alt+L`): swaps the language with the _Alternate Language_ you set on the General page.
- **Prompt picker** (`Ctrl+Alt+P`): a numbered list of your prompts in a small window. Press a number key or click a row to choose, `Esc` (or `Ctrl+Alt+P` again) to close. The first nine prompts are listed; the one in use is highlighted. The window never takes focus, so your selection stays where it is.
- **Prompt per app** (Post-Processing page): choose the prompt by the app you dictate into, for example Slack → _Informal message_. Rules are "program (+ optional part of the window title) → prompt"; the first matching rule wins, otherwise the selected prompt is used. Windows, and Linux with X11 (Wayland does not let apps see the active window). On Linux the program name is the process name, for example `slack` or `firefox`. The app is the one you were in when you **started** speaking, not where you are when the text is ready. The Handy log shows the program name it saw.
- **Keeps dictation out of the wrong window** (Advanced, on by default; Windows and Linux/X11): if you switch to another window or app while a dictation is being prepared, it is not pasted there. It is put on the clipboard and a notice says so.
- **Paste last dictation** (`Ctrl+Alt+V`): paste your most recent dictation again.
- A short notice and a caption under the recording controls show the current language and prompt.

## Working on text you already have

- **Reformat selection** (`Ctrl+Alt+F`): run the selected text through the selected prompt and replace it with the result.
- **Transform** (`handy --transform ID`, or Talon: "make that formal"): run the selection, or with nothing selected the last dictation, through a one-purpose prompt and replace it. Built in: formal, informal, shorter, fuller, clearer, fix spelling and grammar, translate to Norwegian or English, bullet list, summary (the prompts whose id starts with `t_`; they are not in the picker). For the last dictation Handy first checks that the text before the cursor really is that dictation, and does nothing otherwise.
- **Undo and redo by voice** (Talon): "scratch dictation" deletes the last dictation and "redo as email" (message, note, meeting, formal, ...) processes the same recording again with that prompt and replaces the text. Handy first checks that the text before the cursor really is the last dictation (`--scratch-last`, `--redo-with ID`).
- **Re-run with next prompt** (`Ctrl+Alt+R`): redo your last dictation with the next prompt in the list. Select the text you pasted before to replace it.
- **Edit by instruction**: Talon's "edit this" copies the selected text, then you speak the change you want ("shorter and friendlier, mention Thursday"), stop with your Handy key, and the result replaces the selection (it uses `handy --use-prompt-once edit --toggle-post-process`). The fixed "make that ..." commands are faster for common changes.
- **Reply with context**: `handy --use-prompt-once reply --toggle-post-process` (Talon: "reply to this") makes the next dictation use the _Reply_ prompt, with the clipboard (the message you are answering) as context; the prompt is then forgotten.

## Teaching Handy your words

- **By dictation** (post-processing shortcut): say a word and then spell it ("dyst, delta yankee sierra tango"), or say "add to vocabulary" followed by the word. It is added to Custom Words.
- **Learn from correction** (`Ctrl+Alt+K`): fix a dictation by hand, select the fixed text and press the shortcut. Handy compares it with what was heard and learns new words and recurring mishearings.
- **Learned Corrections** (Advanced): the list of mishearings it learned. A rule is either _Always_ (replaced automatically; only for heard text that is not a real word, like "Superwisper") or _Hint_ (only shown to the model, which applies it when the sentence fits; used for real words like "carry" for "Kari"). Click the button to switch a rule, or remove it.
- **Snippets** (Advanced): store text such as a signature or link, then say "insert my signature". The text is inserted locally; only the snippet _names_ reach the language model.
- **Learn a project's words**: `handy --learn-repo FOLDER` (Talon: "learn this repo" in the terminal) scans a folder and asks the model for the names (people, places, products, codes) and the domain vocabulary of the project, and adds them to Custom Words; `handy --import-words FILE` adds the words in a text file.
- **Sync between computers**: `handy --sync-lists FILE` merges Custom Words, Snippets and Learned Corrections with a JSON file you keep in git. It only adds entries, so deletions are not carried over.

## The Activity page

Next to History. Every on-screen notice (learned, nothing learned, lists synced, reformat problems ...) is kept here with the details behind it: which dictation was compared, what the model said, what was added and what was left out and why. Click an entry to see it. Only your most recent dictation is compared when you learn from a correction.

## Writing prompts

- Variables: `${output}` the transcript, `${vocabulary}` your Custom Words, `${corrections}` the learned mishearings ("X is Y" for _Always_ rules, "X may be Y" for hints), `${snippets}` the snippet names, `${clipboard}` the clipboard text (read only if the prompt uses it), `${examples}` the prompt's examples, `${app}` and `${title}` the program and window title where you started speaking (Windows), `${language}` the dictation language, `${date}`, `${time}` and `${weekday}`. Window titles are untrusted text, so treat them as data in your prompt.
- The **Examples** box is for fixed structures: put the structure with `[placeholders]` in the prompt, then add one or two `Dictation: ... / Result: ...` pairs separated by `---`.
- The _Document template_ and _Reply_ prompts are working samples. _Reply_ uses the clipboard as the message you are answering.
- Say a format at the end of a dictation ("...format as email", "...som punktliste") with the _Super_ prompt to choose the style.
- A prompt whose id starts with `t_` is a transform: it works on written text and is left out of the picker and of "re-run with next prompt".

## When the model is not available

- If the language model cannot be reached, Handy cleans the text locally (spoken punctuation and capital letters) and says "Offline: basic cleanup only". Hint rules do nothing then; _Always_ rules still apply.
- **Skip the model for short dictations** (Post-Processing page) uses the same local cleanup for dictations under a number of words you choose.

## Together with Talon

- While Handy records it writes `%USERPROFILE%\.cache\hv\handy-state.txt`; the Talon setup in `kolaf/handy` uses it to switch Talon's speech off during a dictation and on again afterwards, so your dictation is not taken for voice commands.
- Talon commands in that setup: "make that formal|informal|shorter|fuller|clearer", "fix that up", "translate that to norwegian|english", "bullet that", "summarize that", "reply to this", and "learn this repo" (terminal).

## Command line

`handy --swap-language`, `--prompt-picker`, `--reformat`, `--transform ID`, `--redo-with ID`, `--scratch-last`, `--use-prompt-once ID`, `--rerun`, `--paste-last`, `--learn`, `--learn-repo FOLDER`, `--import-words FILE`, `--sync-lists FILE`, `--set-language no`, `--set-prompt email`. They work with a running Handy and can be combined with `--toggle-post-process`, for example from a window-manager key binding, a Logitech button or Talon.
