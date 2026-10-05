#!/usr/bin/env bash
# Brings upstream (cjpais/Handy main) into the fork, unattended, and leaves a pull request for you to review.
#
#   1. fetch upstream; stop quietly if there is nothing new (or a sync for this upstream commit already exists)
#   2. in a separate git worktree (your working copy is never touched) merge upstream into a branch made from the fork's branch
#   3. run the checks (frontend build, Rust unit tests, lint, type check)
#   4. conflicts or failing checks: ask Claude Code (headless, `claude -p`) to finish the merge following FORK.md; it may edit and
#      commit in the worktree, but it may not push or use gh. Then THIS SCRIPT runs the checks again; Claude's word is not trusted
#   5. checks pass: push the branch and open a pull request into the fork's branch (the body says if Claude resolved conflicts)
#      checks fail: push the branch anyway and open an issue with the end of the log
#
# Run by hand: fork/scripts/sync-upstream.sh      Scheduled: see "Weekly upstream sync" in FORK.md (Windows Task Scheduler).
# Watch a run:   tail -f ~/.cache/hv/upstream-sync.log           the steps of the script
#                fork/scripts/sync-upstream.sh --watch            what Claude is doing (tool calls and text), live
# Afterwards:    claude --resume <session id from the log>        opens Claude's session (in the worktree directory)
# Output: everything goes to the log; on a terminal (or VERBOSE=1) it is shown as well. When it is not run from a terminal (cron, Hermes) the only
# thing printed is one result line (pull request, issue or failure), and nothing at all when there was nothing to do.
# Settings through the environment (defaults in the block below). DRY=1 prints what it would push or open and does not do it.
set -uo pipefail

if [ "${1:-}" = "--watch" ]; then
  F=${LOG:-$HOME/.cache/hv/upstream-sync.log}.claude.jsonl
  [ -f "$F" ] || { echo "no Claude run recorded yet ($F)"; exit 1; }
  tail -n +1 -f "$F" | python3 -u -c '
import sys, json
for line in sys.stdin:
    try:
        e = json.loads(line)
    except ValueError:
        print(line.rstrip()); continue
    t = e.get("type")
    if t == "assistant":
        for b in e.get("message", {}).get("content", []):
            if b.get("type") == "text" and b.get("text", "").strip():
                print("CLAUDE:", b["text"].strip()[:600])
            elif b.get("type") == "tool_use":
                arg = b.get("input", {})
                arg = arg.get("command") or arg.get("file_path") or arg.get("pattern") or json.dumps(arg)[:120]
                print("  tool %s: %s" % (b.get("name"), str(arg)[:200]))
    elif t == "result":
        print("RESULT:", str(e.get("result", ""))[:1200])
'
  exit 0
fi

REPO=${REPO:-$HOME/dev/Handy}
WORK=${WORK:-$REPO-sync}
UPSTREAM=${UPSTREAM:-origin}            # remote that points at cjpais/Handy
UP_BRANCH=${UP_BRANCH:-main}
BASE=${BASE:-dev/hotkeys-build}         # the fork's product branch
GH_REPO=${GH_REPO:-kolaf/Handy}
PUSH_URL=${PUSH_URL:-https://github.com/$GH_REPO.git}
DRY=${DRY:-0}
CLAUDE_CMD=${CLAUDE_CMD:-claude}
CLAUDE_BUDGET_USD=${CLAUDE_BUDGET_USD:-10}
LOG=${LOG:-$HOME/.cache/hv/upstream-sync.log}
DEFAULT_CHECKS='bun install && bun run build && (cd src-tauri && cargo test --lib) && bun run lint && npx tsc --noEmit'
CHECKS=${SYNC_CHECKS:-$DEFAULT_CHECKS}

mkdir -p "$(dirname "$LOG")"
exec 3>&1
QUIET=1
if [ -t 1 ] || [ "${VERBOSE:-0}" = 1 ]; then QUIET=0; exec > >(tee -a "$LOG") 2>&1; else exec >>"$LOG" 2>&1; fi
say() { printf '[%s] %s\n' "$(date '+%F %T')" "$*"; }
out() { [ "$QUIET" = 1 ] && printf '%s\n' "$*" >&3; return 0; }       # the one line a scheduler delivers
fail() { say "$*"; out "Upstream sync failed: $*"; exit 1; }

# Tools a scheduled (non-login) shell may not have on PATH.
[ -s "$HOME/.nvm/nvm.sh" ] && . "$HOME/.nvm/nvm.sh" >/dev/null 2>&1
[ -s "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
export PATH="$HOME/.bun/bin:$HOME/.local/bin:$PATH"

# One run at a time.
exec 9>"$(dirname "$LOG")/upstream-sync.lock"
flock -n 9 || { say "another sync is running; stopping"; exit 0; }

gitpush() { git -c credential.helper= -c credential.helper='!gh auth git-credential' push "$PUSH_URL" "$@"; }
run() { if [ "$DRY" = 1 ]; then say "DRY: $*"; else "$@"; fi; }

cd "$REPO" || fail "no repository at $REPO"
git fetch -q "$UPSTREAM" "$UP_BRANCH" || fail "cannot fetch $UPSTREAM"
UP_REF="$UPSTREAM/$UP_BRANCH"
# Compare with the fork's branch as it is on GitHub (not with a local branch that may be stale or hold unpushed work).
BASE_REF=refs/sync/base
git fetch -q "$PUSH_URL" "+$BASE:$BASE_REF" || fail "cannot fetch $BASE from $PUSH_URL"
COUNT=$(git rev-list --count "$BASE_REF..$UP_REF")
if [ "$COUNT" = 0 ]; then say "up to date with $UP_REF"; exit 0; fi
UP_SHA=$(git rev-parse --short "$UP_REF")
BRANCH="upstream-sync-$UP_SHA"
say "$COUNT new upstream commit(s) up to $UP_SHA"

if git ls-remote --exit-code --heads "$PUSH_URL" "$BRANCH" >/dev/null 2>&1; then
  say "branch $BRANCH already exists on GitHub; a sync for this upstream commit was already made"; exit 0
fi

# Without the native build libraries the checks can only fail, and Claude would be asked to "fix" a missing system package.
if [ -z "${SYNC_CHECKS:-}" ] && [ "$(uname -s)" = Linux ] && ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  fail "the Linux build libraries are missing on this machine (see the Linux section in FORK.md), so the checks cannot run; $COUNT new upstream commit(s) are waiting"
fi

COMMITS=$(git log --oneline "$BASE_REF..$UP_REF" | head -40)
git worktree remove --force "$WORK" 2>/dev/null; rm -rf "$WORK"; git worktree prune
git branch -D "$BRANCH" >/dev/null 2>&1
git worktree add -q -b "$BRANCH" "$WORK" "$BASE_REF" || fail "cannot create the worktree"
cd "$WORK" || exit 1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO/src-tauri/target}"   # reuse the compiled dependencies

CONFLICTS=""
if git merge --no-edit "$UP_REF" >/tmp/sync-merge.out 2>&1; then
  say "merge is clean"
else
  CONFLICTS=$(git diff --name-only --diff-filter=U)
  say "merge conflicts in: $(echo $CONFLICTS)"
fi

check() { bash -c "$CHECKS" >/tmp/sync-checks.out 2>&1; }

ASSISTED=0
NEED_CLAUDE=0
if [ -n "$CONFLICTS" ]; then NEED_CLAUDE=1; elif ! check; then NEED_CLAUDE=1; say "merged cleanly but the checks fail"; else say "checks pass"; fi

CLAUDE_SUMMARY=""
if [ "$NEED_CLAUDE" = 1 ]; then
  ASSISTED=1
  PROMPT=$(cat <<EOF
You are in a git worktree of the fork kolaf/Handy, branch $BRANCH (made from $BASE). The task is to bring upstream ($UP_REF, $COUNT new commits) into the fork.
State: $( [ -n "$CONFLICTS" ] && echo "a 'git merge $UP_REF' is in progress with conflicts in: $(echo $CONFLICTS)" || echo "the merge is committed and clean, but the checks fail (the log is in /tmp/sync-checks.out)" ).

Read FORK.md (section "Merging upstream" and "Known limits") and AGENTS.md first. Rules:
- Keep the fork's features. Where upstream deleted or replaced code the fork also touched, adopt upstream's new design and re-apply only the fork's own part.
- Version label: the fork's version is '<upstream version>-hotkeys.1' in package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json and the 'handy' entry of src-tauri/Cargo.lock; update it when upstream's version changed, and update the version text in FORK.md, src/content/fork-guide.md and fork/scripts/build-portable.ps1.
- Fix compile, type and test errors that the merge causes (removed or renamed upstream APIs used by fork code), including src/bindings.ts, which is edited by hand in this fork.
- Do not weaken or delete tests to make them pass. Do not touch other branches. Never run git push, gh, or anything that talks to GitHub.
- Run the checks yourself ($CHECKS) until they pass, then finish the merge with a commit (git commit, or git add + git commit for a merge in progress).
- Finish with a short summary on stdout: per conflicted file what you decided and why, what you changed to make the checks pass, and anything you are unsure of.
EOF
)
  say "asking Claude to finish the merge (budget \$$CLAUDE_BUDGET_USD)"
  SESSION_ID=$(cat /proc/sys/kernel/random/uuid)
  STREAM="$LOG.claude.jsonl"
  say "Claude session id: $SESSION_ID (claude --resume $SESSION_ID); live view: fork/scripts/sync-upstream.sh --watch"
  : > "$STREAM"
  "$CLAUDE_CMD" -p "$PROMPT" --session-id "$SESSION_ID" --output-format stream-json --verbose \
    --permission-mode acceptEdits --max-budget-usd "$CLAUDE_BUDGET_USD" \
    --allowedTools "Read" "Edit" "Write" "Grep" "Glob" "Bash(git status:*)" "Bash(git diff:*)" "Bash(git log:*)" "Bash(git show:*)" \
      "Bash(git add:*)" "Bash(git commit:*)" "Bash(git checkout:*)" "Bash(git merge --continue:*)" "Bash(cargo:*)" "Bash(bun:*)" "Bash(npx:*)" \
      "Bash(python3:*)" "Bash(ls:*)" "Bash(cat:*)" "Bash(grep:*)" "Bash(sed:*)" "Bash(cd:*)" "Bash(bash:*)" "Bash(true:*)" \
    --disallowedTools "Bash(git push:*)" "Bash(gh:*)" "Bash(curl:*)" "Bash(wget:*)" "Bash(rm -rf:*)" >>"$STREAM" 2>&1 || say "claude exited with an error"
  CLAUDE_SUMMARY=$(python3 - "$STREAM" <<'PY'
import sys, json
raw = open(sys.argv[1], errors="replace").read()
result = None
for line in raw.splitlines():
    try:
        e = json.loads(line)
    except ValueError:
        continue
    if isinstance(e, dict) and e.get("type") == "result":
        result = e.get("result")
print(result if result else raw[-3000:])
PY
)
  say "Claude finished"
  printf '%s\n' "$CLAUDE_SUMMARY" | tail -40
fi

# The verdict is ours, not Claude's: the merge must be complete and the checks must pass now.
STATE=ok
if [ -n "$(git diff --name-only --diff-filter=U)" ] || git rev-parse -q --verify MERGE_HEAD >/dev/null; then STATE="the merge is not finished"
elif [ -n "$(git status --porcelain)" ]; then STATE="uncommitted changes remain"
elif ! git merge-base --is-ancestor "$UP_REF" HEAD; then STATE="upstream is not in the branch"
elif [ "$NEED_CLAUDE" = 1 ] && ! check; then STATE="the checks fail"
fi

SUMMARY_BLOCK=""
if [ "$ASSISTED" = 1 ]; then
  SUMMARY_BLOCK=$(printf '\n\n**Conflicts or failing checks were resolved by Claude Code (unattended). Review the resolutions before merging.**\nConflicted files: %s\n\nClaude'"'"'s summary:\n\n%s\n' "${CONFLICTS:-none (the checks failed after a clean merge)}" "$(printf '%s\n' "$CLAUDE_SUMMARY" | tail -40)")
fi

if [ "$STATE" = ok ]; then
  say "ready: pushing $BRANCH and opening a pull request"
  run gitpush "$BRANCH" || fail "could not push $BRANCH"
  BODY=$(printf 'Upstream %s: %s new commit(s) merged into %s.\n\nChecks (frontend build, Rust unit tests, lint, type check) pass.%s\n\nNew upstream commits:\n\n```\n%s\n```\n\nNot run: the app itself. Merge, then tag a build (`git tag build-N && git push <repo> build-N`) and try dictation.\n' "$UP_SHA" "$COUNT" "$BASE" "$SUMMARY_BLOCK" "$COMMITS")
  PR_URL=$(run gh pr create --repo "$GH_REPO" --base "$BASE" --head "$BRANCH" --title "Merge upstream $UP_BRANCH ($UP_SHA, $COUNT commits)" --body "$BODY" 2>>"$LOG" | tail -1)
  [ "$DRY" = 1 ] && PR_URL="(dry run)"
  say "pull request: $PR_URL"
  out "Upstream sync: $COUNT new commit(s) from $UP_BRANCH merged into a pull request$([ "$ASSISTED" = 1 ] && echo ", conflicts or failures resolved by Claude, please review closely"): $PR_URL"
else
  say "NOT ready: $STATE"
  git merge --abort 2>/dev/null
  BODY=$(printf 'The automatic upstream sync for %s (%s new commit(s)) did not finish: **%s**.%s\n\nNew upstream commits:\n\n```\n%s\n```\n\nEnd of the checks log:\n\n```\n%s\n```\n\nThe work so far is on branch `%s` if it could be pushed; the log is %s on the machine that ran it.\n' "$UP_SHA" "$COUNT" "$STATE" "$SUMMARY_BLOCK" "$COMMITS" "$(tail -40 /tmp/sync-checks.out 2>/dev/null)" "$BRANCH" "$LOG")
  PUSHED_NOTE="Nothing was pushed (the merge could not be completed)."
  if [ "$(git rev-list --count "$BASE_REF..HEAD")" -gt 0 ]; then
    if run gitpush "$BRANCH"; then PUSHED_NOTE="The work so far is on branch \`$BRANCH\`."; else say "could not push the branch"; fi
  fi
  BODY=${BODY//"The work so far is on branch \`$BRANCH\` if it could be pushed;"/$PUSHED_NOTE}
  ISSUE_URL=$(run gh issue create --repo "$GH_REPO" --title "Upstream sync $UP_SHA needs attention: $STATE" --body "$BODY" 2>>"$LOG" | tail -1)
  [ "$DRY" = 1 ] && ISSUE_URL="(dry run)"
  say "issue: $ISSUE_URL"
  out "Upstream sync $UP_SHA needs attention ($STATE): $ISSUE_URL"
fi

cd "$REPO" && git worktree remove --force "$WORK" 2>/dev/null
say "done"
