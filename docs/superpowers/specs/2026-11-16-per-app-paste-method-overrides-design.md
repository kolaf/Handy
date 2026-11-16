# Per-app Paste Method Overrides Design

## Intent

Handy currently uses one global paste method for every target application. That is not sufficient on Linux/X11 because different applications accept different paste chords: terminals often need `Ctrl+Shift+V`, while browser/PWA text inputs such as Teams in Vivaldi expect `Ctrl+V`. Direct typing is slower and less reliable for the user's workflow. Handy should support a global default paste method plus per-app exceptions, with a UI consistent with existing per-app prompt rules.

## Goals

- Let the user keep one global default paste method.
- Let the user add app-specific paste method overrides.
- Reuse Handy's existing app-rule matching semantics: app match required, optional title substring, title-specific rules take precedence over app-only rules.
- Make the first useful built-in rule easy: terminal apps can be overridden to `Ctrl+Shift+V` while the default remains `Ctrl+V`.
- Keep existing settings working unchanged when no override rules are configured.

## Non-goals

- No giant preset catalog of applications.
- No automatic migration that silently adds many app-specific exceptions.
- No per-app typing-tool override in this change.
- No external-script UI or script management in this change.

## Constraints

- Follow the existing `AppPrompt`/active-window matching pattern in `src-tauri/src/context.rs`.
- Preserve current behavior when the feature is disabled or the rule list is empty.
- Settings must remain serializable through the existing Tauri settings pipeline and reflected in generated bindings.
- UI should fit naturally into existing Handy settings groups and translation structure.
- The initial shipped user experience should work with only a small starter ruleset; terminal override is enough.

## Approaches considered

### 1. External script only

This would solve the user's immediate workflow but pushes the complexity out of Handy, is less discoverable, and gives no settings UI. It also duplicates app matching logic outside the app.

### 2. Terminal-only hardcoded exception

This would solve today's terminal/browser split quickly, but it would not satisfy the broader need for per-app exceptions and would create rework when other applications need their own override.

### 3. Full per-app paste-method override mechanism reusing app-rule matching

This adds a default plus exceptions model to Handy itself, matching the user's request and Handy's existing architecture. It is the most maintainable approach and supports a minimal initial ruleset without locking the implementation to terminals forever.

**Chosen:** approach 3.

## Design

### Settings model

Add two new persisted settings:

- `app_paste_methods_enabled: boolean`
- `app_paste_methods: AppPasteMethod[]`

Add a new rule type:

- `AppPasteMethod { app: string, title?: string, paste_method: PasteMethod }`

This mirrors `AppPrompt` closely so matching, validation, and UI behavior stay familiar.

### Matching behavior

Reuse the existing context matching semantics:

- normalize executable name the same way as app prompts
- require app name match
- optional title substring narrows the match
- rules with a non-empty title win over app-only rules
- first matching rule wins

Use the active app context of the dictation start (or current foreground app for non-recording actions that paste later the same way prompt choice does).

### Paste method resolution

Introduce a helper that resolves the effective paste method before paste begins:

1. start with the global `settings.paste_method`
2. if `app_paste_methods_enabled` is false or there are no rules, keep it
3. otherwise match the started-in/current app context against the rules
4. if a rule matches, use that rule's `paste_method`

This helper should be called in the paste path before backend/tool selection so every later branch sees the already-resolved method.

### UI

Add a new settings section near paste/output settings:

- toggle to enable per-app paste overrides
- list of rules
- each rule has:
  - app field
  - optional title field
  - paste method dropdown
- add/remove controls mirroring `AppPrompts`

This first version should allow the full mechanism, but it does not need to prepopulate a large ruleset. A single starter rule for terminals is enough if we decide to seed one explicitly; otherwise the UI should make adding it easy.

### Validation

Add backend validation similar to app prompts:

- cap number of rules
- require non-empty app name
- reject control characters / oversized fields
- ensure `paste_method` is a valid enum value

### Starter behavior

The implementation should support a terminal rule cleanly, but should not force many defaults. If a starter rule is seeded, it should be minimal and explicit.

## Risks / review focus

- Rule resolution may accidentally use the wrong app context compared with prompt rules or paste focus guard timing.
- Settings/UI duplication may drift from `AppPrompts` if the shared pattern is not followed closely.
- Non-recording actions like paste-last and rerun may need careful context choice when resolving overrides.
- Existing tests around paste method behavior must keep passing when no per-app rules are configured.
