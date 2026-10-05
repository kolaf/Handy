#!/usr/bin/env bash
# Writes the message (stdin) into the Telegram chat session of Hermes, plus a note for the assistant about what a reply means.
# Hermes' own cron delivery does not record the message in the session, so without this the assistant answers "merge it" with
# "which PR?". Uses Hermes' gateway.mirror.mirror_to_session. Never fatal: prints a line to stderr on failure.
export PYTHONPATH="$HOME/.hermes/hermes-agent${PYTHONPATH:+:$PYTHONPATH}"
exec "$HOME/.hermes/hermes-agent/venv/bin/python" - "$@" <<'PY'
import os, re, sys
msg = sys.stdin.read().strip() if not sys.argv[1:] else open(sys.argv[1]).read().strip()
home = ""
for line in open(os.path.expanduser("~/.hermes/.env")):
    if line.startswith("TELEGRAM_HOME_CHANNEL="):
        home = line.split("=", 1)[1].strip().strip("'\"")
if not msg or not home:
    sys.exit("mirror_context: nothing to mirror or no Telegram home channel")
pr = re.search(r"/pull/(\d+)", msg)
issue = re.search(r"/issues/(\d+)", msg)
if pr:
    note = (
        "[Context for the assistant, not shown to the user. The message above came from the daily cron job handy-upstream-sync "
        f"(it syncs the fork kolaf/Handy with upstream cjpais/Handy). It refers to pull request kolaf/Handy#{pr.group(1)} "
        "(branch upstream-sync-* into dev/hotkeys-build) and gives Claude's verdict (MERGE, REVIEW or CAREFUL). If the user answers "
        "with something like 'merge it', 'ship it', 'ok', 'go ahead' or 'merge and build', load the skill handy-upstream-sync and follow it: "
        f"it merges the pull request and tags the next build-N so that CI publishes the Windows portable zip and the deb. Helper: "
        f"~/.hermes/scripts/handy/merge-and-build.sh {pr.group(1)}. If the user says skip, close or reject, close the pull request "
        "instead (gh pr close). If the verdict is CAREFUL, or the message says Claude resolved conflicts, ask for explicit confirmation first.]"
    )
elif issue:
    note = (
        "[Context for the assistant, not shown to the user. The message above came from the daily cron job handy-upstream-sync: the "
        f"sync could not finish and opened issue kolaf/Handy#{issue.group(1)} with the details and the end of the checks log. If the user "
        "asks what to do, read that issue (gh issue view) and the log ~/.cache/hv/upstream-sync.log on this server; the upstream sync "
        "script is ~/dev/Handy/fork/scripts/sync-upstream.sh. Do not merge anything for this message.]"
    )
else:
    sys.exit(0)
from gateway.mirror import mirror_to_session
ok = mirror_to_session("telegram", home, msg + "\n\n" + note, source_label="cron:handy-upstream-sync")
print("mirrored into the Telegram session" if ok else "mirror_context: no matching Telegram session found", file=sys.stderr)
PY
