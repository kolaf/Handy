# "drowse" puts Talon to sleep (same as "go to sleep"). Same context as community/core/modes/modes_not_dragon.talon.
# Wake with Ctrl+PageUp (see wake_key_and_tag.talon).
mode: command
mode: dictation
mode: sleep
not speech.engine: dragon
-
^drowse [<phrase>]$: speech.disable()
