#!/usr/bin/env bash
# watch-build.sh RUN_ID TAG: waits for the CI build to finish and tells Telegram, with the download links. Started by merge-and-build.sh.
export PATH="$HOME/.local/bin:$PATH"
RUN=${1:-}; TAG=${2:-build}; SLUG=kolaf/Handy
[ -n "$RUN" ] && gh run watch "$RUN" --repo "$SLUG" >/dev/null 2>&1
C=$([ -n "$RUN" ] && gh run view "$RUN" --repo "$SLUG" --json conclusion -q .conclusion 2>/dev/null)
if [ "$C" = success ]; then
  hermes send -t telegram "Fork build $TAG finished: success.
Windows portable zip: https://github.com/$SLUG/releases/latest/download/Handy-portable.zip
Ubuntu deb: https://github.com/$SLUG/releases/latest/download/Handy_amd64.deb
Not tried on a machine yet: try dictation with the new build before relying on it."
else
  hermes send -t telegram "Fork build $TAG did not succeed (${C:-unknown}): https://github.com/$SLUG/actions/runs/${RUN:-?}"
fi
