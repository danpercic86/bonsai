# P112 — External tools: detected-list picker + native Browse (removes free-text commands)

**Status:** spec — **UI contract written** (`docs/contracts/P112-ui.md`); ready for `senior-dev`.
**Authorised:** user 2026-09-11 (ledger #4 end state; `TODO.md` "Roadmap: REMOVE user-supplied
`terminalCommand` / `editorCommand`"). **OQ1 + OQ2 ruled by the user; AMEND-1/2/3 and DEC-1/2/3 from
`P112-ui.md` absorbed here on 2026-09-11 — see §9.**
**Sources:** `docs/audit-2026-09-03-external-launch.md` MEDIUM-2 / LOW-1 ·
`docs/audit-2026-09-11-mcp-tool-contracts.md` HIGH-1 (same class, other surface) ·
`P91-observability.md` F4 (the backend-invoked-dialog pattern, `commands/obs.rs:388-400`) ·
`docs/contracts/P112-ui.md` §0 (AMEND-1/2/3, DEC-1/2/3).
**Split:** the static candidate table, auto ladders and legacy aliases are in
`docs/contracts/P112-tool-catalog.md` — needed only by the `catalog.rs` implementer.

**THE INVARIANT THIS MILESTONE BUYS.** *No byte originating in the renderer, in `settings.json`, or in
a repository ever becomes `LaunchSpec.program`, nor any argv token other than the one target
directory.* Every program, flag and app name is a `&'static str` in a compile-time catalog, an
absolute path Bonsai's own probe produced, or **a path a human chose in a native dialog that the
backend itself opened** (§5.4). A value the renderer can write is only ever a **lookup key**.
Shape-validating a free-text program string cannot achieve this; deleting the field can.

**The frontend half of the same property** (`P112-ui.md` §3): the control is the existing
`src/components/Combobox.tsx` in **strict mode** (`allowFreeInput` omitted ⇒ `false`), which is
*structurally incapable of committing free text* — it reverts to the selected option's label on blur
and Escape. Strict mode here is a **security** property, not a convenience: free-text entry is the
defect P112 removes, so a later `allowFreeInput: true` on these two rows would reintroduce it in the
UI even though Rust still refuses the value. Treat it as load-bearing.

> **Line numbers were taken against the 2026-09-11 working tree and some have already drifted**
> (`external_cmd.rs` grew during the in-flight fix round: `validate_command_setting` is now
> `:176-206`, `is_shell_syntax` `:121`, `safe_cwd` `:233`). **Resolve every citation by symbol name
> first**, line number second.

---

## 0. Verification of the brief (checked against source, not taken on trust)

| Claim | Verdict |
|---|---|
| Route 1: the absolute-path branch accepts any existing file (`is_file()` only) | **TRUE** — `validate_command_setting`, absolute branch (`external_cmd.rs:190-200`). Two precisions: the value must also carry no `is_shell_syntax` char, and `CreateProcess` must accept the image — a PE, or a `.cmd`/`.bat` (which runs via `cmd.exe`, with `%VAR%` argv expansion). |
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
| `tools/mod.rs` | Types + DTOs, the cache, `tool_scan`, `refresh_tool_scan`, `picked`, `coerce_tool_id`, `legacy_tool_id`, `validate_custom_program`, `synthesize_recipe`, `label_map`. | the candidate table; any `Command`; any dialog |
| `tools/catalog.rs` | **Static data only:** `ToolEntry`/`Rung`/`Recipe`/`AutoRung` defs, `CATALOG`, `AUTO_*`, `LEGACY_ALIASES`, `find`, `entries_for`, `CUSTOM_ID`. | filesystem, registry, spawn |
| `tools/detect.rs` | `ToolEnv` + `HostToolEnv`, `probe_entry`, `scan_for`. | the table; `LaunchSpec` |
| `tools/fake.rs` (`cfg(test)`) | `FakeToolEnv` — declarative present paths / PATH hits / registry values (`external_fake.rs` precedent). | — |
| `tools/detect_tests.rs`, `tools/catalog_tests.rs`, `tools/custom_tests.rs` (`cfg(test)`) | per-OS ladders, bundles, coercion, migration, browsed-path validation. | — |

`lib.rs`: add `pub mod tools;`.

### Changed

| File | Change |
|---|---|
| `bonsai-core/src/external.rs` | `template_spec` + `PathDelivery` deleted; the four launch entry points take `Option<&PickedTool>`; new private `spec_from`; auto ladders built from `tools::catalog` (§4); the MEDIUM-2 module-doc section replaced. |
| `bonsai-core/src/external_cmd.rs` | **Deleted** — `safe_cwd()` moves to `procutil.rs`, four helpers move to `tools/mod.rs` (§7). |
| `bonsai-core/src/external_url.rs` | import only → `use crate::procutil::safe_cwd;` |
| `bonsai-core/src/gitbin.rs` | `parse_reg_query` → `pub(crate)`; `HostGitEnv::registry_string` gains the `value == "" ⇒ /ve` branch (§3). No behaviour change for existing callers. |
| `src-tauri/src/commands/external.rs` | `launch_inner` reads the new keys and resolves via `tools::picked` (§4). |
| `src-tauri/src/commands/tools.rs` (new) | `list_external_tools`, `pick_external_tool` (§6); registered in `lib.rs` `generate_handler!` beside `commands::open_in_editor` (`lib.rs:322`). |
| `src-tauri/src/settings.rs` | `terminal_tool`, `editor_tool`, `custom_terminal_path`, `custom_editor_path` added; the two legacy keys become `skip_serializing` migration input (§5.3). |
| `src-tauri/src/commands/ui_settings.rs` | key rename + write-time coercion (`:57,:59,:134,:136,:243-247,:325-326`). **`custom_terminal_path` / `custom_editor_path` are deliberately absent from BOTH `UiSettings` and `UiSettingsPatch`** (§5.4). |
| `src/components/Combobox.tsx` | **no change** — used in strict mode (`allowFreeInput` omitted). Listed so nobody "improves" it for these rows; see the header note. |
| Frontend | `src/ipc/types/{common,settings,ipc-api}.ts` · `src/ipc/tauri/app.ts` · `src/ipc/mock/handlers/{external,session}.ts` · `src/ipc/mock/persistence.ts` · `src/settings/defaults.ts` · `src/settings/uiSettingsDefaults.json` · `src/hooks/useUiSettings.ts` · `src/components/settings/catalog/general.ts:56-75` · `src/components/SettingsExternalToolsSection.tsx` · `src/obs/rawArgPolicy.json` · `src/test/uiSettingsKit.ts`. Exact lines in §7; component/state detail in `P112-ui.md` §9. |

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

/// `Custom` = the browsed path (§5.4). NOTE the rename: an earlier draft called it
/// `UserChosen`; the wire value is `"custom"`, matching the `CUSTOM_ID` pseudo-id,
/// so there is ONE vocabulary for this concept. Do not implement both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolSource { BuiltIn, Path, Registry, WellKnown, AppBundle, Custom }

/// What a successful probe produced. `program`/`bundle` are ALWAYS a catalog
/// `&'static str`, an absolute path this crate built, or the stored browsed path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub program: String,          // absolute path; the catalog name for BuiltIn
    pub bundle: Option<String>,   // macOS `.app` dir; Some ⇔ AppBundle or a Custom bundle
    pub source: ToolSource,
}

/// A resolved selection — the ONLY thing a launch path accepts besides `None`
/// (= auto ladder). Deliberately NOT `&'static ToolEntry`: a browsed tool has no
/// catalog entry, and folding both into one shape is what stops the two launch
/// paths from drifting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedTool {
    pub kind: ToolKind,
    /// The catalog entry's recipe, `MacOpen` for any bundle resolution, or the
    /// synthesized recipe for a browsed tool (§5.4).
    pub recipe: Recipe,
    /// `LaunchSpec.program`: an absolute path, the catalog name for `BuiltIn`,
    /// or `"open"` when `recipe == MacOpen`.
    pub program: String,
    /// The `open -a` argument. `Some` **iff** `recipe == MacOpen`: the resolved
    /// bundle PATH for a bundle/browsed bundle, else `entry.app_name`.
    pub open_arg: Option<String>,
    pub source: ToolSource,
}

/// IPC DTO — display-only. The backend never accepts `label`/`detail`/`source`/
/// `present` back; the frontend may send back only `id`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTool {
    pub id: String,      // a catalog id, or the pseudo-id "custom"
    pub label: String,
    pub kind: ToolKind,
    pub source: ToolSource,
    /// EXACTLY: the resolved absolute path for Path / Registry / WellKnown /
    /// AppBundle / Custom; the literal `"built in"` for BuiltIn.
    pub detail: String,
    /// **AMEND-3.** Did the target resolve at scan time? Always `true` for a
    /// probe-derived row (it was just probed). `false` only for the remembered
    /// `custom` row whose stored path is gone — which is still LISTED, so the UI
    /// can render the "custom path gone" state (`P112-ui.md` §5 state 6) instead
    /// of showing an unexplained empty picker.
    pub present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalToolScan {
    pub terminals: Vec<DetectedTool>,   // detected in table order, then the custom row
    pub editors: Vec<DetectedTool>,
    /// **AMEND-1.** `id -> label` for EVERY catalog entry of this kind on EVERY
    /// OS. Without it a kept-but-undetected selection (the OQ1 ruling) renders as
    /// the raw id — the picker would literally read `notepadpp`. The map is what
    /// makes the OQ1 ruling presentable, so it is required, not advisory.
    pub terminal_labels: BTreeMap<String, String>,
    pub editor_labels: BTreeMap<String, String>,
    /// Freshness identity the frontend hook compares to know a refresh landed.
    /// **DEC-3: never displayed** (the app has no time formatter; the picker note
    /// reports counts instead). Kept in the DTO for exactly that comparison.
    pub scanned_at_ms: u64,
}

// ---- public API (BLOCKING except the pure fns, which are marked) ------------
/// `custom_terminal` / `custom_editor` are the stored browsed paths (`""` = none).
pub fn tool_scan(custom_terminal: &str, custom_editor: &str) -> ExternalToolScan;
pub fn refresh_tool_scan(custom_terminal: &str, custom_editor: &str) -> ExternalToolScan;
/// `""`, an unknown id, a wrong-kind id, `"custom"` with an empty/gone stored path,
/// or a known id whose target no longer exists ⇒ `None` (⇒ auto ladder).
/// One fs recheck, no re-probe.
pub fn picked(setting: &str, kind: ToolKind, custom_path: &str) -> Option<PickedTool>;
/// PURE. Returns the id unchanged when `CATALOG` holds it for `kind` (any OS —
/// settings may be synced between machines), or when it is `CUSTOM_ID` **and**
/// `has_custom_path`; else `""`. No filesystem.
pub fn coerce_tool_id(value: &str, kind: ToolKind, has_custom_path: bool) -> String;
/// PURE. One-shot migration of a legacy free-text command (§5.3). No probe.
pub fn legacy_tool_id(legacy: &str, kind: ToolKind) -> String;
/// PURE. `id -> label` for every catalog entry of `kind`, all OSes (AMEND-1).
pub fn label_map(kind: ToolKind) -> BTreeMap<String, String>;
/// §5.4 rule set for a dialog-returned path. `os` is explicit (house pattern) so
/// the Windows `.exe` rule is unit-testable on any host. Touches the fs
/// (existence, bundle, exec bit); never spawns. Category-only errors.
pub fn validate_custom_program(path: &Path, os: TargetOs)
    -> Result<CustomKindShape, AppError>;
pub enum CustomKindShape { Executable, MacBundle }
/// PURE. `(kind, shape)` ⇒ the recipe a browsed tool launches with (§5.4).
pub fn synthesize_recipe(kind: ToolKind, shape: CustomKindShape) -> Recipe;
```

```rust
// tools/catalog.rs — data in docs/contracts/P112-tool-catalog.md
/// The reserved pseudo-id for the browsed tool. No catalog id may equal it (AC8).
pub const CUSTOM_ID: &str = "custom";

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
/// resolves for `coerce_tool_id` and `label_map`).
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

tool_scan(custom_terminal, custom_editor):
  for kind in [Terminal, Editor]:
    rows = cached scan_for(host) rows of this kind -> DetectedTool{ present: true, .. }
    if custom_path(kind) is non-empty:              # LISTED even when gone (AMEND-3)
      shape = validate_custom_program(path, host).ok()
      rows.push(DetectedTool{ id: CUSTOM_ID, source: Custom, detail: path,
                              label: display_label(path), present: shape.is_some() })
    labels(kind) = label_map(kind)                  # AMEND-1, all OSes
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
  restarting. Only *probe* results are cached; the custom row and the label maps are derived per call
  (a stored path is one stat, a label map is static data).
* **Lazy and explicit.** Nothing scans at boot. The only trigger is `list_external_tools`, called when
  the Settings *External tools* section mounts; it runs under `spawn_blocking` and is then cached for
  the process lifetime. `refresh: true` ⇒ `refresh_tool_scan()`. No timer, no focus rescan —
  detection is not repo state.
* **Launch never re-runs the ladder** (a `Registry` rung spawns a process). `picked()` reads the cache
  (populating it if empty) and does **one** recheck, by source:
  `BuiltIn` ⇒ **no recheck** (present by definition — an `is_file("cmd")` test is false and would
  silently break picking `cmd`/`powershell`); `Path` / `Registry` / `WellKnown` ⇒ `is_file`;
  `AppBundle` ⇒ `is_bundle`; `Custom` ⇒ re-run `validate_custom_program` (cheap: stat + exec bit, or
  bundle check) so a path that changed shape is refused, not launched. Miss ⇒ `None` ⇒ auto ladder
  (OQ1 ruling — no toast).
* `picked()` normalises: for any bundle resolution it returns
  `recipe: MacOpen, program: "open", open_arg: Some(bundle)`, so `spec_from` needs no bundle branch.

> ### AMEND-5 (orchestrator, 2026-09-14) — what sub-inc 1 actually built, folded back
>
> Recorded after `reviewer` and `security-auditor` both passed sub-increment 1 (no MUST-FIX, no
> CRITICAL/HIGH/MEDIUM). **Every delta below makes the code STRICTER than this contract, not looser.**
> That matters: a later reader comparing code to contract would otherwise read these as drift and
> "fix" the wrong side. Each item names its own target so it can be applied in place later.
>
> 1. **§3's probe pseudocode understates the guards.** It specifies `is_file` alone; `executable_hit`
>    additionally requires `looks_absolute` **and**, on `TargetOs::Windows`, a file extension. Both
>    apply to `OnPath`, `AppPaths`, `WinFolder` and `UnixFile`. The extension rule is a **correctness
>    heuristic, not a security boundary** — `CreateProcess` ignores extensions and validates the image
>    header, so a PE without one would run, and `Path::extension()` yields `Some("")` for a
>    trailing-dot name. It exists because a *measured* `os error 193` (see below) would otherwise put
>    an unlaunchable tool in the picker.
> 2. **§3's "`None` on ANY failure" for `registry_string` is false.** An existing key with **no
>    default value** makes `reg query … /ve` exit **0** and print `(Default) REG_SZ (value not set)`,
>    which `parse_reg_query` returns as a non-path `Some`. Verified against `HKCU\Environment` on this
>    host. Safe today only because `looks_absolute` rejects it downstream — a distant guard. Callers
>    must shape-check; do **not** "fix" this by matching the literal, which is localized.
> 3. **AC13's "`detail` = the stored path"** ⇒ **"the *sanitized* stored path; verbatim iff it
>    validates."** `sanitize_detail` and `validate_custom_program` share `is_disallowed_char`, so a
>    validating path is byte-identical by construction — which is why the strip set is the single
>    point of truth.
> 4. **§7 sends four helpers to `tools/mod.rs`; they land in `tools/custom.rs`** — an extra module
>    versus §1's map, split out because `mod.rs` was already 387 lines.
> 5. **§1's module map** is missing `tools/catalog_table.rs` and `tools/scan_tests.rs`, and its
>    "widen `parse_reg_query` to `pub(crate)`" line should be **dropped**: the `/ve` branch lives
>    inside `HostGitEnv::registry_string`, so the parser stays private.
> 6. **AC16 is host-bound on its accept cases, and this is forced, not lazy.** Every *refusal* is
>    asserted on all three OSes, but the `.exe` accept is `cfg(windows)`-only and the bundle /
>    exec-bit accepts `cfg(unix)`-only, because the signature requires a path that both exists on the
>    host and is absolute *for the target OS* — no single machine satisfies both. The
>    security-relevant half (DEC-1's `.cmd`/`.bat`/`.ps1` refusal) **is** proven on Windows, which is
>    the gate host; what goes unasserted there is capability, not safety. A seam-injected filesystem
>    is the only way to close it. **State this in AC16 rather than implying full coverage.**
> 7. **The catalog has 35 rows, not 36** (contract and code agree at 35; only a code comment said 36).
> 8. **`§4`'s auto-ladder lookup — see AMEND-4 above.** Unchanged and still binding: use `find_for`.
>
> **A shipped bug this increment uncovered, measured rather than argued.** `external.rs`'s Windows
> `editor_ladder` opens with bare `spec("code", …)`, and `procutil::resolve_program` returns
> `dir.join(program)` **before** its `PATHEXT` loop — so `"code"` resolves to
> `…\Microsoft VS Code\bin\code`, a 2073-byte file beginning `#!/usr/bin/env sh`. A standalone
> `rustc` probe making the exact `Command::new(path).spawn()` call `SpawnRunner` makes:
> **`bin\code` ⇒ ERR `os error 193` "%1 is not a valid Win32 application"**, while `bin\code.cmd`
> and `Code.exe` both spawn OK. Rung #2 is bare `code-insiders`, normally absent — so on a standard
> Windows VS Code install **"Open in editor" fails today**. P112 is what exposed it, and P112 also
> **removes the `editorCommand` escape hatch that currently masks it**, so the ladder fix is not
> optional cleanup. Probe kept at `D:\Data\Temp\claude\shim-probe\shim_check.rs`.

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

> **AMEND-4 (orchestrator, 2026-09-14, after sub-inc 1 landed).** The `catalog::find(kind, rung.id)`
> on the line above is **wrong for this call site and will panic off-Mac.** `find` resolves
> host-OS-first, so on a Windows host building `AUTO_LIST[MacOs][Editor]` it returns the **Windows**
> `vscode` row — whose `app_name` is `None` — and `e.app_name.expect("AC8")` panics. That also makes
> **AC9 unprovable from Windows**, which is the only machine this project builds on.
>
> **Sub-increment 3 must call `catalog::find_for(kind, id, os)`, not `find`.** `find_for` was added in
> sub-inc 1 for exactly this reason and is pinned by
> `find_for_is_exact_os_which_is_what_an_os_explicit_caller_needs`:
> `find_for(Editor, "vscode", MacOs).app_name == Some("Visual Studio Code")` against
> `find_for(Editor, "vscode", Windows).app_name == None`.
>
> The general rule, since this will recur: **any caller that takes `os` as a parameter must resolve
> the catalog by that `os`, never by the host.** `find` is only correct when the caller genuinely
> means "the current machine". AC8's totality is per-OS, so an `unwrap()` justified by AC8 is only
> justified when the lookup and the ladder agree on which OS they are talking about.

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
    let picked = tools::picked(&s.terminal_tool, ToolKind::Terminal, &s.custom_terminal_path);
    external::open_in_terminal(&runner, os, picked.as_ref(), p)
}
```

---

## 5. Settings

### 5.1 Shape (**AMEND-2**)

| Wire key | Rust field | Written by | Meaning |
|---|---|---|---|
| `terminalTool` | `terminal_tool` | renderer, **coerced** | `""` ⇒ auto ladder · a catalog id · the pseudo-id `"custom"` |
| `editorTool` | `editor_tool` | renderer, **coerced** | same, editors |
| `customTerminalPath` | `custom_terminal_path` | **`pick_external_tool` only** | absolute path the native dialog produced; `""` = none |
| `customEditorPath` | `custom_editor_path` | **`pick_external_tool` only** | same, editors |

All four are `String` with `""` as the default, additive `#[serde(default)]`. `SETTINGS_VERSION` stays
**1** — additive fields plus a removal with a safe default are below the bump bar `settings.rs:58-71`
documents.

**Why the path is a separate field and not encoded in the id.** `terminalTool: "custom:C:\…"` was
considered and **rejected as a defect**: the id is the only storage, so the path would be destroyed
the moment the user picked any other tool, and — because the id round-trips through `UiSettings` —
every full-state resend or failed-patch requeue would carry the path through the renderer and then
coerce it to `""`, a silent loss. Splitting them makes "revert to Auto-detect" **reversible** (the
path survives, re-picking `custom` restores it), which is why the UI needs no confirmation for the
reset (`P112-ui.md` §10). The path travels **outbound only**, once, as `DetectedTool.detail` for
display; it is never an inbound value and never part of an identifier.

### 5.2 Write-time coercion — the "renderer writes garbage ⇒ selects nothing" property

```rust
// apply_patch
if let Some(v) = patch.terminal_tool {
    let has = !s.custom_terminal_path.is_empty();
    s.terminal_tool = tools::coerce_tool_id(&v, ToolKind::Terminal, has);
}
if let Some(v) = patch.editor_tool {
    let has = !s.custom_editor_path.is_empty();
    s.editor_tool = tools::coerce_tool_id(&v, ToolKind::Editor, has);
}
```

* Anything that is neither a catalog id nor `"custom"`-with-a-stored-path becomes `""`. Pure lookup,
  zero IO — so it cannot reproduce the failure that ruled out save-time validation (`external_cmd.rs`
  module doc: the settings writer merges pending keys into one patch and re-queues on failure, so a
  *rejection* would wedge every later settings write). Coercion never errors.
* `"custom"` is renderer-sendable **and** coerce-safe: it selects only a path the user already browsed
  to. With no stored path there is nothing to select, so it degrades to `""`.
* An id **in the catalog but not currently detected** is **kept** — a legitimate stale selection,
  named via the AMEND-1 label map and marked "Not installed"; launch silently uses the auto ladder.
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
1. The output is a catalog id or `""` — **never** a program string, never a path, **never `"custom"`,
   and it must never write `custom_*_path`.** A stored string cannot become an executed program, and
   migration cannot manufacture the human dialog click §5.4 requires. This is the single most
   important migration rule: a legacy `"C:\Tools\npp\notepad++.exe"` becoming
   `custom_editor_path` would reintroduce exactly the capability P112 deletes.
2. Misses are **accepted, not preserved**: `"C:\Tools\payload.exe"` → `""`, `"make"` → `""`,
   `"node"` → `""`, `"/opt/custom/bin/myed"` → `""`. `"powershell -c calc"` → `"powershell"` is
   correct: the user asked for PowerShell and the `-c calc` tail is dropped — which is the point.
3. An absolute path **with spaces** normalises to its first token and so usually misses ⇒ `""`. The
   user re-adds a portable editor with one Browse click (§5.4) — exactly the case OQ2's ruling
   restores.
4. A pre-existing non-empty new key always wins over the legacy key.
5. Idempotent: after one save the legacy keys are gone from the file; a second load is a no-op.

### 5.4 The browsed path — native dialog, backend-invoked (**OQ2 RULED**)

**Why this exists.** When the user ruled on MEDIUM-2 they chose shape validation *because* an
allowlist "breaks a legitimate absolute path to a portable editor". A detected-list-only design breaks
that case harder, so the ruling takes the third path: keep the free-text field deleted **and** give
back the capability through a channel the renderer cannot drive.

**The mechanism, and the exact security property.**

1. `custom_terminal_path` / `custom_editor_path` live on `settings::Settings` and are **absent from
   both `UiSettings` and `UiSettingsPatch`**. `set_ui_settings` therefore has **no field that can
   carry a path** — a renderer-written path is not "rejected by a validator", it is
   **unrepresentable in the patch type**. (`UiSettingsPatch` has no `deny_unknown_fields`, so an extra
   `"customEditorPath"` key in patch JSON is silently ignored; AC15 pins that, so a later refactor
   that "helpfully" adds the field is caught.)
2. The **only** writer is `pick_external_tool(kind)`, which opens the dialog **itself**, in Rust, via
   `tauri_plugin_dialog::DialogExt`. The renderer supplies only `kind` (an enum) — it asks for a
   dialog, it does not supply a result. The chosen path never crosses IPC inbound.
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
5. **There is deliberately no "forget the path" command.** Reverting to Auto-detect is the ordinary
   patch `{ editorTool: "" }` (non-destructive by design, §5.1), and re-running Browse overwrites the
   stored path. One writer, one code path, nothing extra to audit.

**Validation applied to the dialog's result** — `validate_custom_program(path, os)`, called by
`pick_external_tool` **before** anything is written. Validating here is safe: this is a dedicated
command, so a refusal is an ordinary command error and cannot wedge the settings writer the way
patch-time rejection would.

| Rule | Reason |
|---|---|
| absolute | the dialog returns absolute; assert it anyway |
| `is_file()` **or** macOS `is_bundle()` ⇒ `CustomKindShape` | the `.app` case is precisely what `is_file()`-only got wrong |
| **Windows: extension MUST be `.exe`** (case-insensitive) | **DEC-1, and this is the enforcement half** — see below |
| unix: at least one execute bit (file form) | the meaningful gate on unix; a non-executable pick otherwise fails at spawn with an opaque error |
| ~~reject UNC / `//host/share`~~ **-- SUPERSEDED by ruling #26, see AMEND-6** / still reject `\\?\` and `\\.\` device prefixes | was: house rule (`is_unc`, `git::submodule_abs_path`), a remote share is not a local tool. **UNC is now ACCEPTED on the browse path only** -- the device-prefix refusal stands. |
| reject control + bidi characters in the path **and** in the derived label | the one genuine spoofing surface here: a bidi override in a filename makes the picker row read as something it is not |
| length ≤ 512 (`MAX_LEN`) | a longer string is not an install path |

`label` is derived by the **backend** (macOS: bundle name minus `.app`; else the file stem;
control/bidi stripped, truncated) and is never renderer-supplied.

> ### AMEND-6 (user ruling #26, 2026-09-14) -- UNC is allowed here, and ONLY here
>
> The reviewer raised this as an OQ rather than settling it: refusing UNC in **both** detection and
> Browse meant a tool installed on a network share was **neither detectable nor selectable, with no
> workaround** -- plausible on a corporate-managed machine, which is what this project is developed
> on. **The user ruled: allow UNC via Browse only.**
>
> The asymmetry is deliberate and each half stands on its own reason:
> * **Detection keeps refusing UNC** (in `tools::detect`, at the `executable_hit` call site — the
>   `looks_absolute` helper this originally named has since been **deleted**; see the correction
>   below). A scan has **one 1500 ms budget for every rung on the machine**, and a picker that comes
>   up empty on a slow VPN is worse than one that omits a share install.
>
> **CORRECTION to my own reasoning above (orchestrator, 2026-09-14).** I justified detection's
> refusal as *"stat-ing a share inside a budgeted scan can hang or go over the wire."* **That is true
> for the `WinFolder` and `AppPaths` rungs only, and FALSE for `OnPath` — and, per a correction the
> implementer made against me with code evidence, also false for `UnixFile`, where `probe_entry`
> calls `env.is_executable(&cand)` (`detect.rs:318`) *before* `executable_hit` is reached, making the
> refusal post-hoc there too. Moot in practice for that rung — its candidates are static catalog
> literals and can never be UNC — but do not re-widen the claim.** On the `OnPath` rung
> the stats have already happened: `procutil::resolve_in` calls `is_file()` on every candidate and
> `HostToolEnv::resolve_on_path` delegates straight to it, so **a single UNC `PATH` entry costs ~12
> network stats before `executable_hit` ever sees the string.** The refusal there is post-hoc — it
> stops the tool being *offered*, not the network from being touched.
>
> **The ruling is unaffected** (it is the user's, and "allow via Browse only" stands either way), and
> the hang concern is in fact **stronger** than I stated rather than weaker — it applies whether or
> not detection refuses the result. But it relocates the fix: preventing the network I/O needs a UNC
> guard **inside `procutil::resolve_in`**, which changes app-wide program resolution and is therefore
> its own change, filed separately. Do not justify detection's UNC refusal on I/O-avoidance grounds
> without this caveat.
> * **Browse accepts it.** A pick through the native dialog is a deliberate, one-time act in which
>   the user names one exact file. The network cost is paid once, knowingly.
>
> **What this changes in code:** `custom::is_absolute_for` and the UNC arm of the 5.4 validation must
> accept `\\server\share\...` and `//host/share/...`. `looks_absolute` is **unchanged** -- do not "unify"
> the two predicates on the grounds that they now disagree; they disagree *on purpose*, and the
> disagreement IS the ruling.
>
> **Still refused on the browse path:** `\\?\` and `\\.\` device prefixes (not shares -- device
> namespaces, and nothing a file dialog returns), plus every other rule in the table above, including
> the Windows `.exe` requirement. One prior finding bears directly on this: an earlier UNC probe
> established that `canonicalize` **succeeds** on UNC and returns the `\\?\UNC\...` form, so any
> prefix comparison added here must account for that form rather than assuming a share path stays in
> `\\server\share` shape.
>
> **AC to add:** the hostile-selection table (AC5/AC16) currently lists `"\\server\share\x.exe"` among the
> **refusals**. That row must **flip to accepted** for the browse path while remaining a refusal for
> detection -- so the two can no longer share one table and must be asserted separately.

**DEC-1, recorded as a decision with its rationale — and corrected on one point.** The Windows dialog
filters to `*.exe` only, excluding `.cmd`/`.bat`: Rust's `Command` routes those through `cmd.exe`
(the CVE-2024-24576 path), which performs `%VAR%` expansion on the argv it receives, so excluding them
removes that residual for browsed programs entirely. macOS filters `*.app` plus extensionless files;
Linux offers all files and the backend enforces the execute bit. **Correction the implementer must
not skip: a dialog filter is not a gate.** The Windows file dialog still lets a user type
`payload.cmd` in the filename box, and a filter is trivially widened by a later "add All files"
change — so DEC-1 is only real if `validate_custom_program` *rejects* a non-`.exe` extension on
Windows. The filter and the check must agree, and the check is the one that matters.
**Accepted capability cost** (user's consistent preference for the more secure option): a portable
tool that ships only a `.cmd` launcher cannot be browsed to. The common `.cmd` cases (`code.cmd`) are
*detected* catalog entries already, so they remain reachable.
The asymmetry with unix is principled, not an oversight: `.cmd`/`.bat` are re-interpreted by
`cmd.exe`, whereas a unix script with the execute bit is launched by the kernel honouring its
shebang — there is no argv re-expansion to remove.

**Stated honestly: this is not a safety check on the program.** A browsed `.exe` is arbitrary code.
The property is "a human chose it in a native dialog the backend opened", not "it is harmless".
Nothing here decides whether the chosen binary is trustworthy, and nothing should pretend to.

**Launch delivery** — `synthesize_recipe(kind, shape)`:

| kind / shape | recipe | note |
|---|---|---|
| Editor, `Executable` | `DirLastArg(&[])` | folder as one argv token, `hide_console = true`, cwd = `safe_cwd()` |
| Editor, `MacBundle` | `MacOpen` | `open -a <bundle> <dir>`, waits |
| Terminal, `Executable` | `DirCwd(&[])` | no args, cwd = the target — the only mechanism "open a terminal here" has for an unknown program |
| Terminal, `MacBundle` | `MacOpen` | `open -a <bundle> <dir>`, waits |

The `DirCwd(&[])` row **is** audit route 3's shape (`make` with cwd = the repo and no arguments). It
returns **only** for a program a human browsed to and deliberately selected as their terminal. A
renderer cannot create that state, a repository cannot, and migration cannot (invariant 1). What
remains is a user able to misconfigure their own terminal — accepted, and documented rather than
hidden.

---

## 6. IPC surface

### Commands — request/response (a scan is small and one-shot: no event, no channel)

```rust
// src-tauri/src/commands/tools.rs
/// P112: detected tools + the remembered browsed tool + the AMEND-1 label maps.
/// NEVER rejects for detection state — an empty scan is a normal result (the
/// `check_git_availability` precedent). `refresh: true` re-probes and replaces the
/// process cache (the picker's Rescan). Git-state-free.
#[tauri::command]
pub async fn list_external_tools(app: tauri::AppHandle, refresh: bool)
    -> Result<ExternalToolScan, AppError>;                                  // spawn_blocking

/// P112 §5.4: open the NATIVE program picker and, on confirm, write BOTH
/// `custom_<kind>_path` AND `<kind>_tool = "custom"` itself. The renderer cannot
/// supply a path — it only asks for the dialog. `Ok(None)` = cancelled: nothing is
/// written. A path that fails `validate_custom_program` writes NOTHING either —
/// not the path, and not the selection.
#[tauri::command]
pub async fn pick_external_tool(app: tauri::AppHandle, kind: ToolKind)
    -> Result<Option<DetectedTool>, AppError>;
```

Returns `DetectedTool { id: "custom", source: Custom, label, detail: <path>, present: true }`.
Rejection is a **single category error** (the `refuse` idiom — never echoes the path); the UI shows
its own copy, not the message text (`P112-ui.md` §8 `BROWSE_ERR`).

`pick_external_tool` shape: build the dialog with `app.dialog().file()` + `add_filter(..)` per DEC-1,
take the result through the **async** `pick_file(callback)` bridged by a `oneshot` (the blocking
variant must not run on the async runtime, and the dialog must be on the main thread on some
platforms), then validate + write in `spawn_blocking`.

**The write MUST go through `settings::update` (the `SETTINGS_IO` mutex, `settings.rs:26-34`), never a
bare `load_from` + `save_to`, and it must set both fields in ONE cycle.** It is a load→mutate→save
exactly like `apply_patch`; outside the mutex a pick racing a pane-width drag-save would lose one of
the writes to the last rename — the precise failure that mutex exists for. Setting the path and the
selection in separate cycles could also leave `*_tool == "custom"` with an empty path (a state
`coerce_tool_id` would then scrub on the next unrelated patch).

Both are registered in `lib.rs` `generate_handler!`. Existing signatures
(`openInTerminal`/`revealInFileManager`/`openInEditor`/`openUrl`) are **unchanged**.

### TypeScript

```ts
// src/ipc/types/common.ts — mirrors the Rust DTOs
export type ExternalToolKind = 'terminal' | 'editor';
export type ExternalToolSource =
  | 'builtIn' | 'path' | 'registry' | 'wellKnown' | 'appBundle' | 'custom';
export interface DetectedTool {
  /** Opaque id — a catalog id or the pseudo-id 'custom'. The ONLY value sent back. */
  id: string;
  label: string;
  kind: ExternalToolKind;
  /** Not surfaced in the UI (DEC-2); kept for tests and provenance. */
  source: ExternalToolSource;
  /** Display-only: the resolved absolute path, or 'built in'. */
  detail: string;
  /** false only for a remembered 'custom' row whose path is gone (AMEND-3). */
  present: boolean;
}
export interface ExternalToolScan {
  terminals: DetectedTool[];
  editors: DetectedTool[];
  /** id -> label for every catalog entry of that kind, ANY os (AMEND-1). Names a
   *  kept-but-undetected selection, which would otherwise render as a raw id. */
  terminalLabels: Record<string, string>;
  editorLabels: Record<string, string>;
  /** Freshness identity only — NOT displayed (DEC-3). */
  scannedAtMs: number;
}

// src/ipc/types/ipc-api.ts — IpcApi members, beside openInEditor
/** P112: detected + remembered tools and the label maps. Never rejects for
 *  detection state. */
listExternalTools(refresh: boolean): Promise<ExternalToolScan>;
/** P112 §5.4: opens a NATIVE dialog in the BACKEND, which also writes the
 *  selection. Resolves `null` on cancel. There is no way to pass a path in. */
pickExternalTool(kind: ExternalToolKind): Promise<DetectedTool | null>;

// src/ipc/types/settings.ts — REPLACES terminalCommand/editorCommand (:105-108, :180-182)
/** P112: '' ⇒ per-OS auto-detect · a catalog id · 'custom' (the browsed path).
 *  An unrecognised value is coerced to '' on write. There is NO settings key on
 *  this type that carries a program path — the browsed path is backend-write-only
 *  and reaches the UI only as `DetectedTool.detail`. */
terminalTool: string;
editorTool: string;
// patch: terminalTool?: string; editorTool?: string;   // and NOTHING path-shaped
```

`AppHandle` is not an IPC argument, so the TS members take only the declared params. After
`pickExternalTool` resolves non-null the frontend re-reads settings (the backend already wrote the
selection) — it must **not** also patch `{ editorTool: 'custom' }`, or it will race its own write.

### Mock IPC (`VITE_MOCK_IPC=1`) — mandatory

`src/ipc/mock/handlers/external.ts` gains both commands, driven by the `query()` seam
(`src/ipc/mock/repoState.ts:159`). Seam names are `P112-ui.md` §12's — that contract owns the harness
states, this one owns the payload shapes:

| Seam | Behaviour |
|---|---|
| *(none)* | terminals `windows-terminal` (path), `powershell` (builtIn), `cmd` (builtIn); editors `vscode` (wellKnown, detail `C:\Program Files\Microsoft VS Code\Code.exe`), `sublime` (path), `notepadpp` (registry); both label maps fully populated |
| `?tools=none` | empty lists, label maps still populated (the stale state needs them) |
| `?tools=mac` | bundle fixture, `source: 'appBundle'`, `/Applications/….app` details |
| `?tools=stale` | persisted `editorTool: 'zed'`, absent from the list ⇒ named from `editorLabels`, marked Not installed |
| `?tools=slow` | `delay(1200)` ⇒ the scanning state |
| `?tools=custom` | persisted `editorTool: 'custom'` + a **pathological** 118-char `customEditorPath`; the row is listed with `present: true` |
| `?tools=customgone` | as above with `present: false` |
| `?tools=browsecancel` | `pickExternalTool` resolves `null` |
| `?tools=browseerr` | `pickExternalTool` rejects with the category error |
| `?tools=longlabels` | one 64-char label + one 180-char detail path, and a deep unix path |

`pickExternalTool` otherwise resolves a canned `custom` row after `delay(600)` (so the in-flight state
is observable) **and updates the mock's persisted selection**, mirroring the backend writing it.
`refresh: true` bumps `scannedAtMs`. There is **no dialog in the harness and the mock is not a
security seam** — Rust owns the dialog, `validate_custom_program` and `coerce_tool_id` (the
`validate_web_url` precedent). `persistence.ts` and `handlers/session.ts` swap the two legacy string
keys for `terminalTool`/`editorTool` (same `typeof === 'string'` guard, default `''`) and add the two
path keys as mock-only state.

### Observability

`src/obs/rawArgPolicy.json`: add `"listExternalTools": ["refresh"]` and **`"pickExternalTool": []`**.
The `kind` enum would itself be safe; `[]` is the conservative choice because this call's *result*
carries a local absolute path, and nothing about the browse flow needs to be raw-logged. Note the
policy governs **arguments** — no path is ever an argument here, which is the property worth having.
`AppHandle` is not an arg and must not appear in a policy row. Existing rows unchanged.

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
| the two free-text rows + the "Enter a command name…" note | `src/components/SettingsExternalToolsSection.tsx` — rewritten per `P112-ui.md` §9; its cases in `src/components/SettingsSections.test.tsx` ("External tools (program edits + reset)") rewritten as picker + Browse cases |

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
| the auto ladders | Nothing-detected is a real state: a fresh machine, an unlisted Linux terminal, a stale selection, a gone custom path. Byte-identical to today. |
| `spec`, `open_spec`, `reveal_spec`, `launch_first`, `LaunchSpec` (+ its cwd-is-a-directory invariant), `CommandRunner`, `SpawnRunner`, `TargetOs`, the `wait_for_exit` semantics | Unchanged mechanism; `MacOpen` depends on `wait_for_exit`. |
| `external_url.rs` in full, incl. `validate_web_url` | Different surface (URLs), untouched here. |
| `launch_inner`'s `is_dir()` precheck + category-only error (MEDIUM-1 / LOW-2) | Still the only thing keeping a non-directory out of `current_dir`. |
| `procutil::resolve_program`, `gitbin::parse_reg_query`, `winenv` untouched | Now also serve the `OnPath` and `AppPaths` rungs; `winenv`'s budget stays its own (§3). |
| `src/components/Combobox.tsx` as-is | Strict mode (`allowFreeInput: false`) is the frontend half of the no-free-text property (header note). Do not add free input to these rows. |

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
   browsed to". A shell opened at a hostile cwd runs the user's profile, not repo files; an editor
   handed a hostile folder may still execute repo-authored config subject to that editor's own trust
   model (e.g. VS Code Workspace Trust).
2. **The browsed path is arbitrary code — by design, and only by human act.** §5.4 restores "point
   Bonsai at my portable editor" without restoring free-text entry: the path comes from a
   backend-opened native dialog, neither path field exists on the patch type, and migration can never
   manufacture one. Route 1 stays closed because a compromised renderer cannot register the hostile
   `payload.exe` a repo shipped; the *user* would have to browse to it deliberately. The residual is
   user self-harm (including the `DirCwd` terminal shape of route 3), not renderer escalation.
3. **Pre-existing, unchanged:** `open` / `xdg-open` / `explorer` are still bare names resolved through
   `PATH`; a poisoned `PATH` could hijack them. `gitbin` uses an absolute `reg.exe` for exactly this
   reason (OQ3). The renderer keeps `dialog:allow-open` for `openRepo` — a repo path is data, not
   code, so that is out of scope here.
4. **Windows `.cmd`/`.bat` `%VAR%` expansion is now confined to *detected* shims** (`code.cmd`), which
   are catalog entries, not user input. DEC-1 removes it from the browse path entirely (§5.4).

---

## 9. Rulings, decisions, and remaining open questions

**User rulings (do not re-open):**
* **OQ1 — silent fallback.** A selected-but-uninstalled tool (catalog id *or* a gone custom path) is
  **kept** in settings, named from the AMEND-1 label map, marked "Not installed", and launch quietly
  uses the auto ladder. **No error toast.**
* **OQ2 — a native Browse button.** §5.4. The earlier "portable editors are an accepted loss" answer
  is **void**; no part of this contract still recommends it.

**`P112-ui.md` items, settled by the orchestrator:**
* **AMEND-1** — label maps in the scan (§2). Required: without them the OQ1 ruling renders raw ids.
* **AMEND-2** — `"custom"` pseudo-id + backend-write-only path fields (§5.1). Adopted as specified,
  including the rationale that it makes the reset reversible.
* **AMEND-3** — `pick_external_tool` (§6) and `present: bool` on `DetectedTool` (§2) for the
  path-is-gone sub-case. The custom row is listed whenever a path is stored, `present` reporting
  whether it still resolves.
* **DEC-1** — Windows browse filter `*.exe` only. Accepted **with the §5.4 correction that the filter
  must be backed by an extension check in `validate_custom_program`**, since a dialog filter is not a
  gate. Capability cost accepted: a `.cmd`-only portable tool is unbrowsable.
* **DEC-2** — `ToolSource` is not surfaced in the UI. It stays in the DTO for tests and provenance.
* **DEC-3** — `scannedAtMs` is not displayed; it stays in the DTO as the freshness identity. AC10
  unaffected.

**Still open:**
* **OQ3** — absolute `/usr/bin/open` and `%SystemRoot%\System32\explorer.exe`. Cheap, but it changes
  `reveal_spec` / `open_url` argv that existing tests pin. Recommend **not** in P112; file as a
  follow-up so this contract stays one concern.
* **OQ4** — `git-bash --cd=<dir>` is the one catalog flag not verifiable from this repo's source. Keep
  the row; if the native checkpoint shows it does not start in the directory, **drop the row** rather
  than guessing another flag (failure mode: a terminal in the wrong directory, not a security issue).
* **OQ5** — `detail` (an absolute local path) crosses IPC **outbound** for the picker subtitle.
  Display-only, never accepted back, and under AMEND-2 it is the *only* way the browsed path reaches
  the UI at all. Recommend keeping it.

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
`"../../evil"`, `"\\server\share\x.exe"`, a 2000-char string, and a string containing U+202E:
`coerce_tool_id` ⇒ `""` **and** `picked(..)` ⇒ `None`. Include a path that **does exist** on the test
machine, to pin that existence never matters. Plus `"custom"` **with an empty stored path** ⇒ `""`,
and `"custom"` **with a stored path** ⇒ `"custom"`.
**AC6 — provenance invariant.** For every catalog entry, every fake `Resolution`, and every browsed
path, `LaunchSpec.program` is `entry.program`, `"open"`, or an absolute path that came from a probe or
the stored `custom_*_path`; every argv token is a catalog `&'static str`, a catalog prefix + the target
dir, or the target dir. No test may build a `LaunchSpec` from a free-text program — `template_spec` is
gone, so no constructor exists.
**AC7** Migration: one case per §5.3 invariant 1-5, plus `"code {path}"`→`vscode`,
`"wt -d {path}"`→`windows-terminal`, `"cmd /K"`→`cmd`, `"C:\Program Files\X\x.exe"`→`""`,
`"make"`→`""`; both-keys-present keeps the new one; after save→load→save the legacy keys are absent
from the JSON text; a file with neither loads `""`/`""`; **and after every migration case both
`custom_*_path` fields are still `""` and neither `*_tool` is `"custom"`.**
**AC8** Catalog integrity: ids unique per `(kind, os)`; **no catalog id equals `CUSTOM_ID`**; every
`AUTO_*` id resolves via `find`; **every `MacOpen` entry and every `MacApp`-referenced entry has
`Some(app_name)`**; `BuiltIn` is an entry's only rung; every `LEGACY_ALIASES` target exists in
`CATALOG`; `MacOpen` entries are `os == MacOs`.
**AC9** The **auto ladders are unchanged**: the pre-existing `external_tests.rs` assertions (`wt -d` /
`powershell` / `cmd /K`; `open -a Terminal`; `gnome-terminal --working-directory=` / `konsole
--workdir` / `x-terminal-emulator`; `code` / `code-insiders`; the macOS editor trio
`open -a "Visual Studio Code"` / `open -a "Visual Studio Code - Insiders"` / `code`) pass with no edit
other than the new parameter, and `only_the_directory_less_terminal_rungs_keep_the_repo_as_cwd` holds.
**AC10** `list_external_tools` returns both lists, both label maps and `scannedAtMs` in one round
trip, never rejects for detection state, runs under `spawn_blocking`; `refresh: true` re-probes
(`scannedAtMs` advances).
**AC11** `picked()` recheck by source: a `BuiltIn` selection (`cmd`, `powershell`) resolves **without**
any fs test; a `Path`/`WellKnown` selection whose file was removed ⇒ `None`; an `AppBundle` or
custom-bundle selection whose bundle was removed ⇒ `None`; a `custom` selection whose file lost its
exec bit (unix) or changed extension (Windows) ⇒ `None`.
**AC12 — AMEND-1 label maps.** `label_map(kind)` contains an entry for **every** `CATALOG` id of that
kind on **every** OS (so a macOS-authored settings file renders on Windows), and is
**well-defined**: no id maps to two different labels across OSes. A stale selection (`editorTool`
= an id absent from `editors`) is nameable from `editorLabels` — asserted, since this is the test that
would have caught the picker rendering `notepadpp`.
**AC13 — AMEND-3 `present`.** Every probe-derived row has `present: true`; with a stored
`custom_*_path` that resolves, the custom row is listed with `present: true`; with a stored path that
is **gone**, the row is still **listed** with `present: false` and `detail` = the stored path; with no
stored path, no custom row appears.
**AC14** Mock parity: `VITE_MOCK_IPC=1` serves all ten §6 seams with the payload shapes above. One
screenshot of the populated + empty states as the final visual proof (frugal-verification rule);
per-state UI assertions belong to `P112-ui.md` §13.
**AC15 — the §5.4 property, tested four ways.** (a) `UiSettingsPatch` has **no** field carrying a path
or program — asserted structurally (the struct's field list) so a later addition trips the test;
(b) a patch JSON containing `"customEditorPath": "C:\\payload.exe"` leaves the stored path
**unchanged** (unknown key, silently ignored) and returns `Ok`; (c) a patch of
`editorTool: "C:\payload.exe"` — *for a file that exists* — stores `""`; (d) `UiSettings` (the read
DTO) has no path field either, so no settings echo can carry one. Together: **no renderer-written path
is honoured, and none is even representable.**
**AC16 — `validate_custom_program(path, os)` table** (pure w.r.t. the OS param, no dialog):
non-existent ⇒ refused; UNC ⇒ refused; a plain directory ⇒ refused; a `.app` directory with
`Info.plist` ⇒ `MacBundle` (**`os = MacOs`**); a non-executable file on unix ⇒ refused; a path with
U+202E or a control char ⇒ refused; over `MAX_LEN` ⇒ refused. **DEC-1: with `os = Windows`,
`payload.cmd` / `payload.bat` / `payload.ps1` are refused even though they exist, and `Code.exe` is
accepted** — the check, not the dialog filter, is what the test pins. Errors are category-only and
never echo the path.
**AC17** Browsed-tool launch shapes: Editor+Executable ⇒ one argv token, `cwd == safe_cwd()`,
`hide_console`; Terminal+Executable ⇒ `args.is_empty()`, `cwd == target`; either+`MacBundle` ⇒
`open -a <bundle> <dir>` with `wait_for_exit`.
**AC18 — `pick_external_tool` write semantics.** Confirm ⇒ **both** `custom_*_path` and
`*_tool = "custom"` are set in **one** `settings::update` cycle (asserted by a concurrent
pick-vs-patch test that proves neither write is lost); cancel ⇒ `Ok(None)` and **no** mutation; a
refused path ⇒ error and **no** mutation (neither the path nor the selection). Reverting via
`{ editorTool: "" }` leaves `custom_editor_path` **intact**, and re-selecting `"custom"` restores the
tool — the reversibility AMEND-2 exists for.
**AC19** Frontend: picking an option patches exactly one key and nothing else; the row `↺` is
**absent** at `''` and patches `''`; `Browse…` calls `pickExternalTool(kind)` and **never patches**
(the backend owns that write); `null` (cancel) changes nothing; a stale id renders its label from the
map; `useUiSettings` round-trips both new keys; no `terminalCommand`/`editorCommand` identifier
remains in `src/`, `src-tauri/` or `e2e/` outside the §5.3 legacy fields and the alias table (grep is
the evidence). Also: the two picker rows use `Combobox` **without** `allowFreeInput`.
**AC20** An `#[ignore]`d host test (`cargo test -- --ignored`): `HostToolEnv` on this Windows box
detects at least `cmd` and `powershell` (both `BuiltIn`, so it cannot fail on a supported Windows), and
any `Path`/`Registry`/`WellKnown` hit is an **absolute existing** file. Real detection, no native
window.

### USER CHECKPOINT — must NOT be self-confirmed

**UC1** In `pnpm tauri dev` → Settings → External tools, the pickers list the tools actually installed.
Only the native run exercises the real registry / well-known / `PATH` rungs against a real install set.
**UC2** Picking a terminal and a non-VS-Code editor, then using "Open in terminal" / "Open in editor"
from the repo, worktree and submodule menus, launches **that** tool at **that** folder.
**UC3** Rescan reflects reality: install or remove a tool, press Rescan, the list changes.
**UC4** With a selection whose tool is then uninstalled, "Open in editor" still opens something (OQ1
silent fallback) and the picker keeps the selection, named, marked "Not installed".
**UC5** macOS: a `.app`-sourced editor and terminal both launch at the right folder. **May be WAIVED
for lack of a Mac** — say so explicitly rather than implying it passed; the `TargetOs::MacOs` branch
and bundle detection are unit-covered regardless (AC4/AC16).
**UC6 — §5.4 Browse, native by construction.** Press Browse, pick a real portable editor in the OS
dialog, confirm the row appears with the right label and that launching uses it. Then: Cancel the
dialog and confirm nothing changed; reset the row to Auto-detect with `↺` and confirm re-selecting the
browsed tool **restores it** (AMEND-2 reversibility); and on Windows confirm the dialog offers `*.exe`
only (DEC-1). **There is no dialog in the browser harness**, so none of this is orchestrator-verifiable
beyond the mock wiring. `P112-ui.md` UC-UI-1..3 (popover geometry, focus return, native dialog strings)
are the UI half.
**Not a checkpoint item — migration.** The ledger records both legacy values as **empty** in the user's
real `settings.json`, so a native run cannot demonstrate migration. AC7 is the whole proof.

---

## 11. UI contract — written; what binds the backend

`docs/contracts/P112-ui.md` is **complete** (rows `general.terminal-tool` / `general.editor-tool`, the
14 states, copy constants, geometry, a11y, the harness seams, UA1-UA20). This contract defers to it on
everything visual. The parts that constrain **this** spec, already absorbed above:

* `Combobox` in **strict mode** — the frontend half of the no-free-text property (header note, §7).
* **AMEND-1 / AMEND-2 / AMEND-3** are prerequisites, not preferences: §2, §5.1 and §6.
* **DEC-1** narrows the browse filter *and* (per the §5.4 correction) the backend check.
* **DEC-2 / DEC-3** keep `source` and `scannedAtMs` in the DTO but out of the UI.
* There is **no Remove control and no "forget path" command** — reset is `{ tool: "" }` and the path
  survives (§5.4 item 5). An earlier draft of this contract mentioned a Remove affordance; that is
  void.
