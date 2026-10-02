# Wake and sleep Talon from the keyboard. This replaces the spoken wake commands (disabled in the
# disable_wake_*.talon files here). Ctrl+PageUp toggles speech recognition on and off.
# If this ever fails, the Talon tray icon menu can re-enable speech.
-
tag(): user.disable_voice_wake
key(ctrl-pgup): speech.toggle()
