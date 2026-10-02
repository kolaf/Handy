# Same context as community/core/modes/sleep_mode_wakeup.talon plus our tag.
mode: sleep
not tag: user.deep_sleep
tag: user.disable_voice_wake
-
^(welcome back)+$: skip()
