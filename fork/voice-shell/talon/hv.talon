# Voice shell commands: they type an `hv` request into the focused terminal, so the output appears where you
# are working and the terminal's current folder is used automatically.
tag: terminal
-
hermes <user.text>$: user.hv_request(text)
hermes (go | yes | go ahead | do it)$: user.hv_go()
hermes again <user.text>$: user.hv_again(text)
hermes ask <user.text>$: user.hv_ask(text)
hermes cancel$: user.hv_cancel()
