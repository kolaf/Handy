# What's new in this build

This build (`0.9.7-hotkeys.1`) adds features on top of Handy. The full reference is `FORK.md` in the repository.

## Switching quickly

- **Swap language** (`Ctrl+Alt+L`): swaps the language with the _Alternate Language_ you set on the General page.
- **Next prompt** (`Ctrl+Alt+P`): steps through your post-processing prompts.
- **Learn from correction** (`Ctrl+Alt+K`): select text you corrected after a dictation and press it. Handy learns new vocabulary and recurring mishearings (see Advanced → Learned Corrections).
- **Re-run with next prompt** (`Ctrl+Alt+R`): redo your last dictation with the next prompt. Select the text you pasted before to replace it.
- **Paste last dictation** (`Ctrl+Alt+V`): paste your most recent dictation again.
- A short notice and a caption under the recording controls show the current language and prompt.

## Teaching Handy your words

- **By dictation** (post-processing shortcut): say a word and then spell it ("dyst, delta yankee sierra tango"), or say "add to vocabulary" followed by the word. It is added to Custom Words.
- **Snippets** (Advanced): store text such as a signature or link, then say "insert my signature". The text is inserted locally; only the snippet _names_ reach the language model.

## Writing prompts

- Variables: `${output}` the transcript, `${vocabulary}` your Custom Words, `${snippets}` the snippet names, `${clipboard}` the clipboard text (read only if the prompt uses it), `${examples}` the prompt's examples.
- The **Examples** box is for fixed structures: put the structure with `[placeholders]` in the prompt, then add one or two `Dictation: ... / Result: ...` pairs separated by `---`.
- The _Document template_ and _Reply_ prompts are working samples. _Reply_ uses the clipboard as the message you are answering.
- Say a format at the end of a dictation ("...format as email", "...som punktliste") with the _Super_ prompt to choose the style.

## When the model is not available

- If the language model cannot be reached, Handy cleans the text locally (spoken punctuation and capital letters) and says "Offline: basic cleanup only".
- **Skip the model for short dictations** (Post Process page) uses the same local cleanup for dictations under a number of words you choose.

## Command line

`handy --swap-language`, `--next-prompt`, `--rerun`, `--paste-last`, `--learn`, `--sync-lists FILE` (merge words, snippets and learned corrections with a JSON file, e.g. one kept in git), `--set-language no`, `--set-prompt email`. They work with a running Handy and can be combined with `--toggle-post-process`, for example from a window-manager key binding or Talon.
