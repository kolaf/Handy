#!/usr/bin/env bash
# Writes the message (stdin) into the Telegram chat session of Hermes, plus a note for the assistant about what a reply means.
# Hermes' own cron delivery does not record the message in the session, so without this the assistant answers "merge it" with
# "which PR?". Uses Hermes' gateway.mirror.mirror_to_session. Never fatal: prints a line to stderr on failure.
export PYTHONPATH="$HOME/.hermes/hermes-agent${PYTHONPATH:+:$PYTHONPATH}"
HERE="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
exec "$HOME/.hermes/hermes-agent/venv/bin/python" "$HERE/mirror_context.py" "$@"
