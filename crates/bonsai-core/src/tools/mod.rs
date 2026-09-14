//! External-tool **detection** (P112): which terminals and editors are actually
//! installed, and which one a setting selects.
//!
//! ## Why this module exists
//!
//! It replaces user-supplied launch command templates. A `terminalCommand` /
//! `editorCommand` string the renderer could write was a program the renderer
//! could cause to be executed (audit 2026-09-11: three surviving routes
//! survived shape validation). Here, **a value the renderer can write is only
//! ever a lookup key**: an id into a compile-time catalog. The one
//! user-supplied path comes from a native dialog *the backend opens itself*
//! ([`custom`]), and is stored in a settings field the patch type has no way to
//! carry.
//!
//! ## Shape
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`catalog`] | the row types + the lookups |
//! | `catalog_table` | the static candidate table (data only) |
//! | [`detect`] | [`ToolEnv`], the probe ladders, the host prober |
//! | [`custom`] | the browsed path: validation, label, recipe |
//! | this file | the DTOs, the process-wide scan cache, [`tool_scan`], [`picked`] |
//!
//! Detection is **lazy and explicit**: nothing scans at boot, the scan is
//! cached for the process lifetime, and the only refresh is the picker's
//! Rescan. Detection is not repo state, so there is no watcher and no
//! focus-rescan.
//!
//! Everything public here except [`label_map`] and the [`custom`] helpers is
//! **blocking** (filesystem, and `reg.exe` on Windows) and belongs inside
//! `spawn_blocking`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::external::TargetOs;

pub mod catalog;
mod catalog_table;
pub mod custom;
pub mod detect;

#[cfg(test)]
mod fake;

#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod custom_tests;
#[cfg(test)]
mod detect_tests;
#[cfg(test)]
mod scan_tests;

pub use catalog::{Recipe, ToolEntry, CUSTOM_ID};
pub use custom::{synthesize_recipe, validate_custom_program, CustomKindShape};
pub use detect::{scan_for, HostToolEnv, ToolEnv};

/// Which of the two configurable tool slots. The file manager is deliberately
/// **not** configurable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolKind {
    Terminal,
    Editor,
}

/// Where a resolution came from. Not surfaced in the UI (DEC-2); it is the
/// provenance record that drives the launch-time recheck and the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolSource {
    BuiltIn,
    Path,
    Registry,
    WellKnown,
    AppBundle,
    /// The browsed path (§5.4). The wire value is `"custom"`, matching
    /// [`CUSTOM_ID`], so there is ONE vocabulary for this concept.
    Custom,
}

/// What a successful probe produced.
///
/// `program` / `bundle` are ALWAYS a catalog `&'static str`, an absolute path
/// this crate built from a probe, or the stored browsed path — never a string
/// the renderer supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// Absolute path; the catalog name for [`ToolSource::BuiltIn`]; `"open"`
    /// for a bundle.
    pub program: String,
    /// The macOS `.app` directory. `Some` ⇔ a bundle resolution.
    pub bundle: Option<String>,
    pub source: ToolSource,
}

/// A resolved selection — the ONLY thing a launch path accepts besides `None`
/// (= the auto ladder).
///
/// Deliberately not `&'static ToolEntry`: a browsed tool has no catalog entry,
/// and folding both into one shape is what stops the two launch paths from
/// drifting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedTool {
    pub kind: ToolKind,
    /// The catalog entry's recipe, [`Recipe::MacOpen`] for any bundle
    /// resolution, or the synthesized recipe for a browsed tool.
    pub recipe: Recipe,
    /// An absolute path, the catalog name for [`ToolSource::BuiltIn`], or
    /// `"open"` when `recipe == MacOpen`.
    pub program: String,
    /// The `open -a` argument. `Some` **iff** `recipe == MacOpen`: the resolved
    /// bundle PATH for a bundle, else the entry's `app_name`.
    pub open_arg: Option<String>,
    pub source: ToolSource,
}

/// IPC DTO — **display only**. The backend never accepts `label` / `detail` /
/// `source` / `present` back; the frontend may send back only `id`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTool {
    /// A catalog id, or the pseudo-id [`CUSTOM_ID`].
    pub id: String,
    pub label: String,
    pub kind: ToolKind,
    pub source: ToolSource,
    /// EXACTLY: the resolved absolute path for Path / Registry / WellKnown /
    /// AppBundle / Custom; the literal `"built in"` for BuiltIn.
    pub detail: String,
    /// Did the target resolve at scan time? Always `true` for a probe-derived
    /// row (it was just probed). `false` only for the remembered `custom` row
    /// whose stored path is gone — which is still LISTED, so the UI can render
    /// the "custom path gone" state instead of an unexplained empty picker.
    pub present: bool,
}

/// One round trip's worth of picker data.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalToolScan {
    /// Detected terminals in table order, then the remembered custom row.
    pub terminals: Vec<DetectedTool>,
    pub editors: Vec<DetectedTool>,
    /// `id -> label` for EVERY catalog entry of this kind on EVERY OS. Without
    /// it a kept-but-undetected selection renders as the raw id — the picker
    /// would literally read `notepadpp`.
    pub terminal_labels: BTreeMap<String, String>,
    pub editor_labels: BTreeMap<String, String>,
    /// Freshness identity the frontend compares to know a refresh landed.
    /// Never displayed (DEC-3) — the app has no time formatter.
    pub scanned_at_ms: u64,
}

/// The `detail` string for a `BuiltIn` row: it has no path, because it is on
/// every machine of that OS by definition.
const BUILT_IN_DETAIL: &str = "built in";

/// Probe results for the host, cached for the process lifetime.
///
/// `RwLock<Option<_>>` rather than `OnceLock`, and poison-recovering, for the
/// `gitbin::GIT_BIN` reason: "install the editor, press Rescan" must work
/// without restarting the app. Only *probe* results are cached — the custom row
/// is one stat and the label maps are static data, so both are derived per
/// call.
static SCAN: RwLock<Option<CachedScan>> = RwLock::new(None);

struct CachedScan {
    at_ms: u64,
    found: Vec<(&'static ToolEntry, Resolution)>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Poison-recovering read of the cache; a miss probes the host once.
fn cached_rows() -> (u64, Vec<(&'static ToolEntry, Resolution)>) {
    {
        let guard = SCAN.read().unwrap_or_else(|p| p.into_inner());
        if let Some(cached) = guard.as_ref() {
            return (cached.at_ms, cached.found.clone());
        }
    }
    probe_host()
}

/// Probe the host and replace the cache. The ONLY place production code scans.
fn probe_host() -> (u64, Vec<(&'static ToolEntry, Resolution)>) {
    let found = detect::scan_for(&HostToolEnv::new(), TargetOs::host());
    let at_ms = now_ms();
    let mut guard = SCAN.write().unwrap_or_else(|p| p.into_inner());
    *guard = Some(CachedScan {
        at_ms,
        found: found.clone(),
    });
    (at_ms, found)
}

/// Detected tools + the remembered browsed row + the label maps.
///
/// BLOCKING (filesystem, and `reg.exe` on Windows) — call under
/// `spawn_blocking`. `custom_terminal` / `custom_editor` are the stored browsed
/// paths (`""` = none).
pub fn tool_scan(custom_terminal: &str, custom_editor: &str) -> ExternalToolScan {
    let (at_ms, rows) = cached_rows();
    scan_from_rows(&rows, custom_terminal, custom_editor, TargetOs::host(), at_ms)
}

/// [`tool_scan`] with a forced re-probe (the picker's Rescan): `scanned_at_ms`
/// advances and a tool installed since the last scan appears.
pub fn refresh_tool_scan(custom_terminal: &str, custom_editor: &str) -> ExternalToolScan {
    let (at_ms, rows) = probe_host();
    scan_from_rows(&rows, custom_terminal, custom_editor, TargetOs::host(), at_ms)
}

/// The pure assembly half of [`tool_scan`]: probe rows in, DTO out.
///
/// Split out so every scan-shape assertion runs against fake rows on one
/// machine — a test must never call [`tool_scan`], which would populate the
/// process-global cache and spawn `reg.exe`. The only filesystem contact left
/// here is the custom row's one validation stat.
pub(crate) fn scan_from_rows(
    rows: &[(&'static ToolEntry, Resolution)],
    custom_terminal: &str,
    custom_editor: &str,
    os: TargetOs,
    at_ms: u64,
) -> ExternalToolScan {
    ExternalToolScan {
        terminals: kind_rows(rows, ToolKind::Terminal, custom_terminal, os),
        editors: kind_rows(rows, ToolKind::Editor, custom_editor, os),
        terminal_labels: label_map(ToolKind::Terminal),
        editor_labels: label_map(ToolKind::Editor),
        scanned_at_ms: at_ms,
    }
}

/// Probe-derived rows of one kind, then the remembered custom row if a path is
/// stored (LISTED even when it no longer resolves — that is what `present` is
/// for).
fn kind_rows(
    rows: &[(&'static ToolEntry, Resolution)],
    kind: ToolKind,
    custom_path: &str,
    os: TargetOs,
) -> Vec<DetectedTool> {
    let mut out: Vec<DetectedTool> = rows
        .iter()
        .filter(|(e, _)| e.kind == kind)
        .map(|(e, res)| DetectedTool {
            id: e.id.to_string(),
            label: e.label.to_string(),
            kind,
            source: res.source,
            detail: detail_of(res),
            // It was just probed, so it resolved by construction.
            present: true,
        })
        .collect();
    if !custom_path.is_empty() {
        out.push(custom_row(kind, custom_path, os));
    }
    out
}

/// The resolved absolute path, or the literal `"built in"`.
fn detail_of(res: &Resolution) -> String {
    match (&res.source, &res.bundle) {
        (ToolSource::BuiltIn, _) => BUILT_IN_DETAIL.to_string(),
        (_, Some(bundle)) => bundle.clone(),
        (_, None) => res.program.clone(),
    }
}

/// The remembered browsed row. `present` reports whether the stored path still
/// validates — a path that changed shape (lost its execute bit, gained a
/// different extension) is reported gone rather than offered.
fn custom_row(kind: ToolKind, custom_path: &str, os: TargetOs) -> DetectedTool {
    let path = Path::new(custom_path);
    let present = custom::validate_custom_program(path, os).is_ok();
    DetectedTool {
        id: CUSTOM_ID.to_string(),
        label: custom::display_label(path),
        kind,
        source: ToolSource::Custom,
        // A stored path that FAILS validation is still displayed here, so it is
        // sanitized like the label: validation is what normally refuses bidi
        // overrides, and this is the one row that is shown despite failing it
        // (a hand-edited settings.json). Any path that validates is verbatim.
        detail: custom::sanitize_detail(custom_path),
        present,
    }
}

/// Resolve a stored setting to a launch-ready selection, or `None` ⇒ the auto
/// ladder.
///
/// `None` for: `""`, an unknown id, a wrong-kind id, `"custom"` with an empty or
/// no-longer-valid stored path, and a known id whose target no longer exists.
/// A miss is silent by design (the OQ1 ruling: no error toast).
///
/// BLOCKING, but cheap: it reads the process cache (populating it on first use)
/// and does **one** recheck. It never re-runs the ladder — an `AppPaths` rung
/// spawns a process, and a launch must not.
pub fn picked(setting: &str, kind: ToolKind, custom_path: &str) -> Option<PickedTool> {
    let (_, rows) = cached_rows();
    picked_from(
        &HostToolEnv::new(),
        TargetOs::host(),
        &rows,
        setting,
        kind,
        custom_path,
    )
}

/// The env/OS/rows-explicit half of [`picked`], so the recheck is testable on
/// one machine without touching the process cache.
pub(crate) fn picked_from(
    env: &dyn ToolEnv,
    os: TargetOs,
    rows: &[(&'static ToolEntry, Resolution)],
    setting: &str,
    kind: ToolKind,
    custom_path: &str,
) -> Option<PickedTool> {
    if setting.is_empty() {
        return None;
    }
    if setting == CUSTOM_ID {
        return picked_custom(kind, custom_path, os);
    }
    // Exact-OS: a foreign-OS id is nameable (see `catalog::find`) but never
    // launchable, because nothing on this host probed it.
    let entry = catalog::find_for(kind, setting, os)?;
    let (_, res) = rows
        .iter()
        // Pointer identity: `(kind, os, id)` is unique, and the rows hold the
        // very entries `find_for` returns.
        .find(|(e, _)| std::ptr::eq(*e, entry))?;
    detect::still_present(env, res).then(|| detect::resolution_to_picked(kind, entry, res))
}

/// The browsed selection: revalidated on every launch (cheap — one stat, or the
/// bundle check) so a path that changed shape is refused rather than launched.
fn picked_custom(kind: ToolKind, custom_path: &str, os: TargetOs) -> Option<PickedTool> {
    if custom_path.is_empty() {
        return None;
    }
    let shape = custom::validate_custom_program(Path::new(custom_path), os).ok()?;
    let recipe = custom::synthesize_recipe(kind, shape);
    Some(match shape {
        CustomKindShape::MacBundle => PickedTool {
            kind,
            recipe,
            program: "open".to_string(),
            open_arg: Some(custom_path.to_string()),
            source: ToolSource::Custom,
        },
        CustomKindShape::Executable => PickedTool {
            kind,
            recipe,
            program: custom_path.to_string(),
            open_arg: None,
            source: ToolSource::Custom,
        },
    })
}

/// `id -> label` for every catalog entry of `kind`, on EVERY OS.
///
/// PURE. All OSes on purpose: a settings file synced from another machine names
/// an id this host has no row for, and the picker must still be able to name it
/// ("Notepad++ — not installed", never `notepadpp`).
pub fn label_map(kind: ToolKind) -> BTreeMap<String, String> {
    catalog::entries_any_os(kind)
        .map(|e| (e.id.to_string(), e.label.to_string()))
        .collect()
}
