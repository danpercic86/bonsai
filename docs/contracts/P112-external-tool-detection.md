# P112 — External tools: detected-list picker + user-chosen Browse (removes free-text commands)

**Status:** spec — `ui-designer` first (§11), then `senior-dev`.
**Authorised:** user 2026-09-11 (ledger #4 end state; `TODO.md` "Roadmap: REMOVE user-supplied
`terminalCommand` / `editorCommand`"). **OQ1 and OQ2 RULED 2026-09-11 — see §9.**
**Sources:** `docs/audit-2026-09-03-external-launch.md` MEDIUM-2 / LOW-1 ·
`docs/audit-2026-09-11-mcp-tool-contracts.md` HIGH-1 (same class, other surface) ·
`P91-observability.md` F4 (the backend-invoked-dialog pattern, `commands/obs.rs:388-400`).
**Split:** the static candidate table, auto ladders and legacy aliases are in
`docs/contracts/P112-tool-catalog.md` — needed only by the `catalog.rs` implementer.

**THE INVARIANT THIS MILESTONE BUYS.** *No byte originating in the renderer, in `settings.json`, or in
a repository ever becomes `LaunchSpec.program`, nor any argv token other than the one target
directory.* Every program, flag and app name is a `&'static str` in a compile-time catalog, an
absolute path Bonsai's own probe produced, or **a path a human chose in a native dialog that the
backend itself opened** (§5.4). A settings value is only ever a **lookup key**. Shape-validating a
free-text program string cannot achieve this; deleting the field can.

> **Line numbers in this contract were taken against the 2026-09-11 working tree and some have
> already drifted** (`external_cmd.rs` grew during the in-flight fix round: `validate_command_setting`
> is now `:176-206`, `is_shell_syntax` `:121`, `safe_cwd` `:233`). **Resolve every citation by symbol
> name first**, line number second.

---

## 0. Verification of the brief (checked against source, not taken on trust)

| Claim | Verdict |
|---|---|
| Route 1: the absolute-path branch accepts any existing file (`is_file()` only) | **TRUE** — `validate_command_setting`, absolute branch (`external_cmd.rs:190-200`). Two precisions: the value must also carry no `is_shell_syntax` char, and `CreateProcess` must accept the image — a PE, or a `.cmd`/`.bat` (which runs via `cmd.exe`, with the `%VAR%` argv expansion `external.rs` already admits). |
| …"launches with `CREATE_NO_WINDOW` — no visible window" | **PARTLY TRUE.** `editor_ladder` ⇒ `hide_console = true` ⇒ `CREATE_NO_WINDOW`, but that suppresses the console of a **console-subsystem** image only; a GUI payload still shows its own windows. Silent for a console payload, not in general. |
| Route 2: `node <repo>` executes the repo's `package.json` `main` / `index.js` | **TRUE** — `node` passes `is_bare_name_char`; editor delivery is `PathDelivery::Argument`, so the repo dir is argv[1]. Generalises (`python <dir>` → `__main__.py`). |
| Route 3: a bare build tool as the *terminal* command, cwd = repo, zero args | **TRUE** — `PathDelivery::WorkingDir` gives `args = []`, `cwd = path`, so `make` / `nmake` / `just` / `msbuild` execute the repo's own build file. |
| Root cause is a free-text program the renderer can write via `set_ui_settings` | **TRUE** — `ui_settings.rs:243-247` assigns verbatim; `tests_ui_settings_patch_flags.rs:226-230` pins that `"powershell -c calc"` is *stored as typed* and refused only at launch. |
| `TODO.md:808`: removal "must also retire … the LOW-1 cwd hardening" | **FALSE.** LOW-1 is about a hostile **repo** as a child's cwd on the *auto* rungs; unrelated to user commands. `safe_cwd()` **stays** (§7), and `external_url.rs` depends on it. |

---

## 1. Module map

### New — `crates/bonsai-core/src/tools/`

| File | Responsibility | Must not contain |
|---|---|---|
| `tools/mod.rs` | Types + DTOs, the cache, `tool_scan`, `refresh_tool_scan`, `picked`, `coerce_tool_id`, `legacy_tool_id`, `validate_custom_program`, `synthesize_recipe`. | the candidate table; any `Command`; any dialog |
| `tools/catalog.rs` | **Static data only:** `ToolEntry`/`Rung`/`Recipe`/`AutoRung` defs, `CATALOG`, `AUTO_*`, `LEGACY_ALIASES`, `find`, `entries_for`. | filesystem, registry, spawn |
| `tools/detect.rs` | `ToolEnv` + `HostToolEnv`, `probe_entry`, `scan_for`. | the table; `LaunchSpec` |
| `tools/fake.rs` (`cfg(test)`) | `FakeToolEnv` — declarative present paths / PATH hits / registry values (`external_fake.rs` precedent). | — |
| `tools/detect_tests.rs`, `tools/catalog_tests.rs`, `tools/custom_tests.rs` (`cfg(test)`) | per-OS ladders, bundles, coercion, migration, custom-tool validation. | — |

`lib.rs`: add `pub mod tools;`.

### Changed

| File | Change |
|---|---|
| `bonsai-core/src/external.rs` | `template_spec` + `PathDelivery` deleted; the four launch entry points take `Option<&PickedTool>`; new private `spec_from`; auto ladders built from `tools::catalog` (§4); the MEDIUM-2 module-doc section replaced. |
| `bonsai-core/src/external_cmd.rs` | **Deleted** — `safe_cwd()` moves to `procutil.rs`, four helpers move to `tools/mod.rs` (§7). |
| `bonsai-core/src/external_url.rs` | import only → `use crate::procutil::safe_cwd;` |
| `bonsai-core/src/gitbin.rs` | `parse_reg_query` → `pub(crate)`; `HostGitEnv::registry_string` gains the `value == "" ⇒ /ve` branch (§3). No behaviour change for existing callers. |
| `src-tauri/src/commands/external.rs` | `launch_inner` reads the new key and resolves it via `tools::picked` (§4). |
| `src-tauri/src/commands/tools.rs` (new) | `list_external_tools`, `add_custom_tool`, `remove_custom_tool` (§6); registered in `lib.rs` `generate_handler!` beside `commands::open_in_editor` (`lib.rs:322`). |
| `src-tauri/src/settings.rs` | `terminal_tool` / `editor_tool` / `custom_tools` added; the two legacy keys become `skip_serializing` migration input (§5.3). |
| `src-tauri/src/commands/ui_settings.rs` | key rename + write-time coercion (`:57,:59,:134,:136,:243-247,:325-326`). **`custom_tools` is deliberately NOT added to `UiSettings` or `UiSettingsPatch`** (§5.4). |
| Frontend | `src/ipc/types/{common,settings,ipc-api}.ts` · `src/ipc/tauri/app.ts` · `src/ipc/mock/handlers/{external,session}.ts` · `src/ipc/mock/persistence.ts` · `src/settings/defaults.ts` · `src/settings/uiSettingsDefaults.json` · `src/hooks/useUiSettings.ts` · `src/components/settings/catalog/general.ts:56-75` · `src/components/SettingsExternalToolsSection.tsx` · `src/obs/rawArgPolicy.json` · `src/test/uiSettingsKit.ts`. Exact lines in §7. |

No `tauri.conf.json` / capability change: `tauri-plugin-dialog = "2"` is already a dependency
(`src-tauri/Cargo.toml:21`) and initialised (`lib.rs:48`); a **Rust-side** `DialogExt` call is not
gated by a capability, so `dialog:allow-open` stays the renderer's only dialog reach.

---

## 2. Rust types

```rust
// tools/mod.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolKind { Terminal, Editor }       // the file manager stays NON-configurable

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolSource { BuiltIn, Path, Registry, WellKnown, AppBundle, UserChosen }

/// What a successful probe produced. `program`/`bundle` are ALWAYS a catalog
/// `&'static str`, an absolute path this crate built, or a stored user-chosen path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub program: String,          // absolute path; the catalog name for BuiltIn
    pub bundle: Option<String>,   // macOS `.app` dir; Some ⇔ AppBundle (or a UserChosen bundle)
    pub source: ToolSource,
}

/// A resolved selection — the ONLY thing a launch path accepts besides `None`
/// (= auto ladder). Deliberately NOT `&'static ToolEntry`: a user-chosen tool has
/// no catalog entry, and folding both into one shape is what stops the two launch
/// paths from drifting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedTool {
    pub kind: ToolKind,
    /// The catalog entry's recipe, `MacOpen` for any bundle resolution, or the
    /// synthesized recipe for a user-chosen tool (§5.4).
    pub recipe: Recipe,
    /// `LaunchSpec.program`: an absolute path, the catalog name for `BuiltIn`,
    /// or `"open"` when `recipe == MacOpen`.
    pub program: String,
    /// The `open -a` argument. `Some` **iff** `recipe == MacOpen`: the resolved
    /// bundle PATH for a bundle/user-chosen bundle, else `entry.app_name`.
    pub open_arg: Option<String>,
    pub source: ToolSource,
}

/// IPC DTO — display-only. The backend never accepts `label`/`detail`/`source` back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTool {
    pub id: String,      // catalog id or `custom:<n>` — the only field sent back
    pub label: String,
    pub kind: ToolKind,
    pub source: ToolSource,
    /// Subtitle copy. EXACTLY: the resolved absolute path for Path / Registry /
    /// WellKnown / AppBundle / UserChosen; the literal `"built in"` for BuiltIn.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalToolScan {
    pub terminals: Vec<DetectedTool>,   // detected, then user-chosen, in insertion order
    pub editors: Vec<DetectedTool>,
    pub scanned_at_ms: u64,             // epoch ms; the picker shows "checked HH:MM"
}

/// One user-chosen tool (§5.4). Lives in `settings::Settings`, NOT in `UiSettings`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomTool {
    pub id: String,        // "custom:<n>", monotonic per settings file, never reused
    pub kind: ToolKind,
    pub label: String,     // BACKEND-derived from the file stem / bundle name
    pub program: String,   // absolute path, or a macOS `.app` bundle dir
}

// ---- public API (BLOCKING except the pure fns, which are marked) ------------
pub fn tool_scan(custom: &[CustomTool]) -> ExternalToolScan;          // cached probe + custom rows
pub fn refresh_tool_scan(custom: &[CustomTool]) -> ExternalToolScan;  // re-probe, replace the cache
/// `""`, an unknown id, a wrong-kind id, or a known id whose target no longer
/// exists ⇒ `None` (⇒ auto ladder). One fs recheck, no re-probe.
pub fn picked(setting: &str, kind: ToolKind, custom: &[CustomTool]) -> Option<PickedTool>;
/// PURE. The id unchanged when `CATALOG` holds it for `kind` (any OS — settings
/// may be synced between machines) OR when `custom` holds it for `kind`; else `""`.
pub fn coerce_tool_id(value: &str, kind: ToolKind, custom: &[CustomTool]) -> String;
/// PURE. One-shot migration of a legacy free-text command (§5.3). No probe.
pub fn legacy_tool_id(legacy: &str, kind: ToolKind) -> String;
/// The §5.4 rule set for a dialog-returned path. Touches the fs (existence,
/// bundle, exec bit) but never spawns. Category-only errors.
pub fn validate_custom_program(path: &Path) -> Result<CustomKindShape, AppError>;
pub enum CustomKindShape { Executable, MacBundle }
/// PURE. `(kind, shape)` ⇒ the recipe a user-chosen tool launches with (§5.4).
pub fn synthesize_recipe(kind: ToolKind, shape: CustomKindShape) -> Recipe;
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
    /// every entry a `MacApp` auto rung references (AC8). NEVER conflated with
    /// `program`: the mac `vscode` entry has `program = "code"` but
    /// `app_name = Some("Visual Studio Code")`, and the auto ladder needs the latter.
    pub app_name: Option<&'static str>,
    pub rungs: &'static [Rung],
    /// For EXECUTABLE resolutions. Any bundle resolution becomes `MacOpen` in
    /// `picked()` instead.
    pub recipe: Recipe,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Rung {
    /// Present by definition on this OS ⇒ `entry.program`, touches nothing.
    /// MUST be an entry's only rung.
    BuiltIn,
    /// PATH (+PATHEXT on Windows) lookup of `entry.program` ⇒ absolute path.
    OnPath,
    /// Windows App Paths (HKCU then HKLM), default value; `/ve` convention in §3.
    AppPaths { exe: &'static str },
    /// `%var%` + backslash-relative suffix via `win_join`, then `is_file`.
    WinFolder { var: &'static str, suffix: &'static str },
    /// macOS `.app` bundle; `home: true` ⇒ prefixed with `$HOME`. Hit ⇒ AppBundle.
    Bundle { path: &'static str, home: bool },
    /// Absolute unix candidate: `is_file` + at least one execute bit.
    UnixFile { path: &'static str },
}

/// `Copy` — `PickedTool` carries one by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipe {
    /// `<program> [fixed…] <dir>` — dir is the LAST argv token; cwd = safe_cwd().
    DirLastArg(&'static [&'static str]),
    /// `<program> [fixed…] <prefix><dir>` — prefix+dir in ONE token; cwd = safe_cwd().
    DirJoinedArg(&'static [&'static str], &'static str),
    /// `<program> [fixed…]`, NO dir token; cwd = <dir>. Shells only.
    DirCwd(&'static [&'static str]),
    /// `open -a <open_arg> <dir>`; cwd = safe_cwd(); wait_for_exit = true.
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
(`open`'s exit code is the only "app not found" signal).

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
    /// `parse_reg_query(stdout, "(Default)")`). `None` on ANY failure.
    fn registry_string(&self, key: &str, value: &str) -> Option<String>;
    fn home(&self) -> Option<PathBuf>;
}
pub struct HostToolEnv;      // real env/fs
pub fn scan_for(env: &dyn ToolEnv, os: TargetOs) -> Vec<(&'static ToolEntry, Resolution)>;
```

**Registry delegation — two constraints, verified 2026-09-11.** `HostToolEnv::registry_string`
delegates to `gitbin::HostGitEnv` (absolute `reg.exe`, `CREATE_NO_WINDOW`, `parse_reg_query`) and adds
the `/ve` branch there; `resolve_on_path` delegates to the same place. **Do NOT delegate to
`crate::winenv::HostWinEnv`**: its `REG_BUDGET` is 1.5 s *shared across at most three spawns*
(`winenv.rs:150-155`), sized for PATH rehydration, and a dozen `AppPaths` probes would exhaust a
budget another feature depends on. Give the *scan* its own wall-clock budget instead (1.5 s across all
registry rungs of one scan; on exhaustion every remaining `AppPaths` rung yields `None`, which is safe
— degradation only means "not offered").

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
output, failed stat, exhausted budget). A wrong candidate therefore *fails to detect*; it can never
mis-detect, because a hit is always an existence-checked absolute path. **Nothing in detection
executes a candidate** — `ToolEnv` has no "run" method.

### macOS `.app` bundles (brief item 2)

* A bundle is a **directory**, which the old `is_file()` model rejected. `is_bundle` also requires
  `Contents/Info.plist`, distinguishing a real bundle from a directory merely named `*.app`.
* **Launch form: `open -a <absolute bundle path> <dir>`** (`Recipe::MacOpen`, `wait_for_exit = true`,
  so a missing app still falls through the ladder). Both argv tokens are ours: the bundle path came
  from our probe (or a native dialog), the dir is the one target — no injection surface.
* The CLI stub (`Contents/SharedSupport/bin/code`) is deliberately unused: it exists for only some
  apps, and `open -a` needs no per-app knowledge. A `PATH`-installed stub is still detected — the same
  entry's `OnPath` rung (`source: Path`, normal `DirLastArg`).
* Auto-path bundles keep today's **app-name** form (`open -a "Visual Studio Code"`) via
  `AutoVia::MacApp` + `entry.app_name`.

### Cache, refresh, and the launch-time recheck

```rust
static SCAN: RwLock<Option<CachedScan>> = RwLock::new(None);
struct CachedScan { at_ms: u64, found: Vec<(&'static ToolEntry, Resolution)> }
```

* `RwLock` (not `OnceLock`) and poison-recovering (`unwrap_or_else(|p| p.into_inner())`) — the
  `gitbin::GIT_BIN` precedent, same reason: "install the editor, press Rescan" must work without
  restarting. Only *detected* entries are cached; user-chosen rows come from settings each call.
* **Lazy and explicit.** Nothing scans at boot. The only trigger is `list_external_tools`, called when
  the Settings *External tools* section mounts; it runs under `spawn_blocking` and is then cached for
  the process lifetime. `refresh: true` ⇒ `refresh_tool_scan()`. No timer, no focus rescan —
  detection is not repo state.
* **Launch never re-runs the ladder** (a `Registry` rung spawns a process). `picked()` reads the cache
  (populating it if empty) and does **one** recheck, by source:
  `BuiltIn` ⇒ **no recheck** (present by definition — an `is_file("cmd")` test is false and would
  silently break picking `cmd`/`powershell`); `Path` / `Registry` / `WellKnown` ⇒ `is_file`;
  `AppBundle` ⇒ `is_bundle`; `UserChosen` ⇒ `is_file` **or** `is_bundle` per its stored shape.
  Miss ⇒ `None` ⇒ auto ladder (OQ1, ruled).
* `picked()` also normalises: for any bundle resolution it returns
  `recipe: MacOpen, program: "open", open_arg: Some(bundle)`, so `spec_from` needs no bundle branch.

---

## 4. Launch

```rust
// external.rs — replaces template_spec
fn spec_from(picked: &PickedTool, path: &Path) -> LaunchSpec
```

```
hide_console = (picked.kind == Editor)
match picked.recipe:
  MacOpen                 -> open_spec(["-a", picked.open_arg.expect("AC8"), path],
                                       safe_cwd(), hide_console)                   # waits
  DirLastArg(fixed)       -> spec(program, fixed ++ [path],      safe_cwd(), hide_console, false)
  DirJoinedArg(fixed, pf) -> spec(program, fixed ++ [pf + path], safe_cwd(), hide_console, false)
  DirCwd(fixed)           -> spec(program, fixed,                path,       hide_console, false)
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
  if let Some(p) = picked: return vec![spec_from(p, path)]
  for rung in AUTO_LIST[os][kind]:                 # ids only; strings come from the entry
      e = catalog::find(kind, rung.id).unwrap()    # totality pinned by AC8
      # source = Path for BOTH arms: the auto path NEVER probed, so `Path` is the
      # "unverified name" bucket. It must NOT be AppBundle — `open -a "Visual Studio
      # Code"` is an app-NAME launch with no probed bundle, and a later refactor that
      # reused this value would `is_bundle("Visual Studio Code")` and always miss.
      push(spec_from(&match rung.via {
        AutoVia::Name   => PickedTool{ kind, recipe: e.recipe, program: e.program.into(),
                                       open_arg: None, source: Path },
        AutoVia::MacApp => PickedTool{ kind, recipe: MacOpen, program: "open".into(),
                                       open_arg: Some(e.app_name.expect("AC8").into()),
                                       source: Path },
      }, path))
```

One spec builder for every source, so the picked and auto paths cannot drift. The **auto path never
probes**: `launch_first`'s spawn-fail fall-through is the detector, exactly as today, and the argv is
byte-identical to the current hardcoded ladders (AC9). `open_in_terminal` / `open_in_editor` no longer
validate anything — **there is nothing left to validate**: a `PickedTool` can only be built by
`tools::picked` or by the auto arm above. `reveal_in_file_manager`, `reveal_spec`, `launch_first`,
`LaunchSpec`, `CommandRunner`, `SpawnRunner`, `TargetOs` are unchanged.

Command layer (`src-tauri/src/commands/external.rs`, inside the existing `spawn_blocking`):

```rust
Action::Terminal => {
    let s = settings_file.map(|f| settings::load_from(&f)).unwrap_or_default();
    let picked = tools::picked(&s.terminal_tool, ToolKind::Terminal, &s.custom_tools);
    external::open_in_terminal(&runner, os, picked.as_ref(), p)
}
```

---

## 5. Settings

### 5.1 Shape

| Wire key | Rust field | Type | Default | Meaning |
|---|---|---|---|---|
| `terminalTool` | `terminal_tool` | `String` | `""` | `""` ⇒ auto ladder; else a catalog id or `custom:<n>` |
| `editorTool` | `editor_tool` | `String` | `""` | same, editors |
| `customTools` | `custom_tools` | `Vec<CustomTool>` | `[]` | §5.4 — **read-only over IPC; not patchable** |

`String` with `""` = auto rather than `Option<String>`: it reuses the existing `Option<String>` patch
field, the `resetKey(…, 'auto-detect')` descriptor and the mock persistence shape verbatim.
`SETTINGS_VERSION` stays **1** — additive fields plus a removal with a safe default are below the bump
bar `settings.rs:58-71` documents.

### 5.2 Write-time coercion — the "renderer writes garbage ⇒ selects nothing" property

```rust
// apply_patch
if let Some(v) = patch.terminal_tool {
    s.terminal_tool = tools::coerce_tool_id(&v, ToolKind::Terminal, &s.custom_tools);
}
if let Some(v) = patch.editor_tool {
    s.editor_tool = tools::coerce_tool_id(&v, ToolKind::Editor, &s.custom_tools);
}
```

* An id that is neither a catalog id nor an existing `custom_tools` id is stored as `""`. Pure lookup,
  zero IO — so it cannot reproduce the failure that ruled out save-time validation (`external_cmd.rs`
  module doc: the settings writer merges pending keys into one patch and re-queues on failure, so a
  *rejection* would wedge every later settings write). Coercion never errors.
* An id **in the catalog but not currently detected** is **kept** — a legitimate stale selection. The
  picker marks it "not installed"; launch falls back to the auto ladder (OQ1, ruled).
* Launch-side resolution is defence in depth: a value hand-written into `settings.json` still cannot
  name a program.

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
1. The output is a catalog id or `""` — **never** a program string, never a path, and **never a
   `custom_tools` record**. A stored string cannot become an executed program, and migration cannot
   manufacture the human dialog click §5.4 requires.
2. Misses are **accepted, not preserved**: `"C:\Tools\payload.exe"` → `""`, `"make"` → `""`,
   `"node"` → `""`, `"/opt/custom/bin/myed"` → `""`. `"powershell -c calc"` → `"powershell"` is
   correct: the user asked for PowerShell and the `-c calc` tail is dropped — which is the point.
3. An absolute path **with spaces** normalises to its first token and so usually misses ⇒ `""`. The
   user re-adds a portable editor with one Browse click (§5.4) — exactly the case OQ2's ruling
   restores.
4. A pre-existing non-empty new key always wins over the legacy key.
5. Idempotent: after one save the legacy keys are gone from the file; a second load is a no-op.

### 5.4 User-chosen tools — Browse… (**OQ2 RULED: a native, backend-invoked dialog**)

**Why this exists.** When the user ruled on MEDIUM-2 they chose shape validation *because* an
allowlist "breaks a legitimate absolute path to a portable editor". A detected-list-only design breaks
that case harder, so the ruling takes the third path: keep the free-text field deleted **and** give
back the capability through a channel the renderer cannot drive.

**The mechanism, and the exact security property.**

1. `custom_tools` lives on `settings::Settings` and is **absent from both `UiSettings` and
   `UiSettingsPatch`**. `set_ui_settings` therefore has **no field that can carry a path** — a
   renderer-written path is not "rejected by a validator", it is **unrepresentable in the patch
   type**. (`UiSettingsPatch` has no `deny_unknown_fields`, so an extra `"customTools"` key in patch
   JSON is silently ignored; AC15 pins that, so a later refactor that "helpfully" adds the field is
   caught.)
2. The **only** writer is `add_custom_tool(kind)`, which opens the dialog **itself**, in Rust, via
   `tauri_plugin_dialog::DialogExt`. The renderer supplies only `kind` (an enum). The chosen path
   never crosses IPC inbound — it is produced inside the backend and stored there.
3. **How a browsed selection is distinguished from a renderer-written one: it isn't, because there is
   no channel through which a renderer-written one can arrive.** The distinction is type-level, not a
   runtime heuristic — which is the whole point, since a runtime "was this really a dialog?" flag
   would itself be renderer-forgeable. This is the F4 rule already forced on export destinations
   (`commands/obs.rs:388-400`: "the path must come from a Tauri dialog invoked by the BACKEND, whose
   result the webview never chooses").
4. Corollary — **do not** implement this as "the frontend calls `@tauri-apps/plugin-dialog` and sends
   us the path". That is the existing `openRepo` shape (`src/ipc/tauri/repo.ts:27`) and it is
   **exactly the pattern that collapses the property**: the dialog becomes advisory and the value
   arriving at the backend is a renderer-supplied string again. A repo path is data; a program path is
   code.
5. Selection stays id-based: `editorTool = "custom:3"`. Ids are `custom:<n>` with a reserved `custom:`
   prefix no catalog id may contain (AC8). Guessing an id buys nothing — it selects a program the user
   already chose; it cannot create one.

**Validation that still applies to a browsed path** (in `validate_custom_program`, called by
`add_custom_tool`). Validating *here* is safe: this is a dedicated command, so a refusal is an
ordinary command error and cannot wedge the settings writer the way patch-time rejection would.

| Rule | Reason |
|---|---|
| absolute | the dialog returns absolute; assert it anyway |
| `is_file()` **or** macOS `is_bundle()` ⇒ `CustomKindShape` | the `.app` case is precisely what `is_file()`-only got wrong |
| unix: at least one execute bit (file form) | a non-executable pick otherwise fails at spawn with an opaque error |
| reject UNC / `\\?\` / `//host/share` | house rule (`is_unc`, `git::submodule_abs_path`): a remote share is not a local tool |
| reject control + bidi characters in the path **and** in the derived label | the one genuine spoofing surface here: a bidi override in a filename makes the picker row read as something it is not |
| length ≤ 512 (`MAX_LEN`) | a longer string is not an install path |
| ≤ 8 records per kind | bounds settings growth; over-cap is an error, never a silent drop |
| same `program` + `kind` ⇒ return the existing record | no duplicate rows |

`label` is derived by the **backend** (file stem, or bundle name minus `.app`, control/bidi stripped,
truncated); it is never renderer-supplied. Dialog **filters** (Windows `exe;cmd;bat`; macOS `app` plus
all files; Linux all files) are UX only — the user can always switch to "All files", so the table
above is what holds.

**Stated honestly: this is not a safety check on the program.** A browsed path is an arbitrary
executable. The property is "a human chose it in a native dialog the backend opened", not "it is
harmless". Nothing here decides whether the chosen binary is trustworthy, and nothing should pretend
to.

**Launch delivery.** A record carries no recipe, so `synthesize_recipe(kind, shape)`:

| kind / shape | recipe | note |
|---|---|---|
| Editor, `Executable` | `DirLastArg(&[])` | folder as one argv token, `hide_console = true`, cwd = `safe_cwd()` |
| Editor, `MacBundle` | `MacOpen` | `open -a <bundle> <dir>`, waits |
| Terminal, `Executable` | `DirCwd(&[])` | no args, cwd = the target — the only mechanism "open a terminal here" has for an unknown program |
| Terminal, `MacBundle` | `MacOpen` | `open -a <bundle> <dir>`, waits |

The `DirCwd(&[])` row **is** audit route 3's shape (`make` with cwd = the repo and no arguments). It
returns **only** for a program a human browsed to and deliberately selected as their terminal. A
renderer cannot create such a record, a repository cannot, and migration cannot (invariant 1). What
remains is a user able to misconfigure their own terminal — accepted, and documented rather than
hidden.

---

## 6. IPC surface

### Commands — request/response (a scan is small and one-shot: no event, no channel)

```rust
// src-tauri/src/commands/tools.rs
/// P112: detected + user-chosen tools for the Settings pickers. NEVER rejects for
/// detection state — an empty scan is `{ terminals: [], editors: [], … }` (the
/// `check_git_availability` precedent). `refresh: true` re-probes and replaces the
/// process cache (the picker's Rescan). Git-state-free.
#[tauri::command]
pub async fn list_external_tools(app: tauri::AppHandle, refresh: bool)
    -> Result<ExternalToolScan, AppError>;                                  // spawn_blocking

/// P112 §5.4: open a NATIVE file dialog (backend-invoked) and register the chosen
/// program as a user-chosen tool. `Ok(None)` = the user cancelled — not an error.
/// The renderer supplies ONLY `kind`; it cannot supply a path, and there is no
/// other writer of `custom_tools`.
#[tauri::command]
pub async fn add_custom_tool(app: tauri::AppHandle, kind: ToolKind)
    -> Result<Option<DetectedTool>, AppError>;

/// P112 §5.4: forget a user-chosen tool. Also clears `terminalTool`/`editorTool`
/// when the removed id was the selected one. Unknown id ⇒ `Ok(())` (idempotent).
#[tauri::command]
pub async fn remove_custom_tool(app: tauri::AppHandle, id: String) -> Result<(), AppError>;
```

`add_custom_tool` shape: build the dialog with `app.dialog().file()` + `add_filter(..)`, take the
result through the **async** `pick_file(callback)` bridged by a `oneshot` (the blocking variant must
not run on the async runtime, and the dialog must be on the main thread on some platforms), then do
validation + the settings write in `spawn_blocking`. Rejects `externalToolFailed` (category-only, per
the `refuse` idiom) for a path that fails §5.4, and `io` for a settings-write failure.

**Both mutators MUST go through `settings::update` (the `SETTINGS_IO` mutex, `settings.rs:26-34`),
never a bare `load_from` + `save_to`.** `add_custom_tool` / `remove_custom_tool` are load→mutate→save
cycles exactly like `apply_patch`; outside the mutex, a Remove racing a pane-width drag-save would
lose one of the two writes to the last rename — the precise failure that mutex was introduced for.

All three are registered in `lib.rs` `generate_handler!`. Existing signatures
(`openInTerminal`/`revealInFileManager`/`openInEditor`/`openUrl`) are **unchanged**.

### TypeScript

```ts
// src/ipc/types/common.ts — mirrors the Rust DTOs
export type ExternalToolKind = 'terminal' | 'editor';
export type ExternalToolSource =
  | 'builtIn' | 'path' | 'registry' | 'wellKnown' | 'appBundle' | 'userChosen';
export interface DetectedTool {
  /** Opaque id — a catalog id or `custom:<n>`. The ONLY value sent back. */
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

// src/ipc/types/ipc-api.ts — IpcApi members, beside openInEditor
/** P112: detected + user-chosen tools. Never rejects for detection state. */
listExternalTools(refresh: boolean): Promise<ExternalToolScan>;
/** P112 §5.4: opens a NATIVE dialog in the BACKEND. Resolves `null` on cancel.
 *  The path never crosses IPC inbound — there is no way to pass one. */
addCustomTool(kind: ExternalToolKind): Promise<DetectedTool | null>;
/** P112 §5.4: forget a user-chosen tool; clears the selection if it was picked. */
removeCustomTool(id: string): Promise<void>;

// src/ipc/types/settings.ts — REPLACES terminalCommand/editorCommand (:105-108, :180-182)
/** P112: id of the picked terminal; '' ⇒ per-OS auto-detect. An id the backend
 *  does not recognise is coerced to '' on write. There is NO settings key that
 *  carries a program path — user-chosen tools are added via addCustomTool(). */
terminalTool: string;
editorTool: string;
// patch: terminalTool?: string; editorTool?: string;   // and NOTHING path-shaped
```

`AppHandle` is not an IPC argument, so the TS members take only the declared params.

### Mock IPC (`VITE_MOCK_IPC=1`) — mandatory

`src/ipc/mock/handlers/external.ts` gains all three, driven by the `query()` seam
(`src/ipc/mock/repoState.ts:159`):

| Seam | Behaviour |
|---|---|
| *(none)* | terminals `windows-terminal` (path), `powershell` (builtIn), `cmd` (builtIn); editors `vscode` (wellKnown, detail `C:\Program Files\Microsoft VS Code\Code.exe`), `sublime` (path), `notepadpp` (registry) |
| `?tools=none` | `{ terminals: [], editors: [], scannedAtMs }` — nothing-detected empty state |
| `?tools=mac` | bundle fixture: `apple-terminal`, `iterm2`; editors `vscode`, `zed` — all `source: 'appBundle'`, detail `/Applications/….app` |
| `?tools=stale` | default fixture **plus** persisted `editorTool: 'zed'`, absent from the list ⇒ "picked tool not installed" |
| `?tools=slow` | resolves after `delay(1200)` ⇒ the scanning/busy state |
| `?tools=custom` | the default fixture plus one `source: 'userChosen'` row (`custom:1`, detail `D:\Tools\npp\notepad++.exe`) ⇒ the removable-row state |
| `?tools=cancel` | `addCustomTool` resolves `null` ⇒ the cancel path |

`addCustomTool` otherwise appends a canned `userChosen` row after `delay(200)`; `removeCustomTool`
drops it and clears the selection if it pointed there; `refresh: true` bumps `scannedAtMs`.
There is **no dialog in the harness and the mock is not a security seam** — Rust owns both the dialog
and `validate_custom_program` (the `validate_web_url` precedent). `persistence.ts` and
`handlers/session.ts` swap the two string keys for the new ones (same `typeof === 'string'` guard,
default `''`).

### Observability

`src/obs/rawArgPolicy.json`: add `"listExternalTools": ["refresh"]`, `"addCustomTool": ["kind"]`,
`"removeCustomTool": ["id"]` — all three args are safe to log (a bool, an enum, an opaque id; **no
path is ever an argument**, which is itself the property worth logging). `AppHandle` is **not** an arg
and must not appear in a policy row. Existing rows unchanged.

---

## 7. Deletions and keeps — exhaustive, so no dead code survives

### Deleted

| Item | Location |
|---|---|
| `validate_command_setting`, `is_bare_name_char`, `is_shell_syntax` — the **program-string grammar**, and only it | `bonsai-core/src/external_cmd.rs` (the file itself goes; see Kept for the four helpers that move) |
| `pub mod external_cmd;` | `bonsai-core/src/lib.rs:7` |
| `external_cmd_tests.rs` cases for the deleted grammar (bare-name allow-list, shell metachars, embedded arguments, the absolute-`is_file` branch) | `bonsai-core/src/external_cmd_tests.rs` — the UNC / control-char / bidi / over-long / `safe_cwd` cases **migrate** (see Kept) |
| `template_spec`, `PathDelivery` (**both** variants, incl. the `WorkingDir` configured-program rung), every `template: &str` parameter | `bonsai-core/src/external.rs` (was `:211-257, 269, 318, 370-399`) |
| the MEDIUM-2 module-doc section | `external.rs` — replaced by a §"Where the program comes from" stating the §0 invariant + §5.4 |
| `template_spec_*`, `terminal_ladder_template_overrides_to_single_spec`, `editor_ladder_template_overrides_to_single_spec`, `editor_template_is_the_only_candidate_tried`, `configured_program_specs_never_wait_for_exit` | `bonsai-core/src/external_tests.rs` — rewritten against `PickedTool` |
| `terminal_command` / `editor_command` on `UiSettings`, `UiSettingsPatch`, `apply_patch`, the mapper | `src-tauri/src/commands/ui_settings.rs:57,59,134,136,243-247,325-326` |
| the `"powershell -c calc"` stored-as-typed test | `src-tauri/src/commands/tests_ui_settings_patch_flags.rs:158-230` — rewritten: a garbage id patches to `""`, a catalog id round-trips, the keys patch independently |
| `terminalCommand` / `editorCommand` | `src/ipc/types/settings.ts:105-108,180-182` · `src/settings/defaults.ts:94-95` · `src/settings/uiSettingsDefaults.json:38-39` · `src/hooks/useUiSettings.ts:92-93,201-202,393-394,438-439,474-475` · `src/test/uiSettingsKit.ts:65-66` · `src/ipc/mock/persistence.ts:387-394,426-427` · `src/ipc/mock/handlers/session.ts:123-124` · `src/components/settings/catalog/general.ts:56-75` · `src/settings/defaults.test.ts:69-70` · `src/components/settings/settingsCatalog.test.ts:85-86,124-125` · `src/components/settings/settingsCatalogRows.test.ts:53-54` |
| the two free-text rows + the "Enter a command name…" note | `src/components/SettingsExternalToolsSection.tsx` — rewritten per §11; its cases in `src/components/SettingsSections.test.tsx` ("External tools (program edits + reset)") rewritten as picker + Browse cases |

**Three breakages with non-obvious causes — fix, do not delete:**
* `src/components/settings/SettingsPrimitives.test.tsx:20` uses `general.terminal-command` as its
  canonical **text** row (`TEXT_ROW`); no text row survives in *General* → repoint at
  `git-config.user-email`.
* `src-tauri/src/settings_tests.rs:172,188` round-trips `editor_command` as a **concurrency-test
  payload**; with `skip_serializing` it no longer persists → retarget that thread at another plain
  string field.
* `src-tauri/src/settings_ai_tests.rs:214-250` (pre-P49 legacy-key test) → becomes the §5.3 migration
  test.

**`e2e/` was grepped 2026-09-11** for `terminal-command` / `editor-command` / `terminalCommand` /
`editorCommand` / "Terminal command" / "Editor command": **zero hits.** No Playwright selector churn
is expected; a picker spec would be new, not a migration.

### Kept — including four helpers that MOVE rather than die

| Kept | Destination / why |
|---|---|
| **`MAX_LEN`, `is_unc`, `is_disallowed_char` (control + bidi), `refuse`** | **Move to `tools/mod.rs`** — `validate_custom_program` needs all four (§5.4), and `refuse`'s category-only, never-echo-the-value error style is the house rule for this whole surface. Their `external_cmd_tests.rs` cases move with them. Deleting these would be the one misread that costs re-implementation. |
| `safe_cwd()` — **moved verbatim** to `bonsai-core/src/procutil.rs`. **Move the CURRENT version:** its fallback changed from `"."` to `std::env::temp_dir()` in the 2026-09-11 increment (`external_cmd.rs:228-238`) precisely because `"."` under `pnpm tauri dev` IS the repo root — do not resurrect the old body. | Audit LOW-1 is about a hostile **repo** as a child's cwd on the *auto* rungs; nothing to do with user commands. `TODO.md:808` says otherwise and is wrong (§0). `external_url.rs` depends on it. Moving it lets `external_cmd.rs` be deleted outright instead of surviving as a one-function stub. |
| the auto ladders | Nothing-detected is a real state: a fresh machine, an unlisted Linux terminal, a stale picked id. Byte-identical to today. |
| `spec`, `open_spec`, `reveal_spec`, `launch_first`, `LaunchSpec` (+ its cwd-is-a-directory invariant), `CommandRunner`, `SpawnRunner`, `TargetOs`, the `wait_for_exit` semantics | Unchanged mechanism; `MacOpen` depends on `wait_for_exit`. |
| `external_url.rs` in full, incl. `validate_web_url` | Different surface (URLs), untouched here. |
| `launch_inner`'s `is_dir()` precheck + category-only error (MEDIUM-1 / LOW-2) | Still the only thing keeping a non-directory out of `current_dir`. |
| `procutil::resolve_program`, `gitbin::parse_reg_query`, `winenv` untouched | Now also serve the `OnPath` and `AppPaths` rungs; `winenv`'s budget stays its own (§3). |

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
   directory" to "the renderer picks only the directory; the program is one the user detected or
   chose". A shell opened at a hostile cwd runs the user's profile, not repo files; an editor handed a
   hostile folder may still execute repo-authored config subject to that editor's own trust model
   (e.g. VS Code Workspace Trust).
2. **The Browse escape hatch is arbitrary code — by design, and only by human act.** §5.4 restores
   "point Bonsai at my portable editor" without restoring free-text entry: the path is produced by a
   backend-opened native dialog, `custom_tools` has no patch field, and migration can never
   manufacture a record. Route 1 stays closed because a compromised renderer cannot register the
   hostile `payload.exe` a repo shipped; the *user* would have to browse to it deliberately. The
   residual is user self-harm (including the `DirCwd` terminal shape of route 3), not renderer
   escalation.
3. **Pre-existing, unchanged:** `open` / `xdg-open` / `explorer` are still bare names resolved through
   `PATH`; a poisoned `PATH` could hijack them. `gitbin` uses an absolute `reg.exe` for exactly this
   reason (OQ3). The renderer keeps `dialog:allow-open` for `openRepo` — a repo path is data, not
   code, so that is out of scope here.
4. **Windows `.cmd`/`.bat` shims** still receive the directory with `%VAR%` expansion — now only for
   catalog-detected shims (`code.cmd`) or a shim the user browsed to.

---

## 9. Rulings and remaining open questions

* **OQ1 — RULED 2026-09-11 (user): silent fallback.** A selected-but-uninstalled tool is **kept** in
  settings, marked "not installed" in the picker, and launch quietly uses the auto ladder. **No error
  toast.** Specced throughout (§3 recheck, §5.2 coercion, §11 state list).
* **OQ2 — RULED 2026-09-11 (user): add a native Browse button.** Full spec in §5.4; "portable editors
  are an accepted loss" is **no longer the answer** and has been removed from §8.
* **OQ3 — still open.** Absolute `/usr/bin/open` and `%SystemRoot%\System32\explorer.exe`. Cheap, but
  it changes `reveal_spec` / `open_url` argv that existing tests pin. Recommend **not** in P112; file
  as a follow-up so this contract stays one concern.
* **OQ4 — still open.** `git-bash --cd=<dir>` is the one catalog flag not verifiable from this repo's
  source. Keep the row; if the native checkpoint shows it does not start in the directory, **drop the
  row** rather than guessing another flag (failure mode: a terminal in the wrong directory, not a
  security issue).
* **OQ5 — still open.** `detail` (an absolute local path) crosses IPC outbound for the picker
  subtitle. Display-only, never accepted back. Recommend keeping it — it distinguishes two same-label
  installs, and for a user-chosen tool it is the only way to see *what* was picked.

---

## 10. Acceptance criteria

### AI gate (orchestrator-verifiable; no native window)

**AC1** `cargo nextest` green; `cargo clippy -D warnings`, `tsc`, `eslint`, `pnpm build` clean.
**AC2** Detection tests run **all three OS ladders on one machine** via `scan_for(&FakeToolEnv, os)`
with an explicit `TargetOs`: `OnPath`, `AppPaths`, `WinFolder`, `UnixFile` (+ exec bit), `Bundle`, and
rung **order** (an earlier rung wins).
**AC3** Every rung degrades to `None` under a failing env — absent var, absent key, unparseable
registry output, false `is_file`/`is_bundle`, exhausted scan budget. A "nothing exists" `ToolEnv`
yields an empty scan, and `ToolEnv` exposes no way to execute a candidate.
**AC4** Bundles: `/Applications/X.app` with `Contents/Info.plist` ⇒ `source: AppBundle`; the same
directory **without** `Info.plist` ⇒ not detected; the spec is `open -a <bundle> <dir>`,
`wait_for_exit == true`, bundle as **one** token (`args.len() == 3`), incl. a bundle path with spaces.
**AC5 — hostile-selection table.** For each of `"C:\hostile\payload.exe"`, `"/tmp/payload"`, `"node"`,
`"make"`, `"nmake"`, `"just"`, `"msbuild"`, `"python"`, `"powershell -c calc"`, `"code {path}"`,
`"../../evil"`, `"\\server\share\x.exe"`, `"custom:7"` with no such record, a 2000-char string, and a
string containing U+202E: `coerce_tool_id` ⇒ `""` **and** `picked(..)` ⇒ `None`. Include a path that
**does exist** on the test machine, to pin that existence never matters.
**AC6 — provenance invariant.** For every catalog entry, every fake `Resolution`, and every
`CustomTool`, `LaunchSpec.program` is `entry.program`, `"open"`, or an absolute path that came from a
probe or a stored record; every argv token is a catalog `&'static str`, a catalog prefix + the target
dir, or the target dir. No test may build a `LaunchSpec` from a free-text program — `template_spec` is
gone, so no constructor exists.
**AC7** Migration: one case per §5.3 invariant 1-5, plus `"code {path}"`→`vscode`,
`"wt -d {path}"`→`windows-terminal`, `"cmd /K"`→`cmd`, `"C:\Program Files\X\x.exe"`→`""`,
`"make"`→`""`; both-keys-present keeps the new one; after save→load→save the legacy keys are absent
from the JSON text; a file with neither loads `""`/`""`; **and `custom_tools` is still empty after
every migration case**.
**AC8** Catalog integrity: ids unique per `(kind, os)`; **no catalog id contains `:`** (so it can never
collide with `custom:<n>`); every `AUTO_*` id resolves via `find`; **every `MacOpen` entry and every
`MacApp`-referenced entry has `Some(app_name)`**; `BuiltIn` is an entry's only rung; every
`LEGACY_ALIASES` target exists in `CATALOG`; `MacOpen` entries are `os == MacOs`.
**AC9** The **auto ladders are unchanged**: the pre-existing `external_tests.rs` assertions (`wt -d` /
`powershell` / `cmd /K`; `open -a Terminal`; `gnome-terminal --working-directory=` / `konsole
--workdir` / `x-terminal-emulator`; `code` / `code-insiders`; the macOS editor trio
`open -a "Visual Studio Code"` / `open -a "Visual Studio Code - Insiders"` / `code`) pass with no edit
other than the new parameter, and `only_the_directory_less_terminal_rungs_keep_the_repo_as_cwd` holds.
**AC10** `list_external_tools` returns both lists (detected then user-chosen) in one round trip, never
rejects for detection state, runs under `spawn_blocking`; `refresh: true` re-probes (`scannedAtMs`
advances).
**AC11** `picked()` recheck by source: a `BuiltIn` selection (`cmd`, `powershell`) resolves **without**
any fs test; a `Path`/`WellKnown` selection whose file was removed ⇒ `None`; an `AppBundle` or
`UserChosen`-bundle selection whose bundle was removed ⇒ `None`.
**AC12** Mock parity: `VITE_MOCK_IPC=1` serves all seven §6 seams; the harness shows the populated
picker, the nothing-detected empty state, the stale-selection state, the busy state, a user-chosen row
with its Remove control, and the cancel path. One screenshot of populated + empty as the final visual
proof (frugal-verification rule).
**AC13** Frontend tests: picking a tool patches `{ editorTool: '<id>' }` and nothing else; the row `↺`
is **absent** at `''` and patches `''` when a tool is picked; Browse calls `addCustomTool(kind)` and
selects the returned id; `null` (cancel) changes nothing; `useUiSettings` round-trips both new keys; no
`terminalCommand`/`editorCommand` identifier remains in `src/`, `src-tauri/` or `e2e/` outside the
§5.3 legacy fields and the alias table (grep is the evidence).
**AC14** An `#[ignore]`d host test (`cargo test -- --ignored`): `HostToolEnv` on this Windows box
detects at least `cmd` and `powershell` (both `BuiltIn`, so it cannot fail on a supported Windows), and
any `Path`/`Registry`/`WellKnown` hit is an **absolute existing** file. Real detection, no native
window.
**AC15 — the §5.4 property, tested three ways.** (a) `UiSettingsPatch` has **no** field carrying a
path or program — asserted structurally (the struct's field list) so a later addition trips the test;
(b) a patch JSON containing `"customTools": [{…}]` leaves `custom_tools` **empty** (unknown key,
silently ignored) and returns `Ok`; (c) a patch of `editorTool: "C:\payload.exe"` — *for a file that
exists* — stores `""`. Together: **no renderer-written path is honoured, and a renderer-written path is
not even representable.**
**AC16 — `validate_custom_program` table** (pure, no dialog): non-existent ⇒ refused; UNC ⇒ refused; a
plain directory ⇒ refused; a `.app` directory with `Info.plist` ⇒ `MacBundle`; a non-executable file on
unix ⇒ refused; a path with U+202E or a control char ⇒ refused; over `MAX_LEN` ⇒ refused; the 9th
record for a kind ⇒ refused; a duplicate `program`+`kind` ⇒ the existing record, no second row. Error
messages are **category-only and never echo the path** (the `refuse` idiom). Plus: `add_custom_tool`
cancel ⇒ `Ok(None)` with **no** settings mutation.
**AC17** Custom-tool launch shapes: Editor+Executable ⇒ one argv token, `cwd == safe_cwd()`,
`hide_console`; Terminal+Executable ⇒ `args.is_empty()`, `cwd == target`; either+`MacBundle` ⇒
`open -a <bundle> <dir>` with `wait_for_exit`.
**AC18** `remove_custom_tool` drops the record **and** resets a `terminalTool`/`editorTool` that
pointed at it to `""`; an unknown id is `Ok(())`; ids are never reused after removal. Both mutators go
through `settings::update` — asserted by a concurrent add-vs-patch test that proves neither write is
lost.

### USER CHECKPOINT — must NOT be self-confirmed

**UC1** In `pnpm tauri dev` → Settings → External tools, the pickers list the tools actually installed.
Only the native run exercises the real registry / well-known / `PATH` rungs against a real install set.
**UC2** Picking a terminal and a non-VS-Code editor, then using "Open in terminal" / "Open in editor"
from the repo, worktree and submodule menus, launches **that** tool at **that** folder.
**UC3** Rescan reflects reality: install or remove a tool, press Rescan, the list changes.
**UC4** With `editorTool` set to a tool then uninstalled, "Open in editor" still opens something
(OQ1 silent fallback) and the picker marks the selection "not installed".
**UC5** macOS: a `.app`-sourced editor and terminal both launch at the right folder. **May be WAIVED
for lack of a Mac** — say so explicitly rather than implying it passed; the `TargetOs::MacOs` branch
and bundle detection are unit-covered regardless (AC4).
**UC6 — §5.4 Browse, native by construction.** Press Browse, pick a real portable editor in the OS
dialog, confirm the row appears with the right label, select it, and launch a folder with it. Then
Cancel the dialog and confirm nothing changed, and Remove the row and confirm the selection reverts to
Auto-detect. **There is no dialog in the browser harness**, so no part of this is
orchestrator-verifiable beyond the mock wiring (AC12/AC13).
**Not a checkpoint item — migration.** The ledger records both legacy values as **empty** in the user's
real `settings.json`, so a native run cannot demonstrate migration. AC7 is the whole proof.

---

## 11. `ui-designer` prerequisite (workflow step 2b — before implementation)

The settings rows change from free-text inputs to a selection control **plus a Browse affordance**, so
`ui-designer` writes `docs/contracts/P112-ui.md` (and updates `ui-reference.md` if a new state pattern
appears) **first**. Inputs: this file, `src/components/SettingsExternalToolsSection.tsx`,
`src/components/settings/catalog/general.ts:56-75`, `src/components/settings/types.ts:44-52`.
Must cover:

* Rows `general.terminal-tool` / `general.editor-tool`; **reuse the existing `SettingsControlKind:
  'combobox'`** (`types.ts:47-49`) — do not invent a control. `"Auto-detect"` is the `''` option and
  the reset target (`resetKey(…, 'auto-detect')`).
* Option row shape: `label` + the `detail` subtitle, and whether `source` is surfaced at all.
* All states: auto-detect (default) · populated · **nothing detected** (which must still offer Browse)
  · picked-but-not-installed (kept, marked — OQ1 ruled) · scanning/busy · Rescan and its result copy
  (`scannedAtMs`).
* **The Browse affordance (§5.4):** where it sits relative to the combobox; the user-chosen row's
  Remove control and its confirm-or-not decision; the cancel path (no visible change); the cap-reached
  and refusal messages (category-only — they must not echo the path); and honest copy that Bonsai will
  **run the program you choose**, so choose one you trust. Do **not** imply Bonsai validates safety.
* Replacement copy for the deleted "Enter a command name … no arguments" note.
* Keyboard/a11y for the combobox, Browse and Remove; both themes; the `general.*` search keywords.
