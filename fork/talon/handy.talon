# Voice commands and hotkey for driving Handy from Talon.
-

# One key: starts/stops a Handy dictation and mutes/unmutes Talon around it.
# Do NOT bind the same key inside Handy -- on X11 only one program can grab a key.
key(f13): user.handy_toggle()

handy start: user.handy_toggle()
handy stop: user.handy_toggle()
handy cancel: user.handy_cancel()
handy reply: user.handy_reply()
handy rerun: user.handy_rerun()

handy norwegian: user.handy_language("no")
handy english: user.handy_language("en")
handy auto language: user.handy_language("auto")
handy swap language: user.handy_swap_language()

handy next style: user.handy_next_prompt()
handy style {user.handy_style}: user.handy_prompt(handy_style)

handy auto styles on: user.handy_auto_prompt_set(true)
handy auto styles off: user.handy_auto_prompt_set(false)
