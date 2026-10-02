# Same context as community/core/modes/sleep_mode_not_dragon.talon plus our tag, so this one is more specific
# and wins. skip() does nothing: the spoken wake commands no longer work.
mode: command
mode: dictation
mode: sleep
not speech.engine: dragon
not tag: user.deep_sleep
tag: user.disable_voice_wake
-
^(wake up)+$: skip()
^talon wake [<phrase>]$: skip()
