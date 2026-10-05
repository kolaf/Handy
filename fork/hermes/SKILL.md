---
name: handy-upstream-sync
description: "Handle the user's answer to the daily Handy upstream-sync message (a cron message with a pull request link and a verdict): merge it and start a CI build, skip it, or explain it. Use when the user says things like merge it, ship it, ok, go ahead, merge and build, skip it, close it, or asks what the sync found."
version: 1.0.0
author: kolaf
license: MIT
platforms: [linux]
metadata:
  hermes:
    tags: [github, handy, ci, release]
---

# Handy upstream sync: what to do with the reply

A cron job (`handy-upstream-sync`, daily 07:30) merges upstream `cjpais/Handy` into a branch of the fork `kolaf/Handy`, runs the checks,
and sends the user a Telegram message: a pull request link (`upstream-sync-*` into `dev/hotkeys-build`), Claude's **verdict** (MERGE, REVIEW or
CAREFUL) and a summary. Or it sends an issue link when the sync could not finish. The message is in this conversation (look back for
"Upstream sync"); if you cannot find which pull request it was, list the open ones:
`gh pr list --repo kolaf/Handy --search "head:upstream-sync-" --state open`.

## If the user says merge / ship it / ok / go ahead / merge and build

1. Identify the pull request (from the message in this conversation). If there are several open, ask which.
2. If the verdict was **CAREFUL**, or the message says Claude resolved conflicts or failures, do **not** merge yet: restate the verdict reason and the
   "test after merging" points in two or three lines and ask for an explicit confirmation ("merge anyway?"). A plain "ok" to the original message
   is not enough for CAREFUL.
3. Run the helper (it does the safety checks itself):
   `~/.hermes/scripts/handy/merge-and-build.sh <PR number>`
   It refuses anything that is not an open upstream-sync pull request, waits for the CI checks (up to 20 minutes), merges with a merge commit,
   deletes the branch, tags the next `build-N` and pushes the tag, which starts the build workflow. It sends a separate Telegram message with the
   download links when the build is finished (about 25 minutes). Do not wait for it.
4. Tell the user, briefly: merged as which commit, which tag, that the build takes about 25 minutes and the links arrive by message.
   Use `--dry-run` first only if the user asks what would happen.
5. If the helper refuses (failing check, still pending, conflicts), say why in one or two lines and offer: wait, or `--ignore-checks` if the user
   says the failing check does not matter (only on their explicit say-so).

Never push to `dev/hotkeys-build` directly, never merge any other pull request with this procedure, and never force anything.

## If the user says skip / close / reject
Close the pull request with a comment: `gh pr close <N> --repo kolaf/Handy --delete-branch --comment "Closed by the owner."`. Tomorrow's sync will
propose the same changes again (plus anything new) unless they are merged or the branch is closed again; mention that.

## If the user asks what the changes are
Read the pull request (`gh pr view <N> --repo kolaf/Handy`): its top section is Claude's review (verdict, summary, changes, overlap with the fork,
what to test, what was not verified). Answer from that in plain words. The app itself is never run by the sync; after a merge the user tries a
dictation with the new build.

## If the message was about an issue (the sync did not finish)
Read it (`gh issue view <N> --repo kolaf/Handy`) and the log `~/.cache/hv/upstream-sync.log` on this server. Explain what failed (a conflict Claude could
not resolve, failing checks, a push problem). The script is `~/dev/Handy/fork/scripts/sync-upstream.sh`; the project notes are in
`~/dev/Handy/FORK.md` (section "Weekly upstream sync"). Do not merge anything for an issue.

## Credentials
The helper and the sync use a token for this one repository in `~/.config/handy-sync/token`. Do not print it, and do not use other credentials for this.
