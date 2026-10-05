#!/usr/bin/env bash
# Hermes cron (no-agent) job: daily upstream sync of the Handy fork. Its stdout is delivered to Telegram, so it always prints something.
# Installed on the server as ~/.hermes/scripts/handy/sync-upstream.sh (a symlink to this file in the repository clone).
HERE="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"
export UPSTREAM=upstream
# A token with write access to this one repository, used instead of the general gh login (see FORK.md, upstream sync).
[ -s "$HOME/.config/handy-sync/token" ] && export GH_TOKEN="$(tr -d '\r\n ' < "$HOME/.config/handy-sync/token")"
git -C "$HOME/dev/Handy" pull -q --ff-only 2>/dev/null     # the scripts and the fork branch are kept current
out=$("$HOME/dev/Handy/fork/scripts/sync-upstream.sh")
code=$?
if [ -n "$out" ]; then
  echo "$out"
  # A pull request or an issue needs an answer from you: tell the Telegram chat session what the message was about, so that
  # "merge it" works as a reply (the cron delivery itself does not reach the session).
  case "$out" in *"/pull/"*|*"/issues/"*) printf '%s' "$out" | "$HERE/mirror-context.sh" || true ;; esac
elif [ "$code" = 0 ]; then echo "Handy upstream sync ran: no new commits from cjpais/Handy."
else echo "Handy upstream sync ended with exit code $code and no message; see ~/.cache/hv/upstream-sync.log on the server."; fi
