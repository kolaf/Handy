#!/usr/bin/env bash
# Tests for hv with a stub in place of Hermes: no model calls, no network.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
export HV_STATE_DIR="$T/state" HV_CONFIG_DIR="$T/config" HV_SELECTION_FILE="$T/selection.txt" HV_HERMES="$T/hermes-stub"
pass=0; fail=0
ok()   { pass=$((pass+1)); printf '  ok    %s\n' "$1"; }
bad()  { fail=$((fail+1)); printf '  FAIL  %s\n' "$1"; }
check(){ if eval "$2"; then ok "$1"; else bad "$1"; fi; }

# stub: records argv (one per line, NUL-safe enough for tests) and the prompt, answers like Hermes -Q
cat > "$T/hermes-stub" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$HV_STUB_ARGS"
while [[ $# -gt 0 ]]; do [[ "$1" == "-q" ]] && printf '%s' "$2" > "$HV_STUB_PROMPT"; shift; done
echo "stub answer"
echo "session_id: ${HV_STUB_SID:-STUB1}" >&2
exit "${HV_STUB_RC:-0}"
STUB
chmod +x "$T/hermes-stub"
export HV_STUB_ARGS="$T/args" HV_STUB_PROMPT="$T/prompt"
HV="$HERE/hv"

echo "new request"
out="$("$HV" "copy this file to the reports folder" 2>&1)"
check "prints the model answer"            '[[ "$out" == *"stub answer"* ]]'
check "session id saved"                   '[[ "$(cat "$T/state/last_session")" == "STUB1" ]]'
check "uses -Q and a fresh session"        'grep -qx -- "-Q" "$T/args" && ! grep -qx -- "--resume" "$T/args"'
check "narrow toolsets (no clarify: it blocks)" 'grep -qx "terminal,file,memory,skills" "$T/args"'
check "checkpoints on"                     'grep -qx -- "--checkpoints" "$T/args"'
check "step limit set"                     'grep -A1 -x -- "--max-turns" "$T/args" | grep -qx 12'
check "source tag voice"                   'grep -A1 -x -- "--source" "$T/args" | grep -qx voice'
check "prompt carries the safety preamble" 'grep -q "Terminal safety" "$T/prompt"'
check "prompt is in PLAN phase"            'grep -q "PHASE: PLAN" "$T/prompt"'
check "prompt ends with the request"       'tail -n1 "$T/prompt" | grep -qx "Request: copy this file to the reports folder"'
check "journal line written"               'grep -q "plan" "$T/state/journal.tsv"'
check "places file created with examples"  'grep -q "reports =" "$T/config/places.md"'

echo "context"
printf 'C:\\Users\\me\\a file.txt\n/home/me/b.txt\n' > "$T/selection.txt"
"$HV" "move these" >/dev/null 2>&1
check "selected paths included"            'grep -q "/home/me/b.txt" "$T/prompt"'
check "paths with spaces kept"             'grep -q "a file.txt" "$T/prompt"'
touch -d '10 minutes ago' "$T/selection.txt"
"$HV" "move these" >/dev/null 2>&1
check "stale selection ignored"            'grep -q "Selected paths: (none)" "$T/prompt"'
echo "reports = /data/reports" > "$T/config/places.md"
"$HV" "where is it" >/dev/null 2>&1
check "places file included"               'grep -q "reports = /data/reports" "$T/prompt"'
check "comment lines not sent"             '! grep -q "usual place for invoices" "$T/prompt"'

echo "awkward input"
nasty='say "hi" $(rm -rf /) `id` and '"'"'quote'"'"
"$HV" "$nasty" >/dev/null 2>&1
check "quotes, \$() and backticks arrive verbatim" 'grep -qF -- "Request: $nasty" "$T/prompt"'
nl=$'first line\nsecond line'
"$HV" "$nl" >/dev/null 2>&1
check "newlines kept"                      'grep -q "second line" "$T/prompt"'

echo "sessions"
HV_STUB_SID=SESSION-A "$HV" "copy a to b" >/dev/null 2>&1
"$HV" go >/dev/null 2>&1
check "go resumes the saved session"       'grep -A1 -x -- "--resume" "$T/args" | grep -qx SESSION-A'
check "go sends EXECUTE without preamble"  'grep -q "PHASE: EXECUTE" "$T/prompt" && ! grep -q "Terminal safety" "$T/prompt"'
"$HV" again "no, the other folder" >/dev/null 2>&1
check "again resumes and plans again"      'grep -q "follow-up" "$T/prompt" && grep -qx -- "--resume" "$T/args"'
check "again carries the correction"       'grep -q "Request: no, the other folder" "$T/prompt"'
"$HV" "go to the reports folder" >/dev/null 2>&1
check "'go to ...' is a request, not go"   'grep -q "PHASE: PLAN" "$T/prompt" && ! grep -qx -- "--resume" "$T/args"'
"$HV" ask "what is in here" >/dev/null 2>&1
check "ask is read-only and fresh"         'grep -q "PHASE: ASK" "$T/prompt" && ! grep -qx -- "--resume" "$T/args"'

echo "errors"
rm -f "$T/state/last_session"
out="$("$HV" go 2>&1)"; rc=$?
check "go without a session fails clearly" '[[ $rc -ne 0 && "$out" == *"no earlier hv session"* ]]'
out="$(HV_STUB_RC=3 "$HV" "do something" 2>&1)"; rc=$?
check "model failure exit code passed on"  '[[ $rc -eq 3 ]]'
out="$("$HV" again 2>&1)"; rc=$?
check "again needs text"                   '[[ $rc -ne 0 ]]'
out="$("$HV" --nope x 2>&1)"; rc=$?
check "unknown option rejected"            '[[ $rc -ne 0 ]]'

echo "dry run"
rm -f "$T/args" "$T/prompt"
out="$("$HV" --dry "copy this" 2>&1)"
check "dry run prints prompt, calls nothing" '[[ "$out" == *"PHASE: PLAN"* && ! -e "$T/args" ]]'
echo "extra args"
HV_EXTRA_ARGS="--ignore-rules --verbose" "$HV" "x" >/dev/null 2>&1
check "HV_EXTRA_ARGS appended"              'grep -qx -- "--ignore-rules" "$T/args" && grep -qx -- "--verbose" "$T/args"'

printf '\n%d passed, %d failed\n' "$pass" "$fail"
[[ $fail -eq 0 ]]
