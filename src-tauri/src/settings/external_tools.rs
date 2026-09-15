//! P112 §5.3 — the one-shot migration of the deleted free-text
//! `terminalCommand` / `editorCommand` settings, plus the reasoning behind the
//! six fields it moves between.
//!
//! ## What P112 changed, and why it is not "validation"
//!
//! `terminalCommand` / `editorCommand` were renderer-writable **program
//! strings**: a value the renderer could write was a program the renderer could
//! cause to be executed (the 2026-09-11 audit found three routes that survived
//! shape validation). They are replaced by
//!
//! * `terminalTool` / `editorTool` — renderer-writable, but only ever a LOOKUP
//!   KEY into a compile-time catalog, coerced on write by
//!   [`bonsai_core::tools::coerce_tool_id`] (`commands::ui_settings::apply_patch`);
//! * `customTerminalPath` / `customEditorPath` — the browsed path, written
//!   **only** by `pick_external_tool` from a native dialog the BACKEND opens,
//!   and **absent from both `UiSettings` and `UiSettingsPatch`**. A
//!   renderer-written path is therefore *unrepresentable*, not merely rejected.
//!
//! The two halves are separate fields on purpose: encoding the path into the id
//! (`"custom:C:\…"`) would destroy it the moment another tool was selected, and
//! would round-trip the path through the renderer on every settings echo.
//! Splitting them makes "revert to Auto-detect" reversible (the path survives a
//! `{ editorTool: "" }` patch, and re-selecting `"custom"` restores it).
//!
//! ## No version bump
//!
//! `SETTINGS_VERSION` stays `1`. Four additive `#[serde(default)]` fields plus
//! the removal of two keys **with a safe default and a migration** are below
//! the bar the `Settings` wire-format doc documents for a bump: a file written
//! by an older build loads, migrates, and keeps working.
//!
//! ## Idempotence
//!
//! The legacy fields survive on [`Settings`] as migration INPUT only and carry
//! `#[serde(skip_serializing)]`, so the first ordinary `update` writes a file
//! without them. Until then the migration runs on every `load_from` and is a
//! no-op after the first, because a non-empty new key always wins and the
//! legacy fields are cleared in memory regardless.

use bonsai_core::tools::{self, ToolKind};

use super::Settings;

/// Maps a legacy free-text command onto a catalog id, then clears it (P112
/// §5.3). Pure and in-memory: called from `load_from`, it never writes.
///
/// The invariants, each of which is a test:
///
/// 1. The output is a catalog id or `""` — never a program string, never a
///    path, **never `"custom"`, and never a `custom_*_path`**. Migration cannot
///    manufacture the human dialog click §5.4 requires; a legacy
///    `C:\Tools\npp\notepad++.exe` becoming `custom_editor_path` would
///    reintroduce exactly the capability P112 deletes.
/// 2. Misses are accepted, not preserved (`"make"` ⇒ `""`, and the user re-adds
///    an unlisted tool with one Browse click).
/// 3. A pre-existing non-empty new key always wins over the legacy key.
/// 4. Idempotent: running it again on its own output changes nothing.
pub fn migrate_external_tools(s: &mut Settings) {
    if s.terminal_tool.is_empty() && !s.terminal_command.trim().is_empty() {
        s.terminal_tool = tools::legacy_tool_id(&s.terminal_command, ToolKind::Terminal);
        note_if_dropped(ToolKind::Terminal, &s.terminal_command, &s.terminal_tool);
    }
    if s.editor_tool.is_empty() && !s.editor_command.trim().is_empty() {
        s.editor_tool = tools::legacy_tool_id(&s.editor_command, ToolKind::Editor);
        note_if_dropped(ToolKind::Editor, &s.editor_command, &s.editor_tool);
    }
    // Never carried further, whether or not anything mapped — the legacy value
    // must not reach a launch site again.
    s.terminal_command.clear();
    s.editor_command.clear();
}

/// The **only** writer of `custom_terminal_path` / `custom_editor_path` and of
/// the `"custom"` selection that names them (P112 §5.4 item 2 / AC18).
///
/// Both halves in ONE mutator, so the caller's single `settings::update` cycle
/// sets both under the `SETTINGS_IO` mutex. Two cycles could leave
/// `*_tool == "custom"` with an empty path — a state `coerce_tool_id` would then
/// scrub on the next unrelated patch, silently reverting the user's pick.
///
/// `path` must already have passed `tools::validate_custom_program`: this
/// function writes, it does not judge. Its caller (`commands::tools`) validates
/// first and writes nothing on a refusal.
///
/// There is deliberately no "forget the path" counterpart. Reverting to
/// Auto-detect is the ordinary `{ editorTool: "" }` patch, which leaves the path
/// intact so re-selecting `"custom"` restores the tool (§5.1 reversibility).
pub fn set_browsed_tool(s: &mut Settings, kind: ToolKind, path: &str) {
    match kind {
        ToolKind::Terminal => {
            s.custom_terminal_path = path.to_string();
            s.terminal_tool = tools::CUSTOM_ID.to_string();
        }
        ToolKind::Editor => {
            s.custom_editor_path = path.to_string();
            s.editor_tool = tools::CUSTOM_ID.to_string();
        }
    }
}

/// Invariant 2's data loss, made VISIBLE — one line, on the miss branch only.
///
/// A user whose `editorCommand` was a bare `myed` (which worked pre-P112) gets
/// the auto ladder on the first launch after upgrading, and the loss becomes
/// permanent at the next settings write of any kind. The contract sanctions
/// that and recovery is one Browse click, but until this line existed nothing
/// anywhere said it had happened.
///
/// **What is logged is the DERIVED STEM, never the stored command.** The stem
/// is what [`tools::legacy_tool_id`] looked up — first token, file stem,
/// lowercased — so the directories and the arguments are already gone
/// (`C:\Tools\npp\notepad++.exe -multiInst` ⇒ `notepad++`) and a user's install
/// path cannot reach a log through here.
///
/// `eprintln!` is this crate's facade for non-fatal diagnostics
/// (`commands::repo`, `lib.rs`, `commands::ui_settings`); no workspace crate
/// depends on `tracing`, and the Dev-mode `obs` sink is not an option: it is
/// configured FROM the settings this runs inside `load_from` for, so it cannot
/// be running yet.
fn note_if_dropped(kind: ToolKind, legacy: &str, mapped: &str) {
    if !mapped.is_empty() {
        return;
    }
    let stem = tools::legacy_tool_stem(legacy);
    eprintln!(
        "bonsai: the pre-P112 {kind:?} command named {stem:?}, which is not a known tool — \
         falling back to auto-detect (choose it under Settings → External tools)"
    );
}

#[cfg(test)]
#[path = "external_tools_tests.rs"]
mod tests;
