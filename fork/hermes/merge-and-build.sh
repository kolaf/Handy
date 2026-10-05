#!/usr/bin/env bash
# Merge the daily upstream-sync pull request of the Handy fork and start a CI build.
#   merge-and-build.sh PR_NUMBER [--dry-run] [--ignore-checks]
# 1. refuse anything that is not an open upstream-sync-* pull request into dev/hotkeys-build, or that cannot be merged cleanly
# 2. wait (up to 20 minutes) for the pull request's CI checks; a failing check stops it unless --ignore-checks
# 3. merge it (a merge commit, so upstream's history is kept) and delete the branch
# 4. tag the merge commit build-N (N = highest existing + 1) and push the tag: this starts .github/workflows/fork-build.yml,
#    which builds the portable zip and the deb and publishes a release marked latest
# 5. in the background, tell Telegram when the build is finished, with the download links
# --dry-run does steps 1 and 2 and says what it would do. Run on the server, as the owner, with the repository-scoped token.
set -uo pipefail
PR=${1:-}; shift || true
DRY=0; IGNORE=0
for a in "$@"; do case "$a" in --dry-run) DRY=1;; --ignore-checks) IGNORE=1;; esac; done
[[ "$PR" =~ ^[0-9]+$ ]] || { echo "usage: merge-and-build.sh PR_NUMBER [--dry-run] [--ignore-checks]"; exit 64; }
HERE="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
SLUG=kolaf/Handy; BASE=dev/hotkeys-build; REPO=${REPO:-$HOME/dev/Handy}
export PATH="$HOME/.local/bin:$PATH"
[ -s "$HOME/.config/handy-sync/token" ] && export GH_TOKEN="$(tr -d '\r\n ' < "$HOME/.config/handy-sync/token")"

read -r STATE HEAD BASEREF MERGEABLE TITLE < <(gh pr view "$PR" --repo $SLUG --json state,headRefName,baseRefName,mergeable,title \
  -q '[.state,.headRefName,.baseRefName,.mergeable,(.title|gsub(" ";"_"))]|@tsv' 2>/dev/null) || { echo "Cannot read pull request #$PR."; exit 1; }
echo "PR #$PR: ${TITLE//_/ } ($HEAD -> $BASEREF), $STATE, $MERGEABLE"
[ "$STATE" = OPEN ] || { echo "Refusing: the pull request is $STATE, not open."; exit 3; }
[[ "$HEAD" == upstream-sync-* && "$BASEREF" == "$BASE" ]] || { echo "Refusing: this is not an upstream-sync pull request into $BASE."; exit 3; }
[ "$MERGEABLE" = CONFLICTING ] && { echo "Refusing: the pull request has conflicts."; exit 3; }

deadline=$((SECONDS + 1200))
while :; do
  checks=$(gh pr checks "$PR" --repo $SLUG 2>/dev/null)
  failed=$(printf '%s\n' "$checks" | awk -F'\t' '$2=="fail"{print $1}')
  pending=$(printf '%s\n' "$checks" | awk -F'\t' '$2=="pending"{print $1}')
  [ -n "$failed" ] && break
  [ -z "$pending" ] && break
  [ $SECONDS -ge $deadline ] && { echo "Still waiting for: $(echo $pending). Nothing was merged."; exit 4; }
  [ "$DRY" = 1 ] && { echo "Checks still running: $(echo $pending)"; break; }
  sleep 30
done
if [ -n "$failed" ] && [ "$IGNORE" != 1 ]; then echo "Refusing: failing check(s): $(echo $failed). Nothing was merged. (--ignore-checks overrides.)"; exit 2; fi
echo "Checks: ${failed:+failing ($(echo $failed)), ignored; }${pending:+still running ($(echo $pending)); }${failed:-${pending:-all passed}}" | sed 's/  */ /g'

LAST=$(git ls-remote --tags "https://github.com/$SLUG.git" 'refs/tags/build-*' 2>/dev/null | sed 's|.*refs/tags/build-||; s|\^{}||' | grep -E '^[0-9]+$' | sort -n | tail -1)
TAG="build-$(( ${LAST:-0} + 1 ))"
if [ "$DRY" = 1 ]; then echo "Dry run: would merge #$PR (merge commit, delete branch $HEAD), then tag $TAG on the merge commit and push it to start the CI build."; exit 0; fi

gh pr merge "$PR" --repo $SLUG --merge --delete-branch >/tmp/merge-pr.out 2>&1 || { echo "Merge failed: $(tail -3 /tmp/merge-pr.out)"; exit 1; }
SHA=""
for i in 1 2 3 4 5; do SHA=$(gh pr view "$PR" --repo $SLUG --json mergeCommit -q .mergeCommit.oid 2>/dev/null); [ -n "$SHA" ] && break; sleep 3; done
[ -n "$SHA" ] || { echo "Merged #$PR, but could not read the merge commit, so no build was tagged."; exit 1; }
echo "Merged #$PR as ${SHA:0:7}."
cd "$REPO" || exit 1
git fetch -q origin "$BASE" 2>/dev/null
git tag "$TAG" "$SHA" 2>/dev/null
git -c credential.helper= -c credential.helper='!gh auth git-credential' push -q origin "$TAG" 2>/tmp/merge-tag.out || { echo "Merged, but the tag $TAG could not be pushed: $(tail -2 /tmp/merge-tag.out)"; exit 1; }
echo "Tagged $TAG; the CI build is starting (about 25 minutes)."

sleep 15
RUN=$(gh run list --repo $SLUG --workflow fork-build.yml --branch "$TAG" --limit 1 --json databaseId -q '.[0].databaseId' 2>/dev/null)
[ -n "$RUN" ] && echo "Run: https://github.com/$SLUG/actions/runs/$RUN"
# Tell Telegram when it is finished (a separate process, so it outlives the conversation; the token is inherited through the environment).
export GH_TOKEN
nohup "$HERE/watch-build.sh" "$RUN" "$TAG" >/dev/null 2>&1 &
disown 2>/dev/null
