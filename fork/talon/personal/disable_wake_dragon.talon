# Same context as community/core/modes/sleep_mode_dragon.talon plus our tag (only matters with Dragon).
mode: all
speech.engine: dragon
tag: user.disable_voice_wake
-
^talon wake [<phrase>]$: skip()
