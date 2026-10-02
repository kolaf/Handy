# Say this while a file manager (Explorer, Nautilus...) has focus. It remembers the selected files for the
# next hermes request, then you switch to the terminal and say e.g. "hermes copy these to the reports folder".
-
grab files$: user.hv_grab_selection()
