# Per-app Paste Method Overrides Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a default-plus-exceptions paste-method system to Handy so users can keep one global paste method and override it per application, starting with terminal use cases.

**Architecture:** Reuse Handy's existing active-window matching model and app-rule patterns. Add a new persisted rule type for paste overrides, resolve the effective paste method in the backend before paste execution, and add a settings UI modeled on the existing per-app prompt rules.

**Tech Stack:** Rust/Tauri backend, React/TypeScript frontend, Zustand settings store, i18next translations, generated Tauri bindings.

**Spec:** `docs/superpowers/specs/2026-11-16-per-app-paste-method-overrides-design.md`

## Global Constraints

- Preserve current behavior exactly when per-app paste overrides are disabled or empty.
- Reuse the existing app/title matching semantics from `src-tauri/src/context.rs`.
- Keep settings compatible with the existing persistence and generated bindings pipeline.
- UI copy must go through i18n.
- Start with a small useful ruleset; do not add a large application catalog.

## Review Focus

- A dictation that starts in one app and finishes in another should still resolve the paste override from the intended context.
- Title-specific rules should beat app-only rules for the same executable.
- Paste-last and rerun should resolve overrides consistently with their current app-context behavior.
- Empty or disabled rule lists should leave the global paste method untouched.
- Terminal overrides should not accidentally affect browser/PWA inputs with similar titles.

---

### Task 1: Add persisted backend rule model and resolution

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/context.rs`
- Modify: `src-tauri/src/clipboard.rs`
- Test: backend unit tests in `src-tauri/src/context.rs` and/or `src-tauri/src/clipboard.rs`

**Interfaces:**
- Consumes: existing `PasteMethod`, `AppSettings`, `AppContext`, `matching_rule` semantics
- Produces: `AppPasteMethod` rule type, validation/update helpers, effective paste-method resolution helper used by paste execution

- [ ] **Step 1: Write failing backend tests for paste-method rule matching and fallback behavior**

Add tests covering:
- no rules or disabled rules → global paste method remains unchanged
- app-only rule match → override wins
- title-specific rule beats app-only rule
- terminal executable rule can resolve `CtrlShiftV` while default stays `CtrlV`

- [ ] **Step 2: Run backend tests to verify they fail for the missing rule model/resolution**

Run: `cargo test app_paste_method`
Expected: FAIL because the new types/helpers do not exist yet

- [ ] **Step 3: Implement backend settings and rule resolution**

Add `AppPasteMethod { app: String, title: String, paste_method: PasteMethod }` plus:
- persisted settings fields for enable flag and rule list
- validation function mirroring app-prompt rules
- matching/resolution helper that returns the effective paste method for a given context
- paste-path integration in `clipboard.rs` so downstream injection sees the resolved method

- [ ] **Step 4: Run targeted backend tests to verify they pass**

Run: `cargo test app_paste_method`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/context.rs src-tauri/src/clipboard.rs
git commit -m "feat: add backend per-app paste overrides"
```

### Task 2: Expose commands and generated bindings for the new settings

**Files:**
- Modify: `src-tauri/src/context.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/stores/settingsStore.ts`
- Modify: `src/bindings.ts` (via regeneration)
- Test: existing command/settings tests plus targeted backend tests if needed

**Interfaces:**
- Consumes: backend rule types and validation from Task 1
- Produces: Tauri commands for updating rule list and enabled flag; frontend-accessible bindings/store update functions

- [ ] **Step 1: Write failing tests or compile checks for the missing commands/bindings path**

Add or rely on focused tests/compile expectations covering:
- updating the enabled flag
- updating the rule list through backend validation

- [ ] **Step 2: Run the relevant test/build command to verify the command/binding path fails before implementation**

Run: `cargo test update_app_paste` or the closest new targeted test command
Expected: FAIL because the commands do not exist yet

- [ ] **Step 3: Implement command and store plumbing**

Add Tauri commands parallel to app prompts:
- change enabled flag
- update rule list

Wire them into `lib.rs`, `settingsStore.ts`, and regenerate `src/bindings.ts` using the project's normal debug generation path.

- [ ] **Step 4: Run focused verification for the new plumbing**

Run: `cargo test update_app_paste`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/context.rs src-tauri/src/lib.rs src/stores/settingsStore.ts src/bindings.ts
git commit -m "feat: expose app paste override settings"
```

### Task 3: Add settings UI for per-app paste overrides

**Files:**
- Create or modify: `src/components/settings/...` components for the new rule editor
- Modify: `src/components/settings/PasteMethod.tsx` or the nearest settings grouping component
- Modify: `src/i18n/locales/en/translation.json`
- Test: frontend typecheck/build verification

**Interfaces:**
- Consumes: store actions and bindings from Task 2; existing paste-method enum values
- Produces: enabled toggle and editable exception list in settings UI

- [ ] **Step 1: Write the failing frontend expectation by adding the component usage/types first**

Create the component wiring so TypeScript/build fails until the new props/store paths and translation keys are fully implemented.

- [ ] **Step 2: Run frontend verification to confirm the UI path fails before completion**

Run: `bun run build`
Expected: FAIL with missing component/store/translation references for the new UI

- [ ] **Step 3: Implement the UI**

Model it closely on `AppPrompts`:
- enabled toggle
- repeated rows with app, optional title, paste-method select, remove button
- add button gated on a minimally valid app/method entry
- clear explanatory copy that the global paste method remains the default and matching rules override it

- [ ] **Step 4: Run frontend verification to confirm the UI builds**

Run: `bun run build`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/components/settings src/i18n/locales/en/translation.json
git commit -m "feat: add per-app paste override settings UI"
```

### Task 4: Seed or support terminal-first usage and verify end-to-end behavior

**Files:**
- Modify: backend/frontend files needed for starter-rule behavior or UX guidance
- Test: `src-tauri` tests plus full build/test commands

**Interfaces:**
- Consumes: full backend/frontend feature from Tasks 1-3
- Produces: usable terminal-first override workflow and verified end-to-end implementation

- [ ] **Step 1: Add failing tests for the terminal-first override behavior you choose**

Cover one of:
- seeded default terminal rule if the implementation chooses to ship one, or
- explicit backend/frontend tests proving a terminal rule can be added and resolved correctly without special-case logic

- [ ] **Step 2: Run targeted tests to verify the chosen terminal-first behavior fails before implementation**

Run: `cargo test terminal_paste_override`
Expected: FAIL until the chosen behavior is implemented

- [ ] **Step 3: Implement the smallest terminal-first completion step**

Prefer no hardcoded catalog. Either:
- ship no prepopulation but ensure the UI/backend cleanly support adding a terminal rule, or
- seed exactly one terminal override in the least surprising way if that better fits current Handy conventions.

- [ ] **Step 4: Run full verification**

Run: `cargo test --lib`
Expected: PASS

Run: `bun run build`
Expected: PASS

- [ ] **Step 5: Build the Debian package for user testing**

Run: `bun run tauri build --bundles deb --config src-tauri/tauri.fork.conf.json`
Expected: `.deb` produced under `src-tauri/target/release/bundle/deb/`

- [ ] **Step 6: Commit**

```bash
git add src-tauri src docs
git commit -m "feat: add per-app paste method overrides"
```
