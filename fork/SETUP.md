# Setting up a Windows + WSL machine (for example the work computer)

The whole voice setup in the order it should be installed. The parts are independent, so you can stop after any step.
Nothing here contains secrets; keys come from 1Password. Items marked **untested** have not been run on a fresh machine.

| Part | Where it runs | Needs |
|---|---|---|
| A. Terminal tools, Hermes, `hv`, the shell hook for Talon | WSL (Ubuntu) | the dotfiles repository (Ansible) |
| B. Handy (dictation, picker, learn, meetings, ...) | Windows | a `handy.exe` that the machine's policy lets you run |
| C. Talon (voice commands) | Windows | Talon, the `kolaf/community` fork, Rango |

## 0. Check the policy first (work computers)

Handy built from this fork is **unsigned**. Defender SmartScreen, AppLocker or similar may refuse to run it, and the options for
getting around that are not mine to take on a managed machine. Ask IT whether a folder can be approved, or pick the fallback in
B below. A and C do not depend on this.

## A. WSL: terminal tools, Hermes, `hv`

If the dotfiles are already installed (you ran `ansible-playbook -i inventory playbook.yml` before), just update and run again:

```
cd ~/dotfiles
git pull --rebase
ansible-playbook -i inventory playbook.yml -K
```

New in the playbook since an old install: the `hermes` role (installs Hermes if missing, clones the private
`hermes-skills` repository, creates the Hindsight config, links `hv`), the Talon terminal hook (`~/.config/talon-terminal.bash`,
sourced from `.bashrc`), and `yazi`. Handy and Talon are not installed on WSL by the playbook (`-e install_handy=true` is for a
native Linux desktop). Then do the manual steps in `~/dotfiles/MANUAL-STEPS.md`. For this machine in particular:

1. **GitHub SSH access** must work in WSL (`ssh -T git@github.com`): the playbook clones private repositories.
2. **1Password `op` in WSL.** An earlier change removed the Linux `op` in favour of the Windows one. The Hindsight step calls
   `op read`. Either leave `hindsight_api_key_op_ref` empty in `group_vars/all.yml` and put the key into
   `~/.hermes/hindsight/config.json` by hand, or make `op` available in WSL, for example
   `ln -s "$(command -v op.exe)" ~/.local/bin/op` (**untested**).
3. **Hindsight address** is `https://hermes-hindsight-api.kolaf.net` (the API). The address without `-api` is only the web UI.
4. Run `hermes` once to sign in. The playbook links `hermes-skills-sync` into `~/.local/bin` and runs it when Hermes is
   installed; run the playbook again after signing in, or run it by hand: `hermes-skills-sync --dry-run`, then `hermes-skills-sync`.
5. Open a **new** terminal (or `source ~/.bashrc`) so the shell hook and `z`/`zi`/`y` exist.

Check it:

```
type z y                          # z, zi and y are shell functions
echo "$PROMPT_COMMAND" | grep -c __tt_update   # 1 or more
hv --dry "list the files"         # prints the prompt, calls nothing
hermes-skills-sync --dry-run
```

## B. Windows: Handy

Handy has no download page for this fork (no releases): the portable zip is built from the repository.

1. **Get a build.** Either copy the newest `Handy-<version>-<commit>-portable.zip` from the machine that builds (it is in
   `C:\dev`), or build it on this machine: prerequisites and commands are in `FORK.md` ("Building and installing";
   Visual Studio C++ workload, Windows SDK, Rust, Bun, CMake, Vulkan SDK), then `fork\scripts\build-portable.ps1`.
2. **Unpack to a folder that is allowed to run programs and is not deleted by updates**, for example `D:\Handy`
   (or the folder IT approves). Keep the `Data` folder: it holds settings, models and history, and
   `fork\scripts\deploy-portable.ps1 -Target <folder>` updates the program without touching it.
3. **Speech model.** The Norwegian model is a single file:
   `https://huggingface.co/NbAiLab/nb-whisper-medium/resolve/main/ggml-model-q5_0.bin` (540 MB), saved as
   `Data\models\nb_ggml-model-q5_0.bin`. For English dictation also download Parakeet on Handy's Models page.
4. **Start Handy once**, then set (all on its pages): post-processing on, provider "custom" with the LiteLLM address
   (`https://hermes-litellm.kolaf.net/`), model `gpt-5.4`, and the key from 1Password; the speech model; language and
   alternate language.
5. **Install the prompts** with Handy closed. From WSL, with the repository cloned (`git clone git@github.com:kolaf/Handy.git ~/dev/Handy`,
   branch `dev/hotkeys-build`):
   `python3 ~/dev/Handy/fork/scripts/install-prompts.py /mnt/d/Handy/Data/settings_store.json` (adjust the path).
   It replaces the built-in prompts by id, keeps your own, and writes a backup next to the settings file.
6. **Words, snippets and learned corrections** come from the dotfiles repository with Handy running:
   `handy.exe --sync-lists \\wsl.localhost\<distro>\home\<user>\dotfiles\handy-lists.json`.
7. Optional links (Settings): "Model per language" (English → Parakeet, Norwegian → NB-Whisper), "Prompt per app", the recorder
   folder for meetings (where OBS Studio saves).

If the policy blocks the fork's `handy.exe`: the official, signed Handy can still be used with these prompts and a profile switcher
(`fork\scripts\handy-profile.ps1`), but the fork-only features are missing: the numbered pickers, learn from correction,
transforms and "scratch", the per-app and per-language rules, the Activity page, and meeting minutes. The other
fallback is Linux (`FORK.md`, Linux build).

## C. Windows: Talon

1. Install Talon (talonvoice.com) and set the microphone and speech engine in its menu.
2. In `%APPDATA%\talon\user`: `git clone git@github.com:kolaf/community.git` (this contains the `kolaf/` folder with all the
   personal commands) and then, inside it, `git remote add upstream https://github.com/talonhub/community.git`.
   Next to it: `git clone https://github.com/david-tejada/rango-talon.git` and
   `git clone https://github.com/cursorless-dev/cursorless-talon.git`. Install the Rango extension in the browser (it must match
   the Talon side's version).
3. Create `%APPDATA%\talon\user\settings.talon` (machine-only): `settings():` then `speech.timeout = 0.4`.
4. If Handy is not in `D:\Handy`, set its path in a `.talon` file of your own:
   `settings():` `user.kolaf_handy_path = "C:/path/to/handy.exe"`.
5. Talon starts asleep; `Ctrl+PageUp` toggles speech. The list of terminal commands is in
   `kolaf/terminal/README.md` ("terminal help" opens it).
6. The terminal commands need part A's shell hook, which writes `%USERPROFILE%\.cache\hv\terminal-state.txt`.

## Optional: a local language model for post-processing

Nothing here is required. Handy's post-processing can use any OpenAI-compatible server instead of LiteLLM, and `llama-server`
(llama.cpp) is the simplest one: no other service, it reads model files directly. It needs a **GGUF** file (the old GGML `.bin`
format is not read, and Whisper's `.bin` files are speech models, a different thing). Benchmarked on the RTX 3080 home machine with
`fork/prompts/bench.py` (41 cases; gpt-5.4 passes all): Qwen2.5-7B 34, Qwen3-4B 33, Gemma-3-12B 31, Gemma-3-4B 27. Local models are
weakest at Norwegian spoken punctuation and format commands and at ignoring instructions inside the dictated text, so keep
gpt-5.4 for Norwegian and for the "make that ..." transforms, and use a local model for quick English cleanup or as an offline
fallback. Not tried on the work laptop.

1. Download the **Vulkan** build `llama-<tag>-bin-win-vulkan-x64.zip` (about 31 MB, any GPU with a current driver, no CUDA) from
   `https://github.com/ggml-org/llama.cpp/releases` and unpack it, for example to `C:\llm\bin`. If the machine's policy blocks an
   unsigned `llama-server.exe`, stop here (same rule as for Handy).
2. Download one model file (copy it over from another computer if downloads are restricted):
   - 6 GB GPU (RTX A3000 Laptop): `Qwen3-4B-Instruct-2507-Q4_K_M.gguf` (2.5 GB, from `unsloth/Qwen3-4B-Instruct-2507-GGUF` on
     Hugging Face) fits completely. `Qwen2.5-7B-Instruct-Q4_K_M.gguf` (4.7 GB, `bartowski/Qwen2.5-7B-Instruct-GGUF`) scores
     slightly better but is tight next to Handy's speech model; if it is slow, the model did not fit on the GPU, so use the 4B.
   - 10 GB or more: the 7B. Models of 12B and up were slower and not better.
3. Start it (leave the window open, or make a shortcut):
   `llama-server.exe -m C:\llm\models\Qwen3-4B-Instruct-2507-Q4_K_M.gguf -ngl 99 -c 4096 --host 127.0.0.1 --port 8081`
4. In Handy (Post-Processing): choose the provider "Local (llama-server)" (address `http://127.0.0.1:8081/v1`, change it there if
   you use another port), any API key (for example `x`), any model name. Your LiteLLM settings stay under "Custom". Switch with
   `handy --set-llm local` and `handy --set-llm cloud` (Talon: "language model local" / "language model cloud"). Speaker
   identification in meetings always uses the cloud (custom) endpoint, also while "local" is selected.
5. Check quality yourself before trusting it:
   `BENCH_BASE_URL=http://127.0.0.1:8081/v1 BENCH_API_KEY=x BENCH_MODEL=local python3 fork/prompts/bench.py` (standard library
   only; Talon's `python.exe` also runs it on Windows).

## Is it working? (checks, in a few minutes)

- Handy: the picker opens with `Ctrl+Alt+P`; a dictation with post-processing pastes; the Activity page lists notices.
- Talon: `terminal help` opens the command list; in a WSL terminal `folders` shows the sub-folders; `model picker` opens Handy's list.
- Together: start a dictation by hotkey; Talon should switch its speech off while Handy records and on again afterwards.
- Hermes: `hv ask "what is in this folder"` answers in a few seconds.

## What is not automated

Handy's endpoint/key/language settings (typed once), Hermes sign-in, the 1Password references, the speech model download, the
Rango and Cursorless browser/editor extensions, `places.md` for `hv`, and anything the machine's policy has to approve.
