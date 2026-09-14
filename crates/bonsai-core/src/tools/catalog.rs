//! Catalog *shape*: the types a candidate row is made of, plus the lookups over
//! the table. The table itself — every literal — lives in
//! [`super::catalog_table`] (P112 §1: "static data only", the house
//! `fixtures/*` separation).
//!
//! Nothing here touches the filesystem, the registry, or a process: a catalog
//! row is a *description* of where a tool might be, and [`super::detect`] is
//! the only place that goes looking.

use crate::external::TargetOs;

use super::catalog_table::{
    AUTO_EDITOR_LINUX, AUTO_EDITOR_MAC, AUTO_EDITOR_WIN, AUTO_TERMINAL_LINUX, AUTO_TERMINAL_MAC,
    AUTO_TERMINAL_WIN,
};
use super::ToolKind;

/// The table lives in `catalog_table` (data) and is reached through here
/// (shape + lookups), so no caller needs to know about the split.
pub use super::catalog_table::{CATALOG, LEGACY_ALIASES};

/// The reserved pseudo-id for the browsed (native-dialog) tool. No catalog id
/// may equal it (AC8) — a collision would let a detected tool impersonate the
/// browsed-path row, which is the one row whose target is not from this table.
pub const CUSTOM_ID: &str = "custom";

/// Windows App Paths, the one documented "where is this program installed"
/// registry mechanism. Joined with the hive prefix and the exe name by
/// [`super::detect::probe_entry`].
pub const APP_PATHS_SUBKEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";

/// One candidate tool on one OS.
///
/// `id` is unique per `(kind, os)`, **not** globally: the same tool on three
/// OSes shares one id so a settings file syncs between machines.
#[derive(Debug, PartialEq, Eq)]
pub struct ToolEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ToolKind,
    pub os: TargetOs,
    /// The bare EXECUTABLE name — what [`Rung::OnPath`] looks up and what an
    /// `AutoVia::Name` rung launches. For a row whose every rung is a macOS
    /// bundle this is `"open"`, because that IS what launches it
    /// ([`Recipe::MacOpen`]); it is not a placeholder.
    pub program: &'static str,
    /// The macOS `open -a` APP NAME. NEVER conflated with [`Self::program`]:
    /// the mac `vscode` row has `program = "code"` but
    /// `app_name = Some("Visual Studio Code")`, and the auto ladder needs the
    /// latter — `open -a code` is a different (and usually failing) launch.
    pub app_name: Option<&'static str>,
    /// Probe ladder, in order; the first hit wins.
    pub rungs: &'static [Rung],
    /// For EXECUTABLE resolutions. Any *bundle* resolution is normalised to
    /// [`Recipe::MacOpen`] by [`super::picked`] instead.
    pub recipe: Recipe,
}

/// One rung of a probe ladder. Every variant degrades to `None` on any failure,
/// so a wrong candidate fails to detect and can never mis-detect.
#[derive(Debug, PartialEq, Eq)]
pub enum Rung {
    /// Present by definition on this OS ⇒ [`ToolEntry::program`], touches
    /// nothing. MUST be an entry's only rung (AC8) — a fallback after it would
    /// be unreachable.
    BuiltIn,
    /// `PATH` (+`PATHEXT` on Windows) lookup of [`ToolEntry::program`].
    OnPath,
    /// Windows App Paths (HKCU then HKLM), default value, quotes trimmed.
    AppPaths { exe: &'static str },
    /// `%var%` + a backslash-relative suffix, then `is_file`.
    WinFolder {
        var: &'static str,
        suffix: &'static str,
    },
    /// macOS `.app` bundle. `home: true` ⇒ `path` is relative to `$HOME`.
    Bundle { path: &'static str, home: bool },
    /// Absolute unix candidate: `is_file` **and** at least one execute bit.
    UnixFile { path: &'static str },
}

/// How a resolved tool is handed the target directory.
///
/// `Copy` because [`super::PickedTool`] carries one by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipe {
    /// `<program> [fixed…] <dir>` — the dir is the LAST argv token.
    DirLastArg(&'static [&'static str]),
    /// `<program> [fixed…] <prefix><dir>` — prefix and dir in ONE token.
    DirJoinedArg(&'static [&'static str], &'static str),
    /// `<program> [fixed…]`, NO dir token; the cwd IS the dir. Shells only.
    DirCwd(&'static [&'static str]),
    /// `open -a <open_arg> <dir>`.
    MacOpen,
}

/// One rung of an **auto** ladder (the `""` setting): an id plus how to launch
/// it. Ids only — every string still comes from the referenced [`ToolEntry`],
/// so there is no second program table to drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoRung {
    pub id: &'static str,
    pub via: AutoVia,
}

/// `Name` ⇒ [`ToolEntry::program`] + its recipe; `MacApp` ⇒
/// `open -a` [`ToolEntry::app_name`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoVia {
    Name,
    MacApp,
}

/// Exact `(kind, os, id)` lookup.
///
/// This — not [`find`] — is what an OS-explicit caller (the auto ladder, which
/// takes a [`TargetOs`] param) must use: the mac and Windows `vscode` rows
/// share an id but only the mac one has an `app_name`, so a host-relative
/// lookup would hand a Windows host the wrong row while building the macOS
/// ladder.
pub fn find_for(kind: ToolKind, id: &str, os: TargetOs) -> Option<&'static ToolEntry> {
    if id.is_empty() {
        return None;
    }
    CATALOG
        .iter()
        .find(|e| e.kind == kind && e.os == os && e.id == id)
}

/// Host-OS row first, then ANY OS.
///
/// The any-OS fallback is deliberate: a settings file synced from another
/// machine names an id this host may not have a row for, and both
/// `coerce_tool_id` (keep the selection) and [`super::label_map`] (name it)
/// must still resolve it. Detection is unaffected — a foreign-OS row's rungs
/// simply never hit here.
pub fn find(kind: ToolKind, id: &str) -> Option<&'static ToolEntry> {
    find_for(kind, id, TargetOs::host())
        .or_else(|| CATALOG.iter().find(|e| e.kind == kind && e.id == id))
}

/// Every row of `kind` for `os`, in table order (which is probe order).
pub fn entries_for(kind: ToolKind, os: TargetOs) -> impl Iterator<Item = &'static ToolEntry> {
    CATALOG.iter().filter(move |e| e.kind == kind && e.os == os)
}

/// Every row of `kind`, all OSes — the source of [`super::label_map`].
pub fn entries_any_os(kind: ToolKind) -> impl Iterator<Item = &'static ToolEntry> {
    CATALOG.iter().filter(move |e| e.kind == kind)
}

/// The auto ladder for `(kind, os)` — byte-identical in effect to the ladders
/// hardcoded in `external.rs` today (AC9).
pub fn auto_rungs(kind: ToolKind, os: TargetOs) -> &'static [AutoRung] {
    match (kind, os) {
        (ToolKind::Terminal, TargetOs::Windows) => AUTO_TERMINAL_WIN,
        (ToolKind::Terminal, TargetOs::MacOs) => AUTO_TERMINAL_MAC,
        (ToolKind::Terminal, TargetOs::Linux) => AUTO_TERMINAL_LINUX,
        (ToolKind::Editor, TargetOs::Windows) => AUTO_EDITOR_WIN,
        (ToolKind::Editor, TargetOs::MacOs) => AUTO_EDITOR_MAC,
        (ToolKind::Editor, TargetOs::Linux) => AUTO_EDITOR_LINUX,
    }
}
