#!/usr/bin/env bash
# A throwaway SilverBullet for testing the meeting and journal features. NEVER test against a real notes space: use this.
#   fork/silverbullet/dev-instance.sh start    download (once), set up, seed with invented notes, run on http://127.0.0.1:3010/notes
#   fork/silverbullet/dev-instance.sh stop
#   fork/silverbullet/dev-instance.sh reset    stop, delete everything, start fresh
# Afterwards (token is in $DIR/token):
#   SB_URL=http://127.0.0.1:3010/notes SB_TOKEN=$(cat ~/dev/sb-test/token) cargo test --lib live_ -- --ignored --nocapture
# It mirrors a multi-space server (space "notes" under /notes, per-account API token), like the real one.
set -euo pipefail
DIR=${SB_TEST_DIR:-$HOME/dev/sb-test}; PORT=${SB_TEST_PORT:-3010}; VERSION=${SB_VERSION:-2.12.0}
B=http://127.0.0.1:$PORT
stop() { pkill -f "$DIR/silverbullet $DIR/data" 2>/dev/null || true; }
case "${1:-start}" in
  stop) stop; exit 0;;
  reset) stop; rm -rf "$DIR/data" "$DIR/jar" "$DIR/token";;
esac
mkdir -p "$DIR"; cd "$DIR"
if [ ! -x silverbullet ]; then
  curl -sL -o server.zip "https://github.com/silverbulletmd/silverbullet/releases/download/$VERSION/silverbullet-server-linux-x86_64.zip"
  unzip -qo server.zip && chmod +x silverbullet
fi
[ -d data ] || ./silverbullet setup --admin tester:testpass-123 --space notes --at /notes data >/dev/null
if ! curl -s -o /dev/null -m 2 "$B/notes/.ping"; then
  (nohup ./silverbullet data -L 127.0.0.1 -p "$PORT" > server.log 2>&1 &)
  for _ in $(seq 20); do curl -s -o /dev/null -m 1 "$B/notes/.ping" && break; sleep 1; done
fi
if [ ! -s token ]; then
  rm -f jar
  curl -s -c jar -H "Content-Type: application/json" -d '{"username":"tester","password":"testpass-123"}' "$B/.dashboard/api/login" >/dev/null
  curl -s -b jar -X POST -H "Content-Type: application/json" -d '{"name":"handy-test"}' "$B/.dashboard/api/admin/users/tester/tokens" \
    | python3 -c "import sys,json; print(json.load(sys.stdin)['token'], end='')" > token
  chmod 600 token
fi
T=$(cat token)
put() { # page text: create-only, so a second run changes nothing
  curl -s -o /dev/null -X PUT -H "Authorization: Bearer $T" -H "X-Sync-Mode: true" -H "If-None-Match: *" -H "Content-Type: text/markdown" \
    --data-binary "$2" "$B/notes/.fs/$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$1").md" || true
}
put "Saga" $'---\ntags: project\nstatus: active\npriority: high\n---\n\n# Basic Design\n\n* [ ] Implement the crypto management system as a front end for the crypto implementation. The system owns the mapping between requests and crypto keys, SPI.\n* [ ] Set up the staging environment\n'
put "Website Redesign" $'---\ntags: project\nstatus: active\n---\n\n# Scope\n\n* [ ] Create wireframes\n* [ ] Get client approval #waiting\n'
put "People/Kari Nordmann" $'---\ntags: person\n---\nDesigner at the vendor.\n'
put "Invisible Cities" $'---\ntags: book\n---\nNovel by Italo Calvino #reading\n'
put "Journal/2026-10-06" $'---\ntags: journal\ndate: 2026-10-06\n---\n\n## Done\n* 09:10 Fixed the login bug [[Saga]]\n  * root cause was a null check\n\n## Tomorrow\n* [ ] Call the vendor [due: "2026-10-07"]\n'
echo "SilverBullet test instance: $B/notes   (account tester, token in $DIR/token)"
