# P112 — External tools: detected-list picker (removes user-supplied command strings)

**Status:** spec — `ui-designer` first (§11), then `senior-dev`.
**Authorised:** user 2026-09-11 (ledger #4 end state; `TODO.md` "Roadmap: REMOVE user-supplied
`terminalCommand` / `editorCommand`").
**Sources:** `docs/audit-2026-09-03-external-launch.md` MEDIUM-2 / LOW-1 ·
`docs/audit-2026-09-11-mcp-tool-contracts.md` HIGH-1 (same class, other surface).
**Split:** the static candidate table, auto ladders and legacy aliases are in
`docs/contracts/P112-tool-catalog.md` — needed only by the `catalog.rs` implementer.

**THE INVARIANT THIS MILESTONE BUYS.** *No byte originating in the renderer, in `settings.json`, or in
a repository ever becomes `LaunchSpec.program`, nor any argv token other than the one target
directory.* Every program, flag and app name is a `&'static str` in a compile-time catalog, or an
absolute path Bonsai's own probe produced. A settings value is only ever a **lookup key** into that
catalog. Shape-validating a free-text program string cannot achieve this; deleting the field can.

---

## 0. Verification of the brief (checked against source, not taken on trust)

| Claim | Verdict |
|---|---|
| Route 1: the absolute-path branch accepts any existing file (`is_file()` only) | **TRUE** — `external_cmd.rs:145-155`. Two precisions: the value must also carry no `is_shell_syntax` char (`:82`), and `CreateProcess` must accept the image — a PE, or a `.cmd`/`.bat` (which runs via `cmd.exe`, with the `%VAR%` argv expansion `external.rs:39-44` already admits). |
| …"launches with `CREATE_NO_WINDOW` — no visible window" | **PARTLY TRUE.** `editor_ladder` ⇒ `hide_console = true` ⇒ `CREATE_NO_WINDOW` (`external.rs:147-151`), but that suppresses the console of a **console-subsystem** image only; a GUI payload still shows its own windows. Silent for a console payload, not in general. |
| Route 2: `node <repo>` executes the repo's `package.json` `main` / `index.js` | **TRUE** — `node` passes `is_bare_name_char`; editor delivery is `PathDelivery::Argument`, so the repo dir is argv[1] (`external.rs:244`). Generalises (`python <dir>` → `__main__.py`). |
| Route 3: a bare build tool as the *terminal* command, cwd = repo, zero args | **TRUE** — `PathDelivery::WorkingDir` gives `args = []`, `cwd = path` (`:245`), so `make` / `nmake` / `just` / `msbuild` execute the repo's own build file. |
| Root cause is a free-text program the renderer can write via `set_ui_settings` | **TRUE** — `ui_settings.rs:243-247` assigns verbatim; `tests_ui_settings_patch_flags.rs:226-230` pins that `"powershell -c calc"` is *stored as typed* and refused only at launch. |
| `TODO.md:808`: removal "must also retire … the LOW-1 cwd hardening" | **FALSE.** LOW-1 is about a hostile **repo** as a child's cwd on the *auto* rungs; unrelated to user commands. `safe_cwd()` **stays** (§7), and `external_url.rs:127` depends on it. |

---

## 1. Module map

### New — `crates/bonsai-core/src/tools/`

| File | Responsibility | Must not contain |
|---|---|---|
| `tools/mod.rs` | Types + DTOs, the process-lifetime cache, `tool_scan`, `refresh_tool_scan`, `picked`, `coerce_tool_id`, `legacy_tool_id`. | the candidate table; any `Command` |
| `tools/catalog.rs` | **Static data only:** `ToolEntry`/`Rung`/`Recipe`/`AutoRung` defs, `CATALOG`, `AUTO_*`, `LEGACY_ALIASES`, `find`, `entries_for`. | filesystem, registry, spawn |
| `tools/detect.rs` | `ToolEnv` + `HostToolEnv`, `probe_entry`, `scan_for`. | the table; `LaunchSpec` |
| `tools/fake.rs` (`cfg(test)`) | `FakeToolEnv` — declarative present paths / PATH hits / registry values (`external_fake.rs` precedent). | — |
| `tools/detect_tests.rs`, `tools/catalog_tests.rs` (`cfg(test)`) | per-OS ladders, bundles, coercion, migration. | — |

`lib.rs`: add `pub mod tools;`.

### Changed

| File | Change |
|---|---|
| `bonsai-core/src/external.rs` | `template_spec` + `PathDelivery` deleted; the four launch entry points take `Option<&PickedTool>` instead of `template: &str`; new private `spec_from`; auto ladders built from `tools::catalog` (§4); module doc `:19-30` replaced. |
| `bonsai-core/src/external_cmd.rs` | **Deleted** — `safe_cwd()` moves verbatim into `procutil.rs` (§7). |
| `bonsai-core/src/external_url.rs` | import only → `use crate::procutil::safe_cwd;` |
| `bonsai-core/src/gitbin.rs` | `parse_reg_query` (`:203`) widened to `pub(crate)`; no behaviour change. |
| `src-tauri/src/commands/external.rs` | `launch_inner` reads the new key and resolves it via `tools::picked` (§4). |
| `src-tauri/src/commands/tools.rs` (new) | `list_external_tools` (§6); registered in `lib.rs` `generate_handler!` beside `commands::open_in_editor` (`lib.rs:322`). |
| `src-tauri/src/settings.rs` | `terminal_tool` / `editor_tool` added; the two legacy keys become `skip_serializing` migration input (§5.3). |
| `src-tauri/src/commands/ui_settings.rs` | key rename + write-time coercion at `:57,:59,:134,:136,:243-247,:325-326`. |
| Frontend | `src/ipc/types/{common,settings,ipc-api}.ts` · `src/ipc/tauri/app.ts` · `src/ipc/mock/handlers/{external,session}.ts` · `src/ipc/mock/persistence.ts` · `src/settings/defaults.ts` · `src/settings/uiSettingsDefaults.json` · `src/hooks/useUiSettings.ts` · `src/components/settings/catalog/general.ts:56-75` · `src/components/SettingsExternalToolsSection.tsx` · `src/obs/rawArgPolicy.json` · `src/test/uiSettingsKit.ts`. Exact lines in §7. |

---

## 2. Rust types

```rust
// tools/mod.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolKind { Terminal, Editor }       // the file manager stays NON-configurable

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolSource { BuiltIn, Path, Registry, WellKnown, AppBundle }

/// What a successful probe produced. `program`/`bundle` are ALWAYS either a
/// catalog `&'static str` or an absolute path this crate built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub program: String,          // absolute path; the catalog name for BuiltIn
    pub bundle: Option<String>,   // macOS `.app` dir; Some ⇔ source == AppBundle
    pub source: ToolSource,
}

/// A catalog entry the user picked AND that resolved. Besides `None` (= auto
/// ladder) this is the ONLY thing a launch path accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedTool { pub entry: &'static ToolEntry, pub resolution: Resolution }

/// IPC DTO — display-only. The backend never accepts `label`/`detail`/`source` back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTool {
    pub id: String,      // catalog id — the only field the frontend may send back
    pub label: String,
    pub kind: ToolKind,
    pub source: ToolSource,
    /// Subtitle copy. EXACTLY: the resolved absolute path for Path / Registry /
    /// WellKnown / AppBundle; the literal `"built in"` for BuiltIn. No other value.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalToolScan {
    pub terminals: Vec<DetectedTool>,
    pub editors: Vec<DetectedTool>,
    pub scanned_at_ms: u64,       // epoch ms; the picker shows "checked HH:MM"
}

// ---- public API (BLOCKING except coerce_/legacy_, which are pure) -----------
pub fn tool_scan() -> ExternalToolScan;          // cached; runs the ladder on first call
pub fn refresh_tool_scan() -> ExternalToolScan;  // re-probe everything, replace the cache
/// `""`, an unknown id, a wrong-kind id, or a known id whose cached target no
/// longer exists ⇒ `None` (⇒ the caller's auto ladder). One fs recheck, no re-probe.
pub fn picked(setting: &str, kind: ToolKind) -> Option<PickedTool>;
/// PURE catalog membership: the id unchanged when `CATALOG` holds it for `kind`
/// (any OS — settings.json may be synced between machines), else `""`. Write side (§5.2).
pub fn coerce_tool_id(value: &str, kind: ToolKind) -> String;
/// One-shot migration of a legacy free-text command (§5.3). PURE — no probe.
pub fn legacy_tool_id(legacy: &str, kind: ToolKind) -> String;
```

```rust
// tools/catalog.rs — data in docs/contracts/P112-tool-catalog.md
#[derive(Debug, PartialEq, Eq)]
pub struct ToolEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ToolKind,
    pub os: TargetOs,
    /// The bare EXECUTABLE name (PATH / PATHEXT lookup, and `Name` auto rungs).
    pub program: &'static str,
    /// The macOS `open -a` APP NAME. `Some` for every `Recipe::MacOpen` entry and
    /// every entry referenced by a `MacApp` auto rung (AC8). NEVER conflated with
    /// `program`: the mac `vscode` entry has `program = "code"` but
    /// `app_name = Some("Visual Studio Code")`, and the auto ladder needs the latter.
    pub app_name: Option<&'static str>,
    pub rungs: &'static [Rung],
    /// Applies to EXECUTABLE resolutions; a resolution with `source == AppBundle`
    /// always launches as `MacOpen` instead.
    pub recipe: Recipe,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Rung {
    /// Present by definition on this OS ⇒ `entry.program`, touches nothing.
    /// MUST be an entry's only rung.
    BuiltIn,
    /// PATH (+PATHEXT on Windows) lookup of `entry.program` ⇒ absolute path.
    OnPath,
    /// Windows App Paths (HKCU then HKLM), default value. Key + `/ve` convention:
    /// catalog file.
    AppPaths { exe: &'static str },
    /// `%var%` + backslash-relative suffix via `win_join`, then `is_file`.
    WinFolder { var: &'static str, suffix: &'static str },
    /// macOS `.app` bundle; `home: true` ⇒ prefixed with `$HOME`. Hit ⇒ AppBundle.
    Bundle { path: &'static str, home: bool },
    /// Absolute unix candidate: `is_file` + at least one execute bit.
    UnixFile { path: &'static str },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Recipe {
    /// `<program> [fixed…] <dir>` — dir is the LAST argv token; cwd = safe_cwd().
    DirLastArg(&'static [&'static str]),
    /// `<program> [fixed…] <prefix><dir>` — prefix+dir in ONE token; cwd = safe_cwd().
    DirJoinedArg(&'static [&'static str], &'static str),
    /// `<program> [fixed…]`, NO dir token; cwd = <dir>. Shells only.
    DirCwd(&'static [&'static str]),
    /// `open -a <bundle | app_name> <dir>`; cwd = safe_cwd(); wait_for_exit = true.
    MacOpen,
}

pub struct AutoRung { pub id: &'static str, pub via: AutoVia }
/// `Name` ⇒ `entry.program` + `entry.recipe`; `MacApp` ⇒ `open -a entry.app_name`.
pub enum AutoVia { Name, MacApp }

/// Host-OS entries first, then any-OS (so an id synced from another OS still
/// resolves for `coerce_tool_id`).
pub fn find(kind: ToolKind, id: &str) -> Option<&'static ToolEntry>;
pub fn entries_for(kind: ToolKind, os: TargetOs) -> impl Iterator<Item = &'static ToolEntry>;
```

Derived, never stored: `hide_console = (kind == Editor)` — a terminal window must be visible, an
editor's console flash must not be; `wait_for_exit = true` iff the built spec is an `open` spec
(`open`'s exit code is the only "app not found" signal — the `LaunchSpec::wait_for_exit` rule).

---

## 3. Detection

```rust
// tools/detect.rs — mirrors gitbin::GitEnv: same injection seam, same "no run" rule (D4)
pub trait ToolEnv {
    fn var(&self, key: &str) -> Option<String>;
    fn is_file(&self, p: &Path) -> bool;
    /// Directory AND a real bundle: `p.is_dir() && p.join("Contents/Info.plist").is_file()`.
    fn is_bundle(&self, p: &Path) -> bool;
    fn is_executable(&self, p: &Path) -> bool;         // Windows: true for any file
    fn resolve_on_path(&self, program: &str) -> Option<PathBuf>;
    /// `value == ""` ⇒ the key's DEFAULT value (`reg query <key> /ve`, parsed with
    /// `parse_reg_query(stdout, "(Default)")`).
    fn registry_string(&self, key: &str, value: &str) -> Option<String>;
    fn home(&self) -> Option<PathBuf>;
}
/// Real env/fs. `registry_string` and `resolve_on_path` DELEGATE to
/// `gitbin::HostGitEnv` — reuse `parse_reg_query` and the absolute-`reg.exe`
/// hardening; do not fork them.
pub struct HostToolEnv;
pub fn scan_for(env: &dyn ToolEnv, os: TargetOs) -> Vec<(&'static ToolEntry, Resolution)>;
```

```
probe_entry(env, entry) -> Option<Resolution>:      # first hit wins, rungs in order
  BuiltIn           -> Some{ program: entry.program, bundle: None, source: BuiltIn }
  OnPath            -> env.resolve_on_path(entry.program) => Some{ .., source: Path }
  AppPaths{exe}     -> for root in ["HKCU", "HKLM"]:
                         cand = trim_quotes(env.registry_string(root + APP_PATHS + exe, "")?)
                         if env.is_file(cand) -> Some{ .., source: Registry }
  WinFolder{v,sfx}  -> cand = win_join(env.var(v)?, sfx)
                       if env.is_file(cand) -> Some{ .., source: WellKnown }
  Bundle{path,home} -> cand = if home { env.home()?.join(path) } else { path }
                       if env.is_bundle(cand)
                         -> Some{ program: "open", bundle: Some(cand), source: AppBundle }
  UnixFile{path}    -> if env.is_file(path) && env.is_executable(path)
                         -> Some{ .., source: WellKnown }

scan_for(env, os) = CATALOG entries with entry.os == os, in table order,
                    filter_map(|e| probe_entry(env, e).map(|r| (e, r)))
```

**Every rung degrades to `None` on any failure** (missing var, absent key, unparseable registry
output, failed stat). A wrong candidate therefore *fails to detect*; it can never mis-detect, because
a hit is always an existence-checked absolute path. **Nothing in detection executes a candidate** —
`ToolEnv` has no "run" method.

### macOS `.app` bundles (brief item 2)

* A bundle is a **directory**, which the old `is_file()` model rejected. `is_bundle` also requires
  `Contents/Info.plist`, distinguishing a real bundle from a directory merely named `*.app`.
* **Launch form: `open -a <absolute bundle path> <dir>`** (`Recipe::MacOpen`, `wait_for_exit = true`
  so a missing app still falls through the ladder). Both argv tokens are ours: the bundle path came
  from our probe, the dir is the one target — no injection surface.
* The CLI stub (`Contents/SharedSupport/bin/code`) is deliberately unused: it exists for only some
  apps, and `open -a` needs no per-app knowledge. A `PATH`-installed stub is still detected — it is
  the same entry's `OnPath` rung (`source: Path`, normal `DirLastArg` recipe).
* Auto-path bundles keep today's **app-name** form (`open -a "Visual Studio Code"`) via
  `AutoVia::MacApp` + `entry.app_name`.

### Cache, refresh, and the launch-time recheck

```rust
static SCAN: RwLock<Option<CachedScan>> = RwLock::new(None);
struct CachedScan { at_ms: u64, found: Vec<(&'static ToolEntry, Resolution)> }
```

* `RwLock` (not `OnceLock`) and poison-recovering (`unwrap_or_else(|p| p.into_inner())`) — the
  `gitbin::GIT_BIN` precedent, same reason: "install the editor, press Rescan" must work without
  restarting.
* **Lazy and explicit.** Nothing scans at boot. The only trigger is `list_external_tools`, called
  when the Settings *External tools* section mounts. A Windows scan can spawn `reg.exe` a handful of
  times, so it runs under `spawn_blocking`, then is cached for the process lifetime. `refresh: true`
  ⇒ `refresh_tool_scan()`. No timer, no focus rescan — detection is not repo state.
* **Launch never re-runs the ladder** (a `Registry` rung spawns a process). `picked()` reads the cache
  (populating it if empty) and does **one** recheck, by source:
  `BuiltIn` ⇒ **no recheck** (present by definition — an `is_file("cmd")` test would be false and
  would silently break picking `cmd`/`powershell`); `Path` / `Registry` / `WellKnown` ⇒ `is_file`;
  `AppBundle` ⇒ `is_bundle`. Miss ⇒ `None` ⇒ auto ladder.

---

## 4. Launch

```rust
// external.rs — replaces template_spec
fn spec_from(entry: &ToolEntry, res: &Resolution, path: &Path) -> LaunchSpec
```

```
hide_console = (entry.kind == Editor)
if res.source == AppBundle:                       # a bundle overrides entry.recipe
    return open_spec(["-a", res.bundle, path], safe_cwd(), hide_console)   # waits
match entry.recipe:
  DirLastArg(fixed)       -> spec(res.program, fixed ++ [path],      safe_cwd(), hide_console, false)
  DirJoinedArg(fixed, pf) -> spec(res.program, fixed ++ [pf + path], safe_cwd(), hide_console, false)
  DirCwd(fixed)           -> spec(res.program, fixed,                path,       hide_console, false)
  MacOpen                 -> open_spec(["-a", entry.app_name.expect("AC8"), path],
                                       safe_cwd(), hide_console)
```

```rust
pub fn terminal_ladder(os: TargetOs, picked: Option<&PickedTool>, path: &Path) -> Vec<LaunchSpec>;
pub fn editor_ladder(os: TargetOs, picked: Option<&PickedTool>, path: &Path) -> Vec<LaunchSpec>;
pub fn open_in_terminal(runner: &dyn CommandRunner, os: TargetOs,
                        picked: Option<&PickedTool>, path: &Path) -> Result<(), AppError>;
pub fn open_in_editor(runner: &dyn CommandRunner, os: TargetOs,
                      picked: Option<&PickedTool>, path: &Path) -> Result<(), AppError>;
```

```
*_ladder(os, picked, path):
  if let Some(p) = picked: return vec![spec_from(p.entry, &p.resolution, path)]
  for rung in AUTO_LIST[os][kind]:                 # ids only; strings come from the entry
      e = catalog::find(kind, rung.id).unwrap()    # totality pinned by AC8
      push(match rung.via {
        Name   => spec_from(e, &Resolution{ program: e.program, bundle: None, source: Path }, path),
        MacApp => open_spec(["-a", e.app_name.expect("AC8"), path], safe_cwd(), kind == Editor),
      })
```

The **auto path never probes**: `launch_first`'s spawn-fail fall-through is the detector, exactly as
today, and the resulting argv is byte-identical to the current hardcoded ladders (AC9).
`open_in_terminal` / `open_in_editor` no longer validate anything — **there is nothing left to
validate**: a `PickedTool` can only be built by `tools::picked` out of a catalog entry.
`reveal_in_file_manager`, `reveal_spec`, `launch_first`, `LaunchSpec`, `CommandRunner`, `SpawnRunner`,
`TargetOs` are unchanged.

Command layer (`src-tauri/src/commands/external.rs`, inside the existing `spawn_blocking`):

```rust
Action::Terminal => {
    let sel = settings_file.map(|f| settings::load_from(&f).terminal_tool).unwrap_or_default();
    let picked = tools::picked(&sel, ToolKind::Terminal);
    external::open_in_terminal(&runner, os, picked.as_ref(), p)
}
```

---

## 5. Settings

### 5.1 Shape

| Wire key | Rust field | Type | Default | Meaning |
|---|---|---|---|---|
| `terminalTool` | `terminal_tool` | `String` | `""` | `""` ⇒ auto ladder; else a **catalog id** |
| `editorTool` | `editor_tool` | `String` | `""` | same, editors |

`String` with `""` = auto rather than `Option<String>`: it reuses the existing `Option<String>` patch
field, the `resetKey(…, 'auto-detect')` descriptor and the mock persistence shape verbatim.
`SETTINGS_VERSION` stays **1** — an additive field plus a removal with a safe default is below the bump
bar `settings.rs:58-71` documents.

### 5.2 Write-time coercion — the "renderer writes garbage ⇒ selects nothing" property

```rust
// apply_patch
if let Some(v) = patch.terminal_tool { s.terminal_tool = tools::coerce_tool_id(&v, ToolKind::Terminal); }
if let Some(v) = patch.editor_tool   { s.editor_tool   = tools::coerce_tool_id(&v, ToolKind::Editor);   }
```

* An id **not in the catalog** is stored as `""`. Pure `&'static` lookup, zero IO — so it cannot
  reproduce the failure that ruled out save-time validation (`external_cmd.rs:16-23`: the settings
  writer merges pending keys into one patch and re-queues on failure, so a *rejection* would wedge
  every later settings write). Coercion never rejects and never errors.
* An id **in the catalog but not currently detected** is **kept** — a legitimate stale selection (tool
  temporarily uninstalled, settings synced from another machine). The picker marks it "not installed"
  (§11); launch falls back to the auto ladder (OQ1).
* Launch-side `catalog::find` is defence in depth: a value hand-written into `settings.json` still
  cannot name a program.

### 5.3 Migration of `terminalCommand` / `editorCommand`

The legacy fields survive on `Settings` **only** as migration input:

```rust
/// LEGACY (P49 … 2026-09-11): read for the one-shot P112 migration and NEVER
/// written again. `skip_serializing` is what makes the migration idempotent.
#[serde(default, skip_serializing)] pub terminal_command: String,
#[serde(default, skip_serializing)] pub editor_command: String,
```

`load_from`, immediately after parsing:

```
migrate_external_tools(&mut s):
  if s.terminal_tool.is_empty() && !s.terminal_command.trim().is_empty():
      s.terminal_tool = tools::legacy_tool_id(&s.terminal_command, Terminal)
  if s.editor_tool.is_empty() && !s.editor_command.trim().is_empty():
      s.editor_tool = tools::legacy_tool_id(&s.editor_command, Editor)
  s.terminal_command.clear(); s.editor_command.clear()      # never carried further

legacy_tool_id(legacy, kind) -> String:        # PURE: no fs, no probe, no spawn
  s = legacy up to the first '{'               # drops "{path}" and everything after it
  s = first whitespace-delimited token of s.trim()
  s = file stem of s                           # strips directories and .exe/.cmd/.bat
  LEGACY_ALIASES.get(&(kind, s.to_lowercase())).copied().unwrap_or("").to_string()
```

**Migration invariants (each is a test, AC7):**
1. The output is a catalog id or `""` — **never** a program string, never a path. A stored string can
   therefore never become an executed program.
2. Misses are **accepted, not preserved**: `"C:\Tools\payload.exe"` → `""`, `"make"` → `""`,
   `"node"` → `""`, `"/opt/custom/bin/myed"` → `""`. `"powershell -c calc"` → `"powershell"` is
   correct: the user asked for PowerShell and the `-c calc` tail is dropped — which is the point.
3. An absolute path **with spaces** normalises to its first token and so usually misses ⇒ `""`.
   Accepted: `""` means auto-detect, which works.
4. A pre-existing non-empty new key always wins over the legacy key.
5. Idempotent: after one save the legacy keys are gone from the file; a second load is a no-op.

---

## 6. IPC surface

### Command — request/response (a scan is small and one-shot: no event, no channel)

```rust
// src-tauri/src/commands/tools.rs
/// P112: detected terminals + editors for the Settings pickers. NEVER rejects for
/// detection state — an empty scan is `{ terminals: [], editors: [], … }` (the
/// `check_git_availability` precedent). `refresh: true` re-probes and replaces the
/// process cache (the picker's Rescan). Git-state-free: no `repo_path`, no managed
/// state, no `opActive` gating.
#[tauri::command]
pub async fn list_external_tools(refresh: bool) -> Result<ExternalToolScan, AppError> // spawn_blocking
```

Registered in `src-tauri/src/lib.rs` `generate_handler!` beside `commands::open_in_editor` (`:322`).
The only possible rejection is a task-join error.

### TypeScript

```ts
// src/ipc/types/common.ts — mirrors the Rust DTOs
export type ExternalToolKind = 'terminal' | 'editor';
export type ExternalToolSource = 'builtIn' | 'path' | 'registry' | 'wellKnown' | 'appBundle';
export interface DetectedTool {
  /** Opaque catalog id — the ONLY value the frontend may send back. */
  id: string;
  label: string;
  kind: ExternalToolKind;
  source: ExternalToolSource;
  /** Display-only subtitle: the resolved absolute path, or 'built in'. */
  detail: string;
}
export interface ExternalToolScan {
  terminals: DetectedTool[];
  editors: DetectedTool[];
  scannedAtMs: number;
}

// src/ipc/types/ipc-api.ts — IpcApi member, beside openInEditor
/** P112: detected terminals + editors for the Settings pickers. Never rejects for
 *  detection state; an empty list is a normal result. `refresh` re-probes. */
listExternalTools(refresh: boolean): Promise<ExternalToolScan>;

// src/ipc/types/settings.ts — REPLACES terminalCommand/editorCommand (:105-108, :180-182)
/** P112: catalog id of the picked terminal; '' ⇒ per-OS auto-detect. An id the
 *  backend does not recognise is coerced to '' on write. */
terminalTool: string;
editorTool: string;
// patch: terminalTool?: string; editorTool?: string;
```

`openInTerminal` / `revealInFileManager` / `openInEditor` / `openUrl` signatures are **unchanged**.

### Mock IPC (`VITE_MOCK_IPC=1`) — mandatory

`src/ipc/mock/handlers/external.ts` gains `listExternalTools`, driven by the `query()` seam
(`src/ipc/mock/repoState.ts:159`) so every picker state is reachable in the browser harness:

| Seam | Scan |
|---|---|
| *(none)* | Windows-flavoured: terminals `windows-terminal` (path), `powershell` (builtIn), `cmd` (builtIn); editors `vscode` (wellKnown, detail `C:\Program Files\Microsoft VS Code\Code.exe`), `sublime` (path), `notepadpp` (registry) |
| `?tools=none` | `{ terminals: [], editors: [], scannedAtMs }` — nothing-detected empty state |
| `?tools=mac` | bundle fixture: terminals `apple-terminal`, `iterm2`; editors `vscode`, `zed` — all `source: 'appBundle'`, detail `/Applications/….app` |
| `?tools=stale` | default fixture **plus** persisted `editorTool: 'zed'`, absent from the list ⇒ "picked tool not installed" |
| `?tools=slow` | resolves after `delay(1200)` ⇒ the scanning/busy state |

`refresh: true` bumps `scannedAtMs` and re-`delay`s, so Rescan is verifiable.
`src/ipc/mock/persistence.ts` and `handlers/session.ts` swap the two string keys for the new ones
(same `typeof === 'string'` guard, default `''`). The mock deliberately does **not** model
`coerce_tool_id` — Rust owns that rule (the `validate_web_url` precedent); the coerced-to-`''` outcome
is asserted in Rust unit tests.

### Observability

`src/obs/rawArgPolicy.json`: add `"listExternalTools": ["refresh"]` (a bool is safe to log). The
`openInTerminal` / `revealInFileManager` / `openInEditor` rows are unchanged.

---

## 7. Deletions and keeps — exhaustive, so no dead code survives

### Deleted

| Item | Location |
|---|---|
| `validate_command_setting`, `MAX_LEN`, `is_bare_name_char`, `is_shell_syntax`, `is_disallowed_char`, `is_unc`, `refuse` | `bonsai-core/src/external_cmd.rs` — **whole file**, see keeps |
| all 18 `external_cmd_tests.rs` tests **except** `safe_cwd_is_an_existing_directory_that_is_not_the_repo` (`:194`, moves with `safe_cwd`) | `bonsai-core/src/external_cmd_tests.rs` |
| `pub mod external_cmd;` | `bonsai-core/src/lib.rs:7` |
| `template_spec`, `PathDelivery` (**both** variants, incl. the `WorkingDir` configured-program rung), every `template: &str` parameter | `bonsai-core/src/external.rs:211-257, 269, 318, 370-399` |
| module doc §"The user-configured program (audit … MEDIUM-2)" (`:19-30`) | replaced by a §"Where the program comes from" stating the §0 invariant |
| `template_spec_*` (`:26-71`, `:140-150`), `terminal_ladder_template_overrides_to_single_spec` (`:153`), `editor_ladder_template_overrides_to_single_spec` (`:256`), `editor_template_is_the_only_candidate_tried` (`:306`), `configured_program_specs_never_wait_for_exit` (`:248`) | `bonsai-core/src/external_tests.rs` — rewritten against `PickedTool` |
| `terminal_command` / `editor_command` on `UiSettings`, `UiSettingsPatch`, `apply_patch`, the mapper | `src-tauri/src/commands/ui_settings.rs:57,59,134,136,243-247,325-326` |
| the `"powershell -c calc"` stored-as-typed test | `src-tauri/src/commands/tests_ui_settings_patch_flags.rs:158-230` — rewritten: a garbage id patches to `""`, a catalog id round-trips, the two keys patch independently |
| `terminalCommand` / `editorCommand` | `src/ipc/types/settings.ts:105-108,180-182` · `src/settings/defaults.ts:94-95` · `src/settings/uiSettingsDefaults.json:38-39` · `src/hooks/useUiSettings.ts:92-93,201-202,393-394,438-439,474-475` · `src/test/uiSettingsKit.ts:65-66` · `src/ipc/mock/persistence.ts:387-394,426-427` · `src/ipc/mock/handlers/session.ts:123-124` · `src/components/settings/catalog/general.ts:56-75` · `src/settings/defaults.test.ts:69-70` · `src/components/settings/settingsCatalog.test.ts:85-86,124-125` · `src/components/settings/settingsCatalogRows.test.ts:53-54` |
| the two free-text rows + the "Enter a command name…" note | `src/components/SettingsExternalToolsSection.tsx` — rewritten per §11; its cases in `src/components/SettingsSections.test.tsx` ("External tools (program edits + reset)") rewritten as picker cases |

**Three breakages with non-obvious causes — fix, do not delete:**
* `src/components/settings/SettingsPrimitives.test.tsx:20` uses `general.terminal-command` as its
  canonical **text** row (`TEXT_ROW`); no text row survives in *General* → repoint at
  `git-config.user-email`.
* `src-tauri/src/settings_tests.rs:172,188` round-trips `editor_command` as a **concurrency-test
  payload**; with `skip_serializing` it no longer persists → retarget that thread at another plain
  string field.
* `src-tauri/src/settings_ai_tests.rs:214-250` (pre-P49 legacy-key test) → becomes the §5.3 migration
  test.

**`e2e/` was grepped 2026-09-11 for `terminal-command` / `editor-command` / `terminalCommand` /
`editorCommand` / "Terminal command" / "Editor command": zero hits.** No Playwright selector churn is
expected; if the picker gets an e2e spec it is new, not a migration.

### Kept, and why

| Kept | Why |
|---|---|
| `safe_cwd()` — **moved verbatim** to `bonsai-core/src/procutil.rs` | Audit LOW-1 is about a hostile **repo** as a child's cwd on the *auto* rungs; nothing to do with user commands. `TODO.md:808` says otherwise and is wrong (§0). `external_url.rs:127` depends on it. Moving it lets `external_cmd.rs` be deleted outright instead of surviving as a one-function stub. |
| the auto ladders | Nothing-detected is a real state: a fresh machine, an unlisted Linux terminal, a stale picked id. Byte-identical to today. |
| `spec`, `open_spec`, `reveal_spec`, `launch_first`, `LaunchSpec` (+ its cwd-is-a-directory invariant), `CommandRunner`, `SpawnRunner`, `TargetOs`, the `wait_for_exit` semantics | Unchanged mechanism; `MacOpen` depends on `wait_for_exit`. |
| `external_url.rs` in full, incl. `validate_web_url` | Different surface (URLs), untouched here. |
| `launch_inner`'s `is_dir()` precheck + category-only error (MEDIUM-1 / LOW-2) | Still the only thing keeping a non-directory out of `current_dir`. |
| `procutil::resolve_program`, `gitbin::parse_reg_query` | Now also serve the `OnPath` and `AppPaths` rungs. |

---

## 8. Residual after P112 — stated honestly

1. **`launch_inner` still accepts any existing directory the renderer names.**
   `commands/external.rs:74-93` takes `path: String` from the frontend and checks only `is_dir()`;
   nothing ties it to an open repository. **P112 does NOT close this.** It is a different defect — an
   *authorization* gap on the target, not an *execution* gap on the program — of the same class as
   `docs/audit-2026-09-11-mcp-tool-contracts.md` HIGH-1 (`bonsai_stage` accepts arbitrary paths the UI
   never offers). Closing it needs a containment policy over the open-repo workdirs plus their
   worktrees and `contained_abs_path`-validated submodule paths, applied at every caller
   (`src/components/workspaceMenusRows.ts`, `src/hooks/useExternalTools.ts`): its own increment, filed
   as a follow-up.
   **What P112 does change:** the primitive degrades from "the renderer picks the program *and* the
   directory" to "the renderer picks only the directory; the program is one of the user's own detected
   tools". A shell opened at a hostile cwd runs the user's profile, not repo files; an editor handed a
   hostile folder may still execute repo-authored config subject to that editor's own trust model
   (e.g. VS Code Workspace Trust).
2. **Capability loss, deliberate.** A portable or unlisted editor is now **unpickable**, and there is
   deliberately **no "browse to an executable" button** — a stored absolute path is exactly the class
   being removed, and the user rejected the native-confirmation route. The remedy for a missing tool
   is a catalog addition, i.e. a code change (OQ2).
3. **Pre-existing, unchanged:** `open` / `xdg-open` / `explorer` are still bare names resolved through
   `PATH`; a poisoned `PATH` could hijack them. `gitbin` uses an absolute `reg.exe` for exactly this
   reason (OQ3).
4. **Windows `.cmd`/`.bat` shims** still receive the directory with `%VAR%` expansion
   (`external.rs:39-44`) — now only for catalog-detected shims such as `code.cmd`.

---

## 9. Open questions — recommendation given; orchestrator confirms with the user

* **OQ1 — a picked tool that is no longer installed.** (a) silently fall back to the auto ladder;
  (b) fail with "Sublime Text is no longer installed — choose another in Settings".
  **Recommend (a)**, matching today's "empty ⇒ auto-detect", with the picker showing the stale
  selection so the state is visible somewhere. (b) is what some users expect — user's call.
* **OQ2 — unlisted tools.** Recommend accepting the loss for v1: the ledger records both legacy values
  as **empty** in the user's real `settings.json`, so nothing in the current install regresses.
  Revisit on a concrete report.
* **OQ3 — absolute `/usr/bin/open` and `%SystemRoot%\System32\explorer.exe`.** Cheap, but it changes
  `reveal_spec` / `open_url` argv that existing tests pin. Recommend **not** in P112; file as a
  follow-up so this contract stays one concern.
* **OQ4 — `git-bash --cd=<dir>`** is the one catalog flag not verifiable from this repo's source. Keep
  the row; if the native checkpoint shows it does not start in the directory, **drop the row** rather
  than guessing another flag (failure mode: a terminal in the wrong directory, not a security issue).
* **OQ5 — `detail` (an absolute local path) crosses IPC** for the picker subtitle. Display-only, never
  accepted back. Recommend keeping it — it distinguishes two same-label installs. Can be reduced to a
  coarse source label if the user prefers.

---

## 10. Acceptance criteria

### AI gate (orchestrator-verifiable; no native window)

**AC1** `cargo nextest` green; `cargo clippy -D warnings`, `tsc`, `eslint`, `pnpm build` clean.
**AC2** Detection tests run **all three OS ladders on one machine** via `scan_for(&FakeToolEnv, os)`
with an explicit `TargetOs` (the `external.rs` house pattern): `OnPath`, `AppPaths`, `WinFolder`,
`UnixFile` (+ execute bit), `Bundle`, and rung **order** (an earlier rung wins).
**AC3** Every rung degrades to `None` under a failing env — absent var, absent key, unparseable
registry output, false `is_file`/`is_bundle`. A "nothing exists" `ToolEnv` yields an empty scan, and
`ToolEnv` exposes no way to execute a candidate.
**AC4** Bundles: `/Applications/X.app` with `Contents/Info.plist` ⇒ `source: AppBundle`; the same
directory **without** `Info.plist` ⇒ not detected; the spec is `open -a <bundle> <dir>`,
`wait_for_exit == true`, bundle as **one** token (`args.len() == 3`), incl. a bundle path with spaces.
**AC5 — hostile-selection table.** For each of `"C:\hostile\payload.exe"`, `"/tmp/payload"`, `"node"`,
`"make"`, `"nmake"`, `"just"`, `"msbuild"`, `"python"`, `"powershell -c calc"`, `"code {path}"`,
`"../../evil"`, `"\\server\share\x.exe"`, a 2000-char string, and a string containing U+202E:
`coerce_tool_id` ⇒ `""` **and** `picked(..)` ⇒ `None`. The audit's three routes are gone by
construction, not by validation.
**AC6 — invariant test.** For every catalog entry and every `Resolution` a fake probe can produce,
`LaunchSpec.program` is `entry.program`, `"open"`, or the probe-produced absolute path; every argv
token is a catalog `&'static str`, a catalog prefix + the target dir, or the target dir. No test may
build a `LaunchSpec` from a free-text program — `template_spec` is gone, so no constructor exists.
**AC7** Migration: one case per §5.3 invariant 1-5, plus `"code {path}"`→`vscode`,
`"wt -d {path}"`→`windows-terminal`, `"cmd /K"`→`cmd`, `"C:\Program Files\X\x.exe"`→`""`,
`"make"`→`""`; both-keys-present keeps the new one; after save→load→save the legacy keys are absent
from the JSON text; a file with neither loads `""`/`""`.
**AC8** Catalog integrity: ids unique per `(kind, os)`; every `AUTO_*` id resolves via
`catalog::find`; **every `MacOpen` entry and every `MacApp`-referenced entry has `Some(app_name)`**;
`BuiltIn` is an entry's only rung; every `LEGACY_ALIASES` target exists in `CATALOG`; `MacOpen`
entries are `os == MacOs`.
**AC9** The **auto ladders are unchanged**: the pre-existing `external_tests.rs` ladder assertions
(`wt -d` / `powershell` / `cmd /K`; `open -a Terminal`; `gnome-terminal --working-directory=` /
`konsole --workdir` / `x-terminal-emulator`; `code` / `code-insiders`; the macOS editor trio
`open -a "Visual Studio Code"` / `open -a "Visual Studio Code - Insiders"` / `code`) pass with no edit
other than the new parameter, and `only_the_directory_less_terminal_rungs_keep_the_repo_as_cwd` holds.
**AC10** `list_external_tools` returns both lists in one round trip, never rejects for detection
state, runs under `spawn_blocking`; `refresh: true` re-probes (assert `scannedAtMs` advances).
**AC11** `picked()` recheck by source: a `BuiltIn` selection (`cmd`, `powershell`) resolves **without**
any fs test; a `Path`/`WellKnown` selection whose file was removed ⇒ `None`; an `AppBundle` selection
whose bundle was removed ⇒ `None`.
**AC12** Mock parity: `VITE_MOCK_IPC=1` serves all five §6 seams; the harness shows the populated
picker, the nothing-detected empty state, the stale-selection state and the busy state. One screenshot
of populated + empty as the final visual proof (frugal-verification rule).
**AC13** Frontend tests: picking a tool patches `{ editorTool: '<id>' }` and nothing else; the row `↺`
is **absent** at `''` and patches `''` when a tool is picked; `useUiSettings` round-trips both new
keys; no `terminalCommand`/`editorCommand` identifier remains in `src/`, `src-tauri/` or `e2e/`
outside the §5.3 legacy fields and the alias table (grep is the evidence).
**AC14** An `#[ignore]`d host test (`cargo test -- --ignored`): `HostToolEnv` on this Windows box
detects at least `cmd` and `powershell` (both `BuiltIn`, so it cannot fail on a supported Windows), and
any `Path`/`Registry`/`WellKnown` hit is an **absolute existing** file. Real detection, no native
window.

### USER CHECKPOINT — must NOT be self-confirmed

**UC1** In `pnpm tauri dev` → Settings → External tools, the pickers list the tools actually installed.
Only the native run exercises the real registry / well-known / `PATH` rungs against a real install set;
the harness serves fixtures.
**UC2** Picking a terminal and a non-VS-Code editor, then using "Open in terminal" / "Open in editor"
from the repo, worktree and submodule menus, launches **that** tool at **that** folder.
**UC3** Rescan reflects reality: install or remove a tool, press Rescan, the list changes.
**UC4** With `editorTool` set to a tool then uninstalled, "Open in editor" still opens something
(OQ1 (a)) and the picker marks the selection "not installed".
**UC5** macOS: a `.app`-sourced editor and terminal both launch at the right folder. **May be WAIVED
for lack of a Mac** — say so explicitly rather than implying it passed; the `TargetOs::MacOs` branch and
bundle detection are unit-covered regardless (AC4).
**Not a checkpoint item — migration.** The ledger records both legacy values as **empty** in the user's
real `settings.json`, so a native run cannot demonstrate migration. AC7 is the whole proof.

---

## 11. `ui-designer` prerequisite (workflow step 2b — before implementation)

The settings rows change from free-text inputs to a selection control, so `ui-designer` writes
`docs/contracts/P112-ui.md` (and updates `ui-reference.md` if a new state pattern appears) **first**.
Inputs: this file, `src/components/SettingsExternalToolsSection.tsx`,
`src/components/settings/catalog/general.ts:56-75`, `src/components/settings/types.ts:44-52`.
Must cover:

* Rows `general.terminal-tool` / `general.editor-tool`; **reuse the existing
  `SettingsControlKind: 'combobox'`** (`types.ts:47-49`) — do not invent a control. `"Auto-detect"` is
  the `''` option and the reset target (`resetKey(…, 'auto-detect')`).
* Option row shape: `label` + the `detail` subtitle, and whether `source` is surfaced at all.
* All states: auto-detect (default) · populated · **nothing detected** · picked-but-not-installed ·
  scanning/busy · the Rescan affordance and its result copy (`scannedAtMs`).
* Replacement copy for the deleted "Enter a command name … no arguments" note: the honest new line is
  that Bonsai launches only tools it detected itself, plus what to do when a tool is missing (OQ2).
* Keyboard/a11y for the combobox and Rescan; both themes; the `general.*` search keywords.
