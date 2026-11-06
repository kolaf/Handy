Read @AGENTS.md

Branching strategy for coding agents:
- This fork's active development branch is `dev/hotkeys-build`.
- Make Handy code changes either directly on `dev/hotkeys-build` or in a worktree branched from `dev/hotkeys-build`.
- Do not base new fixes on `main`, `origin/main`, or `upstream-sync-*` branches unless the human explicitly asks for upstream-integration work.
- Treat `main` as the upstream-aligned integration branch and `dev/hotkeys-build` as the fork feature branch.
- Before starting non-trivial work, verify your current branch/worktree base with `git branch --show-current` and `git log -1 --oneline`.
- If a fix was developed on the wrong branch, port only the proven minimal changes onto `dev/hotkeys-build` rather than continuing work on the wrong base.
