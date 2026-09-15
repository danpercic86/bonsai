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
//! | [`custom`] | the browsed path: [`BrowsedProgram`], validation, label, recipe |
//! | `settings_ids` | the two PURE settings fns: [`coerce_tool_id`], [`legacy_tool_id`] (plus [`legacy_tool_stem`], diagnostics only) |
//! | `scan_cache` | the process-wide probe cache + the one-probe-at-a-time rule |
//! | this file | the DTOs, [`tool_scan`], [`picked`] |
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

use crate::external::TargetOs;

pub mod catalog;
mod catalog_table;
pub mod custom;
pub mod detect;
mod scan_cache;
mod settings_ids;

#[cfg(test)]
mod fake;

#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod custom_tests;
#[cfg(test)]
mod detect_tests;
#[cfg(test)]
mod no_spawn_tests;
#[cfg(test)]
mod scan_tests;
#[cfg(test)]
mod settings_ids_tests;

pub use catalog::{Recipe, ToolEntry, CUSTOM_ID};
pub use custom::{synthesize_recipe, validate_custom_program, BrowsedProgram, CustomKindShape};
pub use detect::{scan_for, HostToolEnv, ToolEnv};
pub use settings_ids::{coerce_tool_id, legacy_tool_id, legacy_tool_stem};

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
///
/// `Serialize` only, deliberately: it rides along inside [`DetectedTool`], and
/// the backend never accepts one back. The inbound vocabulary is [`ToolKind`]
/// and a catalog id — nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
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
///
/// **AC6's invariant, compiler-enforced since 2026-09-15: no code outside
/// `bonsai-core` constructs a `PickedTool`.** Every field is `pub(crate)`, so
/// the only ways to obtain one are [`picked`] — which revalidates the selection
/// against the filesystem — and `external`'s auto arm, which reads the static
/// catalog. The PRODUCTION literal sites are [`picked_custom`] here,
/// `detect::resolution_to_picked` (the catalog arm of [`picked`] — missing from
/// this list until 2026-09-15) and `external::auto_ladder`; in-crate test modules
/// build literals freely, which is the point of keeping the type constructible
/// at all.
///
/// It was NOT true while the fields were `pub`, and the gap was invisible
/// because it sat one type away from where it was argued: `external::spec_from`
/// is `pub(crate)` on the reasoning that a `pub` spec builder over a
/// public-fielded `PickedTool` would be the "arbitrary program ⇒ `LaunchSpec`"
/// primitive AC6 deletes — while `external::{terminal_ladder, editor_ladder,
/// open_in_terminal, open_in_editor}` are all `pub` and all take
/// `Option<&PickedTool>`, so a hand-built literal from another crate reached
/// `launch_first` through any of the four. One door closed beside four open
/// ones. The fields, not the builder's visibility, are what shuts them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedTool {
    pub(crate) kind: ToolKind,
    /// The catalog entry's recipe, [`Recipe::MacOpen`] for any bundle
    /// resolution, or the synthesized recipe for a browsed tool.
    pub(crate) recipe: Recipe,
    /// An absolute path, the catalog name for [`ToolSource::BuiltIn`], or
    /// `"open"` when `recipe == MacOpen`.
    pub(crate) program: String,
    /// The `open -a` argument. `Some` **iff** `recipe == MacOpen`: the resolved
    /// bundle PATH for a bundle, else the entry's `app_name`.
    pub(crate) open_arg: Option<String>,
    pub(crate) source: ToolSource,
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

/// Detected tools + the remembered browsed row + the label maps.
///
/// BLOCKING (filesystem, and `reg.exe` on Windows) — call under
/// `spawn_blocking`. `custom_terminal` / `custom_editor` are the stored browsed
/// paths ([`BrowsedProgram::from_settings_field`]`("")` = none), typed so a
/// renderer-supplied string cannot reach here.
pub fn tool_scan(
    custom_terminal: BrowsedProgram,
    custom_editor: BrowsedProgram,
) -> ExternalToolScan {
    let (at_ms, rows) = scan_cache::cached_rows();
    scan_from_rows(
        &rows,
        custom_terminal.as_str(),
        custom_editor.as_str(),
        TargetOs::host(),
        at_ms,
    )
}

/// [`tool_scan`] with a forced re-probe (the picker's Rescan): `scanned_at_ms`
/// advances and a tool installed since the last scan appears.
///
/// Concurrent refreshes **coalesce**: the first caller probes and the others
/// wait for its result, so N simultaneous Rescans cost one `reg.exe` sweep and
/// not N (audit LOW-2 — see `scan_cache`). Every caller still gets the fresh
/// rows and the fresh `scanned_at_ms`, **except** on the two paths
/// `scan_cache::ScanCell::probe` enumerates: a leader that overruns its wait
/// hands followers the stale rows, and a leader that panics makes them re-probe.
pub fn refresh_tool_scan(
    custom_terminal: BrowsedProgram,
    custom_editor: BrowsedProgram,
) -> ExternalToolScan {
    let (at_ms, rows) = scan_cache::probe_host();
    scan_from_rows(
        &rows,
        custom_terminal.as_str(),
        custom_editor.as_str(),
        TargetOs::host(),
        at_ms,
    )
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
        // The stored path resolving is exactly `present`: a path that changed
        // shape (lost its execute bit, gained a different extension) is reported
        // gone rather than offered.
        let present = custom::validate_custom_program(Path::new(custom_path), os).is_ok();
        out.push(custom_row(kind, custom_path, present));
    }
    out
}

/// The resolved absolute path, or the literal `"built in"`.
///
/// Sanitized UNCONDITIONALLY, and this is not a browsed-row special case: a
/// probe-derived path never passes through [`validate_custom_program`], and
/// `detect::executable_hit` performs no character check — so a PATH directory
/// whose *name* carries a bidi override would otherwise produce a row whose
/// label is trustworthy (the static catalog) but whose subtitle reads as a
/// different path than the one that launches. A path without those characters
/// is byte-identical, so nothing normal changes.
fn detail_of(res: &Resolution) -> String {
    match (&res.source, &res.bundle) {
        (ToolSource::BuiltIn, _) => BUILT_IN_DETAIL.to_string(),
        (_, Some(bundle)) => custom::sanitize_detail(bundle),
        (_, None) => custom::sanitize_detail(&res.program),
    }
}

/// The browsed row, for both of its producers: the remembered path in a scan
/// (`present` = "the stored path still validates") and [`browsed_tool_row`]
/// (`present = true` by construction — it just validated).
///
/// ONE constructor on purpose: the row the picker lists after a Browse and the
/// row the next scan lists for the same path must be the same row, or the UI
/// would flicker between two spellings of one tool.
fn custom_row(kind: ToolKind, custom_path: &str, present: bool) -> DetectedTool {
    let path = Path::new(custom_path);
    DetectedTool {
        id: CUSTOM_ID.to_string(),
        label: custom::display_label(path),
        kind,
        source: ToolSource::Custom,
        // Sanitized exactly like every other row's detail (`detail_of`) — the
        // treatment is unconditional. It matters MOST here: this is the one row
        // displayed DESPITE failing validation (a hand-edited settings.json, or
        // a path that is simply gone), so it is the only detail string that
        // reaches the UI without validation having refused those characters
        // first. The length cap can fire on a probe-derived row too — nothing
        // bounds a `PATH`-derived program path's length — it just cannot fire
        // on one validation ACCEPTED (`MAX_LEN` is 512 bytes, hence <= 512
        // chars).
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
///
/// `setting` is renderer-writable (it is only ever a lookup key); `custom_path`
/// is NOT, which is why it is a [`BrowsedProgram`] and not a `&str`.
pub fn picked(setting: &str, kind: ToolKind, custom_path: BrowsedProgram) -> Option<PickedTool> {
    let (_, rows) = scan_cache::cached_rows();
    picked_from(
        &HostToolEnv::new(),
        TargetOs::host(),
        &rows,
        setting,
        kind,
        custom_path.as_str(),
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

/// Validate a path the **native dialog the backend opened** just returned and
/// turn it into the picker row for it (P112 §5.4 / §6).
///
/// The command layer calls exactly this and then writes the path: keeping both
/// the validation and the row shape in this crate is what stops the command
/// layer from hand-building a `DetectedTool` that disagrees with the one the
/// next [`tool_scan`] produces for the same path.
///
/// `Err` is the category-only refusal (`custom::refuse`) — it never echoes the
/// path — and the caller MUST write nothing on it: not the path, and not the
/// selection.
///
/// BLOCKING (existence, the bundle check, the unix execute bit) — and, since
/// ruling #26 admits UNC on this path, potentially a full SMB timeout on a
/// disconnected share. Call it under `spawn_blocking`.
pub fn browsed_tool_row(
    kind: ToolKind,
    path: &Path,
    os: TargetOs,
) -> Result<DetectedTool, crate::error::AppError> {
    custom::validate_custom_program(path, os)?;
    Ok(custom_row(kind, &path.to_string_lossy(), true))
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
