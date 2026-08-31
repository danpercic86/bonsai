# P49 — External Integrations

Launch external tools at a repo / worktree / submodule / tab path: **open in terminal**,
**reveal in file manager**, **open in editor**. External launch only (no embedded terminal).

References read: `commands/{mod,shared,ui_settings,health}.rs`, `settings.rs`, `error.rs`,
`ai/mod.rs` (Command idiom), `lib.rs`, `capabilities/default.json`, `types.ts`, `tauri.ts`,
`mock.ts` + `mock/{persistence,handlers/session}.ts`, `workspaceMenus.ts`, `TabStrip.tsx`,
`WorkspaceToolbar.tsx`, `SettingsPanel.tsx`. House pattern: P40-config-editing, P47-cherry-pick.

---

## 0. Key decisions (with rationale)

**D1 — Self-contained Rust, NO plugin, NO `open` crate.** Implement all three as ordinary
app `#[tauri::command]`s that shell out via `std::process::Command` from a new
`bonsai-core::external` module.
- *Capabilities:* app-defined commands registered in `generate_handler!` are **not** gated by
  the capability system (only plugin/core commands are — that is why the current 125 commands run
  with just `core:default` + three plugin perms). ⇒ **`capabilities/default.json` is UNCHANGED.**
  Adopting `tauri-plugin-opener` would add a dependency **and** new capability entries
  (`opener:allow-open-path`, `opener:allow-reveal-item-in-dir`) for no gain, and still would not
  cover the template-based terminal spawn.
- *Idiom reuse:* `ai/mod.rs`/`scheduler.rs` already spawn children via `std::process::Command`
  with the Windows `CREATE_NO_WINDOW` (`0x0800_0000`) flag — we reuse that idiom.
- *Control:* only a self-contained spawn gives us the arg-separated launch + per-OS fallback
  ladder + the "terminal window must stay visible" flag. The `open` crate is rejected: it adds a
  dep for ~3 lines of reveal logic, does not solve terminal/editor, and mixing it in is
  inconsistent.

**D2 — Safety: arg-separated spawn, never a shell string.** Every launch is
`program + [args…] + explicit cwd`. The user template is **tokenized** and `{path}` is substituted
**inside a single argv token** (so `--working-directory={path}` becomes ONE element
`--working-directory=/actual/path`). No `sh -c`, no `cmd /c "<line>"`, no string interpolation ⇒
immune to the Windows Terminal `;` sub-command delimiter, to path-injection, and to spaces.

**D3 — AppError: add one variant `ExternalToolFailed(String)`** (kind `"externalToolFailed"`).
Rationale: the taxonomy is already granular (20+ specific kinds); a distinct user-facing
"could not launch external tool" class lets the toast carry a Settings hint. A **missing path**
precheck uses the existing `AppError::Io` (it is literally a filesystem condition). Alternative
(reuse `Other`) is acceptable but less consistent with house style — recommend the new variant.

**D4 — Settings: single per-machine `terminal_command` + `editor_command` strings** (empty =
auto-detect). Not a per-OS triple: `settings.json` is per-machine and every other setting is
stored flat. The *defaults* are per-OS (auto-detect ladder); the *stored value* is one string for
this machine. Ride on the existing `get_ui_settings`/`set_ui_settings` patch path — **no new
settings command**.

**Command count: 125 → 128** (`open_in_terminal`, `reveal_in_file_manager`, `open_in_editor`).

Open questions for the orchestrator are in §10.

---

## 1. Module boundaries / files

**New**
- `crates/bonsai-core/src/external.rs` — pure argv builders + `CommandRunner` trait + production
  `SpawnRunner` + thin orchestration. OS-branched via an explicit `TargetOs` param (NOT `cfg!`) so
  every branch is unit-testable on one machine.
- `src-tauri/src/commands/external.rs` — the 3 `#[tauri::command]`s + `_inner` helpers.
- `src/components/SettingsExternalToolsSection.tsx` — the Settings UI section (own file).
- `src/ipc/mock/handlers/external.ts` — mock handler group (success + failure).

**Edited**
- `crates/bonsai-core/src/lib.rs` — `pub mod external;`
- `crates/bonsai-core/src/error.rs` — add `ExternalToolFailed`.
- `src-tauri/src/settings.rs` — 2 additive fields + Default + back-compat test.
- `src-tauri/src/commands/ui_settings.rs` — `UiSettings`/`UiSettingsPatch`/`apply_patch` + mapping.
- `src-tauri/src/commands/mod.rs` — `mod external; pub use external::*;`
- `src-tauri/src/lib.rs` — register 3 commands in `generate_handler!`.
- `src/ipc/{types,tauri,mock}.ts`, `src/ipc/mock/{persistence,handlers/session}.ts`.
- `src/components/workspaceMenus.ts`, `WorkspaceToolbar.tsx`, `TabStrip.tsx`, `SettingsPanel.tsx`,
  and the App/RepoWorkspace container that threads the new handlers/props.
- **`capabilities/default.json` — NO CHANGE** (see D1).

---

## 2. Backend core — `crates/bonsai-core/src/external.rs`

```rust
use std::path::{Path, PathBuf};
use crate::error::AppError;

/// Which OS we build argv for. `host()` is used in production; tests pass each
/// variant explicitly so all branches run on one machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs { Windows, MacOs, Linux }

impl TargetOs {
    pub fn host() -> TargetOs; // cfg!(target_os) → Windows | MacOs | Linux(else)
}

/// A fully-resolved child launch. Pure output of the builders; the runner turns
/// it into a real spawn. NEVER a shell command line — `program` + separate args.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Windows only: suppress a transient console window (VS Code's `code.cmd`
    /// shim, explorer). MUST be `false` for terminal launches (we WANT the
    /// window). Ignored on macOS/Linux.
    pub hide_console: bool,
}

/// Injected so argv-building + ladder logic are testable without launching apps.
/// `Ok(())` = spawned (we do NOT wait — the child outlives Bonsai); `Err(msg)` =
/// this candidate failed, caller tries the next ladder entry (or surfaces it if
/// it was the last).
pub trait CommandRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String>;
}

/// Production runner: builds `std::process::Command`, sets program/args/cwd,
/// applies `CREATE_NO_WINDOW` iff `spec.hide_console`, then `spawn()` (no wait).
pub struct SpawnRunner;
impl CommandRunner for SpawnRunner { /* see notes */ }

// ---- pure builders (no fs, no spawn) ----

/// Tokenize `template` on whitespace honoring double-quotes, substitute the
/// literal `{path}` substring inside each token with `path.display()`, take
/// token[0] as program + rest as args, cwd = path. `None` if no tokens
/// (empty/whitespace template).
pub fn parse_template(template: &str, path: &Path, hide_console: bool) -> Option<LaunchSpec>;

/// Ordered terminal ladder. Non-empty template ⇒ `vec![parsed]`; empty ⇒ the
/// per-OS auto ladder (all with hide_console=false).
pub fn terminal_ladder(os: TargetOs, template: &str, path: &Path) -> Vec<LaunchSpec>;

/// Single reveal-in-file-manager spec (not configurable). Opens the directory
/// in the OS file manager (hide_console=true).
pub fn reveal_spec(os: TargetOs, path: &Path) -> LaunchSpec;

/// Ordered editor ladder. Non-empty template ⇒ `vec![parsed]`; empty ⇒ the
/// per-OS auto ladder (all hide_console=true).
pub fn editor_ladder(os: TargetOs, template: &str, path: &Path) -> Vec<LaunchSpec>;

// ---- thin orchestration ----

/// Try each spec in order via `runner`; first `Ok` wins. If all fail, return
/// `AppError::ExternalToolFailed` naming the last program + error. `what` is a
/// label ("terminal" | "file manager" | "editor").
pub fn launch_first(runner: &dyn CommandRunner, ladder: &[LaunchSpec], what: &str) -> Result<(), AppError>;

/// Entry points the command layer calls (path is assumed to already exist — the
/// command does the fs precheck; core does NO fs so builders stay pure).
pub fn open_in_terminal(runner: &dyn CommandRunner, os: TargetOs, template: &str, path: &Path) -> Result<(), AppError>;
pub fn reveal_in_file_manager(runner: &dyn CommandRunner, os: TargetOs, path: &Path) -> Result<(), AppError>;
pub fn open_in_editor(runner: &dyn CommandRunner, os: TargetOs, template: &str, path: &Path) -> Result<(), AppError>;
```

### 2.1 Per-OS default tables (used when the template is empty)

`P` = the target path string. cwd is always `P` unless noted.

**Terminal ladder** (`hide_console=false`):

| OS | Candidates in order (program → args) |
|----|--------------------------------------|
| Windows | `wt` → `["-d", P]`  •  `powershell` → `[]`  •  `cmd` → `["/K"]` |
| macOS | `open` → `["-a", "Terminal", P]` |
| Linux | `gnome-terminal` → `["--working-directory=<P>"]`  •  `konsole` → `["--workdir", P]`  •  `x-terminal-emulator` → `[]` |

- Windows `wt` is an App-Execution-Alias / may be uninstalled: if `spawn()` fails it falls through
  to PowerShell then cmd (see D2/§2.2). PowerShell/cmd spawned from a GUI process allocate their
  own **visible** console at `cwd` — so `hide_console=false` and cwd carries the directory
  (no `cd` in a command string).
- macOS default is a single candidate; the user swaps to iTerm by setting the template to
  `open -a iTerm {path}`.

**Reveal spec** (`hide_console=true`):

| OS | program → args |
|----|----------------|
| Windows | `explorer` → `[P]` |
| macOS | `open` → `[P]` |
| Linux | `xdg-open` → `[P]` |

- Opens the directory itself (our entry-point paths are always directories). `explorer` returns a
  nonzero exit code even on success — irrelevant because we spawn-and-don't-wait (only `spawn()`
  failure is a failure).

**Editor ladder** (`hide_console=true`):

| OS | Candidates in order (program → args) |
|----|--------------------------------------|
| Windows / Linux | `code` → `[P]`  •  `code-insiders` → `[P]` |
| macOS | `open` → `["-a", "Visual Studio Code", P]`  •  `open` → `["-a", "Visual Studio Code - Insiders", P]`  •  `code` → `[P]` |

- If nothing in the ladder launches, `launch_first` returns `ExternalToolFailed` → toast:
  *"No editor found. Set an editor command in Settings."* (frontend message; backend supplies the
  base error).

### 2.2 `launch_first` / `SpawnRunner` pseudocode

```
launch_first(runner, ladder, what):
    last_err = None
    for spec in ladder:
        match runner.run(spec):
            Ok  -> return Ok
            Err(e) -> last_err = Some((spec.program, e))
    return Err(ExternalToolFailed(
        match last_err {
            Some((prog, e)) => "could not launch {what} ({prog}): {e}",
            None            => "no {what} command is configured",
        }))

SpawnRunner::run(spec):
    let mut cmd = Command::new(&spec.program)   // Windows: resolve via PATH+PATHEXT (see note)
    cmd.args(&spec.args).current_dir(&spec.cwd)
    #[cfg(windows)] if spec.hide_console { cmd.creation_flags(0x0800_0000) } // CREATE_NO_WINDOW
    match cmd.spawn() {                          // do NOT wait
        Ok(_child) => Ok(()),                    // drop the handle; child is detached
        Err(e)     => Err(e.to_string()),        // any failure ⇒ caller tries next candidate
    }
```

**Windows PATHEXT note (implementation).** `std::process::Command::new("code")` does **not**
auto-resolve `code.cmd`. `SpawnRunner` must, on Windows, resolve a bare program name against `PATH`
trying the name as-is then with each `PATHEXT` extension (`.exe`, `.cmd`, `.bat`, …); an
unresolvable name ⇒ `Err` (ladder falls through). macOS/Linux use normal `PATH` resolution
(`spawn()` returns `NotFound` → `Err`). Keep this resolver in `SpawnRunner` so the pure builders
stay fs-free.

### 2.3 `parse_template` rules
- Split on ASCII whitespace **outside** double quotes; strip the surrounding quotes from a token.
- In each token, replace every literal `{path}` occurrence with `path.display().to_string()`
  (substring replacement — supports both standalone `{path}` and embedded `--flag={path}`).
- `token[0]` → `program`; remaining → `args`; `cwd = path`; `hide_console` from the caller.
- All-whitespace / empty ⇒ `None` (caller falls back to the auto ladder).
- The template is **never** passed to a shell; a `;`/`&&`/`|` in it becomes literal argv text.

---

## 3. Backend — `error.rs`

Add variant + wire `kind()`/`message()`/doc comment:

```rust
#[error("{0}")]
ExternalToolFailed(String),
// kind()    => "externalToolFailed"
// message() => joins the m arm
```

---

## 4. Backend — commands (`src-tauri/src/commands/external.rs`)

House shape `X → X_inner → spawn_blocking(core::fn)`. Path arrives as a raw string (frontend passes
a repo/worktree/submodule/tab path it already owns). Terminal/editor read the template from
`settings.json`; reveal needs neither `AppHandle` nor state.

```rust
use super::shared::*;
use bonsai_core::external::{self, SpawnRunner, TargetOs};

#[tauri::command]
pub async fn open_in_terminal(app: tauri::AppHandle, path: String) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    launch_inner(Some(file), Action::Terminal, path).await
}

#[tauri::command]
pub async fn reveal_in_file_manager(path: String) -> Result<(), AppError> {
    launch_inner(None, Action::Reveal, path).await
}

#[tauri::command]
pub async fn open_in_editor(app: tauri::AppHandle, path: String) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    launch_inner(Some(file), Action::Editor, path).await
}

enum Action { Terminal, Reveal, Editor }

/// spawn_blocking: (1) fs precheck the path exists → AppError::Io if not; (2) for
/// Terminal/Editor load the template from settings; (3) call the matching
/// external:: entry with SpawnRunner + TargetOs::host().
async fn launch_inner(settings_file: Option<PathBuf>, action: Action, path: String) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = std::path::Path::new(&path);
        if !p.exists() { return Err(AppError::Io(format!("path no longer exists: {path}"))); }
        let os = TargetOs::host();
        let runner = SpawnRunner;
        match action {
            Action::Reveal => external::reveal_in_file_manager(&runner, os, p),
            Action::Terminal => {
                let t = settings_file.map(|f| settings::load_from(&f).terminal_command).unwrap_or_default();
                external::open_in_terminal(&runner, os, &t, p)
            }
            Action::Editor => {
                let t = settings_file.map(|f| settings::load_from(&f).editor_command).unwrap_or_default();
                external::open_in_editor(&runner, os, &t, p)
            }
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

`lib.rs` `generate_handler!` gains (place after the worktree/health block):
`commands::open_in_terminal, commands::reveal_in_file_manager, commands::open_in_editor`.
`commands/mod.rs`: `mod external; pub use external::*;`.

---

## 5. Settings additive fields (`settings.rs` + `ui_settings.rs`)

`Settings` gains (container already has `#[serde(default)]`, so purely additive):
```rust
/// P49: terminal launch command template ("{path}" placeholder). Empty ⇒
/// per-OS auto-detect. Additive #[serde(default)] ⇒ pre-P49 files load "".
pub terminal_command: String,
/// P49: editor launch command template. Empty ⇒ auto-detect (VS Code family).
pub editor_command: String,
```
`Default` → both `String::new()`. No clamp (free-form). serde keys: `terminalCommand`,
`editorCommand`.

`UiSettings` + `UiSettingsPatch` mirror them (`String` / `Option<String>`); `apply_patch` gains:
```rust
if let Some(v) = patch.terminal_command { s.terminal_command = v; }
if let Some(v) = patch.editor_command   { s.editor_command   = v; }
```
Add both fields to the `get_ui_settings` and `set_ui_settings` `UiSettings { … }` constructors.

New back-compat test in `settings.rs`:
`old_settings_file_without_external_commands_loads_defaults` — a pre-P49 JSON loads
`terminal_command == "" && editor_command == ""`, existing fields untouched; plus a round-trip
asserting the camelCase keys serialize.

---

## 6. IPC surface (TypeScript)

### 6.1 `types.ts`
- `AppError.kind` union (≈L1169): add `| 'externalToolFailed'`.
- `UiSettings` (L898): add `terminalCommand: string; editorCommand: string;`
- `UiSettingsPatch` (L923): add `terminalCommand?: string; editorCommand?: string;`
- `IpcApi` (L1197): add
```ts
/** P49: launch the OS terminal at `path` (a repo/worktree/submodule dir). Uses
 *  the configured terminalCommand template (empty ⇒ auto-detect). Rejects
 *  AppError('externalToolFailed' | 'io'). */
openInTerminal(path: string): Promise<void>;
/** P49: reveal `path` in the OS file manager. Rejects AppError('externalToolFailed' | 'io'). */
revealInFileManager(path: string): Promise<void>;
/** P49: open `path` in the configured editor (empty ⇒ auto-detect VS Code).
 *  Rejects AppError('externalToolFailed' | 'io'). */
openInEditor(path: string): Promise<void>;
```

### 6.2 `tauri.ts` (next to `setUiSettings`, ~L648)
```ts
openInTerminal(path: string): Promise<void> { return invoke('open_in_terminal', { path }); },
revealInFileManager(path: string): Promise<void> { return invoke('reveal_in_file_manager', { path }); },
openInEditor(path: string): Promise<void> { return invoke('open_in_editor', { path }); },
```

### 6.3 Mock (must compile + simulate success AND failure)
- `mock/persistence.ts`: add `terminalCommand: ''` + `editorCommand: ''` to `DEFAULT_UI_SETTINGS`;
  add the tolerant parse (`typeof parsed.terminalCommand === 'string' ? … : default`) and include
  both in the returned object.
- `mock/handlers/session.ts` `setUiSettings`: add
  `terminalCommand: patch.terminalCommand ?? current.terminalCommand,` and the editor twin.
- **New `mock/handlers/external.ts`** exporting `externalHandlers satisfies Partial<IpcApi>` with
  the three methods. Behavior: `await delay(120)`; if `path` contains the sentinel `'#fail'`
  (or equals a fixtures constant), **throw an AppError-shaped object**
  `{ kind: 'externalToolFailed', message: 'Mock: could not launch <what>' }` exactly as other mock
  handlers reject (e.g. `requireRepo`), otherwise resolve (optionally `console.info` the intended
  program). This lets the harness drive the success path AND the error toast.
- `mock.ts`: `import { externalHandlers }` and spread `...externalHandlers` into `mockIpc`.

---

## 7. Frontend wiring

### 7.1 Settings section — `SettingsExternalToolsSection.tsx` (own file, ~120 lines)
```ts
export interface SettingsExternalToolsSectionProps {
  terminalCommand: string;
  editorCommand: string;
  /** Same debounced patch channel the other sections use. */
  onChange(patch: UiSettingsPatch): void;
}
```
Renders two labeled text inputs ("Terminal command", "Editor command"), each with a placeholder
`Leave blank to auto-detect`, a helper line *"Use {path} for the folder — it is passed as a
separate argument, never through a shell,"* and a **"Reset to auto-detect"** button that sets `''`.
Save on blur / debounced `onChange({ terminalCommand })` / `onChange({ editorCommand })`.
`SettingsPanelProps` gains `terminalCommand: string; editorCommand: string;`; `SettingsPanel`
imports + renders `<SettingsExternalToolsSection … onChange={onChange} />` (its container App
threads the two values from its `UiSettings` state, exactly like `graph`/`autoFetch`).

### 7.2 Shared menu builder — `workspaceMenus.ts`
Add three handler deps to `WorkspaceMenuDeps`:
```ts
onOpenInTerminal(path: string): void;
onRevealInFileManager(path: string): void;
onOpenInEditor(path: string): void;
```
Add a private spread-builder mirroring `commitActionItems`/`resetMenuItems`:
```ts
// P49: the shared "Open externally" items for a filesystem path. Never gated by
// mutating/opActive (they touch no git state). Spread by row menus.
function externalToolsItems(path: string): ContextMenuItem[] {
  return [
    { label: 'Open in terminal',        icon: …, disabled: false, onSelect: () => onOpenInTerminal(path) },
    { label: 'Reveal in file manager',  icon: …, disabled: false, onSelect: () => onRevealInFileManager(path) },
    { label: 'Open in editor',          icon: …, disabled: false, onSelect: () => onOpenInEditor(path) },
  ];
}
```
Spread `...externalToolsItems(sub.absPath)` into `submoduleMenuItems` and
`...externalToolsItems(wt.absPath)` into `worktreeMenuItems` (after the existing items). Export
`externalToolsItems` on the `WorkspaceMenus` interface so TabStrip/toolbar reuse the same 3 items.
New icons in `menuIcons` (e.g. `TerminalIcon`, `FolderOpenIcon`, `EditorIcon`) — small presentational additions.

### 7.3 Tab menu — `TabStrip.tsx`
Add an `onTabMenu?(repoId: string, x: number, y: number): void` prop and wire `onContextMenu`
on each tab pill (`.tab`) to call it with `e.clientX/Y` + `t.path`. RepoWorkspace opens a
`<ContextMenu>` with `externalToolsItems(tabPath)`. (The `+` menu stays the open-repo menu; the
per-tab right-click is the "tab menu".)

### 7.4 Toolbar — `WorkspaceToolbar.tsx`
Add props `repoPath: string; onOpenInTerminal(): void; onRevealInFileManager(): void;
onOpenInEditor(): void;`. Add one icon button ("Open externally", folder icon) that opens a
`<ContextMenu>` (same caret-dropdown idiom as the existing Push caret at L184) listing the three
actions on `repoPath`.

### 7.5 Container (App / RepoWorkspace)
Implement the three handlers once: `ipc.openInTerminal(path)` etc., `.catch` → `pushToast('error',
errorMessage(e))` (the existing AppError→toast path). Thread them into `WorkspaceMenuDeps`,
`WorkspaceToolbar`, and the TabStrip context menu. Success is silent (the app just appears);
optionally a subtle success toast — recommend **silent** (a window opening is its own feedback).

---

## 8. Testability

**Rust unit tests in `external.rs` (no launching):**
- `parse_template`: standalone `{path}`, embedded `--working-directory={path}`, quoted token with
  spaces (`"C:\\my repo"`), empty/whitespace ⇒ `None`, a `;`-bearing template stays one literal arg.
- Builder tables: `terminal_ladder`/`editor_ladder`/`reveal_spec` for **each `TargetOs`**, asserting
  the exact `LaunchSpec` vec (program, args, cwd, hide_console) for both empty (auto ladder) and a
  user template (single spec).
- Ladder logic via a **`FakeRunner`** recording `run` calls and returning scripted `Err`/`Ok`:
  first-candidate-fails-second-succeeds picks the second; all-fail ⇒
  `Err(AppError::ExternalToolFailed)` naming the last program.

**Command test** (`commands/tests.rs`, optional): missing path ⇒ `AppError::Io` without invoking a
runner.

**USER CHECKPOINT (native, cannot be AI-verified):** actually launching a terminal / file manager /
editor is a native round-trip — see §9 and `docs/contracts/P49-user-checklist.md`. The AI gate only
proves argv correctness + the mock-driven UI (menus, toolbar, settings, error toast).

---

## 9. Sub-increment split

### P49a — Backend + IPC + mock
Scope: `external.rs` (builders + trait + SpawnRunner + orchestration + tests); `error.rs` variant;
settings fields + `ui_settings.rs` triple + back-compat test; 3 commands + `mod.rs` +
`lib.rs` registration; `types.ts` (3 methods + 2 UiSettings fields + AppError kind); `tauri.ts`
wrappers; mock (`persistence`, `session`, new `external` handler, `mock.ts` spread).
**Acceptance:**
1. `cargo test -p bonsai-core external` green: `parse_template`, per-`TargetOs` builder tables, and
   FakeRunner ladder tests pass; settings back-compat test passes.
2. `cargo build` + `cargo clippy` clean; `generate_handler!` lists 128 commands.
3. `capabilities/default.json` unchanged (verified diff).
4. `tsc`/`pnpm build` clean; `VITE_MOCK_IPC=1` harness boots; in console
   `await ipc.openInTerminal('/x')` resolves and `await ipc.openInTerminal('/x#fail')` rejects with
   `{ kind: 'externalToolFailed' }`.
5. Terminal/editor templates round-trip through `getUiSettings`/`setUiSettings` in the mock.

### P49b — Frontend wiring
Scope: `SettingsExternalToolsSection.tsx` + `SettingsPanel` compose + App threading;
`workspaceMenus.ts` `externalToolsItems` + deps + worktree/submodule spread + `menuIcons`;
`TabStrip.tsx` per-tab menu; `WorkspaceToolbar.tsx` dropdown; container handlers + toasts.
**Acceptance:**
1. `tsc`/`pnpm build` clean; no file over the ~500-line soft limit (new section is its own file).
2. Harness screenshot: Settings shows the External tools section with both inputs + reset; editing
   an input persists (reload keeps the value from mock storage).
3. Harness: right-click a worktree row and a submodule row, right-click a tab, and the toolbar
   button each show the three items; a `#fail` sentinel path surfaces an error toast via the
   AppError→toast path; a normal path resolves silently.
4. Menus never gated by mutating/opActive (external items enabled mid-operation).

---

## 10. Open questions (flag to orchestrator)

- **OQ1 — Reveal semantics.** Recommend *open the directory* in the file manager (space-safe single
  arg; our entry points are always dirs) rather than *select-in-parent* (`explorer /select,` /
  `open -R`), which is fiddly with spaces and only meaningful for a file target. Confirm.
- **OQ2 — Editor auto-detect scope.** Recommend the VS Code family only (`code`/`code-insiders`;
  macOS `open -a "Visual Studio Code"`). No `$VISUAL`/`$EDITOR`/`git core.editor` fallback (those are
  usually terminal editors, wrong for opening a folder). If none found ⇒ clear error steering the
  user to set a template. Confirm, or widen the ladder.
- **OQ3 — Per-machine vs per-OS stored template.** Recommend a single per-machine string (D4). Only
  matters if a user syncs `settings.json` across OSes (out of scope for a local app). Confirm.
- **OQ4 — AppError variant.** Recommend adding `ExternalToolFailed`; reuse of `Other` is the lower-
  churn alternative. Confirm which.
