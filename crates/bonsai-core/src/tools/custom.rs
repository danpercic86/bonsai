//! The browsed tool (P112 §5.4): validating a path a **native dialog the
//! backend opened** returned, and deriving its launch recipe and its label.
//!
//! **Stated honestly: this is not a safety check on the program.** A browsed
//! `.exe` is arbitrary code. The security property P112 buys is "a human chose
//! it in a native dialog the backend opened" — enforced by the *types*
//! (`UiSettings`/`UiSettingsPatch` have no field that can carry a path), not by
//! anything here. What this module does is reject shapes that cannot be a local
//! program, and shapes whose *display* would lie (bidi overrides).
//!
//! Nothing here spawns. The filesystem is touched only for existence, the
//! bundle check, and the unix execute bit.

use std::path::Path;

use crate::error::AppError;
use crate::external::TargetOs;

use super::catalog::Recipe;
use super::ToolKind;

/// Generous for any real install path; a longer string is not one.
/// Same value and same reasoning as `external_cmd::MAX_LEN`.
const MAX_LEN: usize = 512;

/// Cap on the derived picker label, so a 200-character filename cannot blow out
/// the row.
const MAX_LABEL: usize = 48;

/// What shape the browsed target is. Chooses the launch recipe
/// ([`synthesize_recipe`]) and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomKindShape {
    /// A regular executable file.
    Executable,
    /// A macOS `.app` bundle (a *directory*), launched via `open -a`.
    MacBundle,
}

/// C0/C1 controls plus the bidi overrides and isolates — the set
/// `ai::stream::strip_control_chars` and `external_cmd` already use, for the
/// same reason: a bidi override in a filename exists only to make a picker row
/// read as something other than what launches.
fn is_disallowed_char(c: char) -> bool {
    let bidi =
        matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
    c.is_control() || bidi
}

/// A UNC / double-slash root (`\\server\share`, `//host/share`, `\\?\C:\…`).
/// A remote share is not a local tool, and `is_file()` on one goes to the
/// network — so it is refused before the filesystem is touched.
fn is_unc(value: &str) -> bool {
    let mut cs = value.chars();
    matches!(
        (cs.next(), cs.next()),
        (Some('\\'), Some('\\')) | (Some('/'), Some('/'))
    )
}

/// Absolute for the **target** OS. `Path::is_absolute` is host-relative, which
/// would make the macOS and Linux rules unassertable from a Windows box.
fn is_absolute_for(os: TargetOs, value: &str) -> bool {
    match os {
        TargetOs::Windows => {
            let mut cs = value.chars();
            matches!(
                (cs.next(), cs.next(), cs.next()),
                (Some(c), Some(':'), Some('/' | '\\')) if c.is_ascii_alphabetic()
            )
        }
        TargetOs::MacOs | TargetOs::Linux => value.starts_with('/'),
    }
}

/// ONE refusal, category-only, in the `external_cmd::refuse` style: it names the
/// rule and **never echoes the path**.
///
/// The path is config- or dialog-derived: it can be arbitrarily long, carry bidi
/// overrides, or imitate a system message, and this string can end up in a
/// toast. The UI shows its own copy (`P112-ui.md` §8 `BROWSE_ERR`) — this text
/// is the API-level explanation.
fn refuse() -> AppError {
    AppError::ExternalToolFailed(
        "that selection cannot be used as a program — choose a program file on this machine \
         (on Windows, a `.exe`)"
            .to_string(),
    )
}

/// The P112 §5.4 rule set for a dialog-returned path.
///
/// `os` is an explicit parameter (house pattern) so the Windows `.exe` rule and
/// the macOS bundle rule are unit-testable on any host. Touches the filesystem
/// (existence, bundle, execute bit); never spawns.
///
/// **One rule is inherently host-bound:** the unix execute bit needs
/// `PermissionsExt`, which does not exist on Windows. With `os` unix on a
/// Windows host the check is skipped (a Windows host cannot answer it), so
/// "a non-executable file on unix is refused" is only provable on a unix host.
/// Every other rule follows the `os` parameter exactly.
pub fn validate_custom_program(path: &Path, os: TargetOs) -> Result<CustomKindShape, AppError> {
    let value = path.to_string_lossy();
    // Textual rules first: no filesystem contact for a string that cannot be a
    // local program path at all.
    if value.is_empty() || value.len() > MAX_LEN {
        return Err(refuse());
    }
    if value.chars().any(is_disallowed_char) {
        return Err(refuse());
    }
    if is_unc(&value) || !is_absolute_for(os, &value) {
        return Err(refuse());
    }
    // The bundle branch first: a bundle is a DIRECTORY, which the `is_file`
    // branch rejects — the exact case an `is_file`-only model got wrong.
    if os == TargetOs::MacOs && is_mac_bundle(path) {
        require_label(path)?;
        return Ok(CustomKindShape::MacBundle);
    }
    if !path.is_file() {
        return Err(refuse());
    }
    match os {
        // DEC-1 and this is its ENFORCEMENT half: a dialog filter is not a gate
        // (the user can type `payload.cmd` in the filename box, and a filter is
        // one "add All files" change away from gone). `.cmd`/`.bat`/`.ps1` are
        // re-interpreted by `cmd.exe`, which performs `%VAR%` expansion on the
        // argv it receives (the CVE-2024-24576 path); refusing them removes
        // that residual for browsed programs entirely.
        TargetOs::Windows => {
            let is_exe = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
            if !is_exe {
                return Err(refuse());
            }
        }
        // The meaningful gate on unix: a non-executable pick would otherwise
        // fail at spawn with an opaque error. The asymmetry with Windows is
        // principled — a unix script with the execute bit is launched by the
        // kernel honouring its shebang, with no argv re-expansion to remove.
        TargetOs::MacOs | TargetOs::Linux => {
            if !has_execute_bit(path) {
                return Err(refuse());
            }
        }
    }
    require_label(path)?;
    Ok(CustomKindShape::Executable)
}

/// `p.is_dir()` AND a real bundle — the `Contents/Info.plist` requirement is
/// what separates a bundle from a directory merely named `*.app`.
fn is_mac_bundle(p: &Path) -> bool {
    p.is_dir() && p.join("Contents").join("Info.plist").is_file()
}

#[cfg(unix)]
fn has_execute_bit(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

/// Windows host: there is no execute bit to read, so this rule cannot be
/// evaluated here. See the `validate_custom_program` doc comment.
#[cfg(not(unix))]
fn has_execute_bit(_p: &Path) -> bool {
    true
}

/// A path whose derived label would be empty is refused: the picker must not
/// show a nameless row.
fn require_label(path: &Path) -> Result<(), AppError> {
    if display_label(path).is_empty() {
        return Err(refuse());
    }
    Ok(())
}

/// The picker label for a browsed tool — derived by the **backend**, never
/// renderer-supplied: the file stem (which on `Foo.app` is the bundle name
/// minus `.app`, i.e. exactly the macOS app name), with control/bidi characters
/// stripped and the result truncated.
pub fn display_label(path: &Path) -> String {
    let raw = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let cleaned: String = raw.chars().filter(|c| !is_disallowed_char(*c)).collect();
    let trimmed = cleaned.trim();
    match trimmed.char_indices().nth(MAX_LABEL) {
        Some((idx, _)) => trimmed[..idx].to_string(),
        None => trimmed.to_string(),
    }
}

/// A stored browsed path as DISPLAY text (`DetectedTool::detail`).
///
/// Verbatim for anything [`validate_custom_program`] would accept — it already
/// refuses these characters. It matters for the one row that is displayed
/// *despite* failing validation: a remembered path that is gone or was
/// hand-written into `settings.json` is still listed (so the UI can explain
/// itself), and it must not be able to carry a bidi override into the picker
/// subtitle.
pub(crate) fn sanitize_detail(value: &str) -> String {
    if value.chars().any(is_disallowed_char) {
        return value.chars().filter(|c| !is_disallowed_char(*c)).collect();
    }
    value.to_string()
}

/// `(kind, shape)` ⇒ the recipe a browsed tool launches with (P112 §5.4).
///
/// The `Terminal` + `Executable` row **is** audit route 3's shape (a program
/// with no arguments and the target as its cwd). It is reachable only for a
/// program a human browsed to and deliberately selected as their terminal — a
/// renderer cannot create that state, a repository cannot, and migration
/// cannot. What remains is a user able to misconfigure their own terminal:
/// accepted, and documented rather than hidden.
pub fn synthesize_recipe(kind: ToolKind, shape: CustomKindShape) -> Recipe {
    match (kind, shape) {
        (_, CustomKindShape::MacBundle) => Recipe::MacOpen,
        (ToolKind::Editor, CustomKindShape::Executable) => Recipe::DirLastArg(&[]),
        // No argument convention is known for an arbitrary terminal, so "open a
        // terminal here" has only one mechanism: start it IN the directory.
        (ToolKind::Terminal, CustomKindShape::Executable) => Recipe::DirCwd(&[]),
    }
}
