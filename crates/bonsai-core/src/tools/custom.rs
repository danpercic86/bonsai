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
/// Same value and same reasoning as the `MAX_LEN` of the deleted
/// `external_cmd` module, whose four shared helpers this file inherited (§7).
const MAX_LEN: usize = 512;

/// Cap on the derived picker label, so a 200-character filename cannot blow out
/// the row.
const MAX_LABEL: usize = 48;

/// The stored browsed path, as a type a renderer-supplied string cannot become.
///
/// P112's security property is that `UiSettingsPatch` has **no field able to
/// carry a program path**, making a renderer-written path *unrepresentable*
/// rather than merely rejected. This extends the same discipline one layer
/// down: [`super::tool_scan`] / [`super::picked`] took the browsed path as a
/// bare `&str`, and nothing in those signatures stopped a future command
/// handler from sourcing it out of a request body instead of
/// `settings::Settings` — the exact route P112 exists to delete.
///
/// There is deliberately **no** `From<&str>`, `From<String>`, `FromStr`,
/// `Deserialize` or `Default` impl, and there must never be one: each silently
/// reopens that hole (`Default` more mildly — it can only yield `""` — but it
/// is still a second, unnamed constructor, and the type's whole value is that
/// construction is a deliberate, grep-able act). [`Self::from_settings_field`]
/// is therefore the only constructor; "no browsed tool" is
/// `from_settings_field("")`.
///
/// **Honest limitation.** `settings` lives in the `bonsai` (src-tauri) crate
/// while this lives in `bonsai-core`, and Rust has no cross-crate form of
/// "constructible only in that module" — so the constructor must be `pub`.
/// What the type buys is that a request-body `&str` no longer *type-checks*
/// into a scan or a launch; it does not prove the string's origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowsedProgram(String);

impl BrowsedProgram {
    /// The ONE constructor: the `custom_terminal_path` / `custom_editor_path`
    /// field of the settings file, which only a native dialog **the backend
    /// opened itself** ever writes (§5.4). `""` = no browsed tool.
    pub fn from_settings_field(stored: &str) -> BrowsedProgram {
        BrowsedProgram(stored.to_string())
    }

    /// The stored path, for the `pub(crate)` half of `tools` that works in
    /// `&str` (scan assembly, the launch recheck) and is unreachable from a
    /// command handler.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

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
/// `ai::stream::strip_control_chars` uses, and the deleted `external_cmd` used,
/// for the same reason: a bidi override in a filename exists only to make a
/// picker row read as something other than what launches.
fn is_disallowed_char(c: char) -> bool {
    let bidi =
        matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
    c.is_control() || bidi
}

/// A UNC / double-slash root (`\\server\share`, `//host/share`, and the mixed
/// spellings `\/server\share` / `/\server\share`) — including the `\\?\` and
/// `\\.\` device prefixes, which share the shape.
///
/// **Every separator pair, homogeneous or mixed**, because Win32 treats `/` and
/// `\` interchangeably when it classifies a path prefix, and because this
/// predicate must cover everything [`is_absolute_for`]'s share arm admits — see
/// the invariant on that function. It matched only the homogeneous pairs until
/// 2026-09-14, which let a mixed-separator share through DETECTION.
///
/// Deliberately OS-agnostic, exactly like [`is_device_prefix`]: a unix path
/// starting `/\` now reads as UNC too. A directory literally named `\opt` is
/// not something the unix ladders or a `PATH` entry produce, and keeping this a
/// one-line shape test is worth more than that case.
///
/// **Shape only: this says nothing about whether such a path is refused.** That
/// is each caller's decision, and the two callers deliberately disagree — see
/// AMEND-6 (user ruling #26) in
/// `docs/contracts/P112-external-tool-detection.md`: the browse path is to
/// accept shares, detection keeps refusing them.
pub(super) fn is_unc(value: &str) -> bool {
    let mut cs = value.chars();
    matches!((cs.next(), cs.next()), (Some('\\' | '/'), Some('\\' | '/')))
}

/// A Windows **device namespace** prefix (`\\?\`, `\\.\`, and the
/// forward-slash spellings Win32 also accepts).
///
/// These share [`is_unc`]'s double-separator shape but are not shares: they are
/// device namespaces, and nothing a file dialog returns. AMEND-6 (user ruling
/// #26) relaxes the browse path's UNC refusal and **keeps this one** — hence
/// the split into two predicates.
///
/// Note `\\?\UNC\server\share\…` (the form `fs::canonicalize` produces for a
/// share) is refused here too: nothing in this module canonicalizes, so a
/// stored path only ever has that shape if it was written that way, and the
/// device namespace bypasses Win32 path normalization.
pub(super) fn is_device_prefix(value: &str) -> bool {
    let mut cs = value.chars();
    matches!(
        (cs.next(), cs.next(), cs.next(), cs.next()),
        (Some('\\' | '/'), Some('\\' | '/'), Some('?' | '.'), Some('\\' | '/'))
    )
}

/// The root shapes the **browse path** accepts (P112 §5.4) — the single seam
/// [`validate_custom_program`] consults, so the AMEND-6 accept-case is
/// assertable without a reachable network share.
///
/// Absolute for the target OS (including a UNC share, per ruling #26) and not a
/// device namespace. Detection deliberately answers this question differently
/// ([`super::detect`]'s `locally_absolute`, which adds `!is_unc`); that
/// divergence IS the ruling, so do not fold the two together.
pub(super) fn browsable_root(os: TargetOs, value: &str) -> bool {
    !is_device_prefix(value) && is_absolute_for(os, value)
}

/// Absolute for the **target** OS — the one predicate genuinely SHARED with
/// [`super::detect`]'s probe hits.
///
/// `Path::is_absolute` cannot be used: it is host-relative, so
/// `/usr/bin/konsole` is not absolute on Windows and the Linux/macOS ladders
/// would be untestable from a Windows box (AC2) while silently accepting
/// relative candidates there.
///
/// Windows requires a DRIVE LETTER, so `\Windows\x.exe` AND `/Windows/x.exe`
/// are both rejected: Win32 treats each as drive-relative (`Path::is_absolute`
/// agrees — it is `false` for both), so either would resolve against the
/// process cwd. **That drive-relative rule is what both callers want, and it is
/// why this predicate is shared.**
///
/// It deliberately does **not** answer the *refusal* question for UNC. Per
/// AMEND-6 (user ruling #26) a `\\server\share\…` root IS absolute here — the
/// browse path accepts shares — while [`is_unc`] at detection's call site is
/// what keeps **detection** refusing them. Keeping the two decisions separate
/// is the whole point; do not fold `is_unc` back in here.
///
/// **That only holds under one invariant: `unc_share ⇒ is_unc`** — every
/// separator pair the share arm below admits, [`is_unc`] must match, mixed
/// spellings included. It did not hold when the arm first landed (`is_unc` took
/// `\\` and `//` only), so `\/server\share\Code.exe` passed detection's
/// `!is_unc && is_absolute_for`. Widen [`is_unc`] alongside any widening here;
/// `custom_tests::a_unc_share_root_is_browsable_but_never_a_detection_hit`
/// and `detect_tests::a_unc_or_drive_relative_candidate_is_never_a_hit` pin it.
///
/// On unix `//host/share/…` was already absolute (it starts with `/`), so the
/// Windows arm is the only one the ruling moves.
pub(super) fn is_absolute_for(os: TargetOs, value: &str) -> bool {
    match os {
        TargetOs::Windows => {
            let mut cs = value.chars();
            let head = (cs.next(), cs.next(), cs.next());
            let drive = matches!(
                head,
                (Some(c), Some(':'), Some('/' | '\\')) if c.is_ascii_alphabetic()
            );
            // A UNC SHARE root: two separators (in ANY mix — `is_unc` must
            // match all four pairs, see above) then a NON-SEPARATOR character.
            // That is all it checks: `\\server` with no share, and `\\?`, pass
            // here too — harmless, since `validate_custom_program`'s `is_file()`
            // refuses them. Device prefixes share this shape and are sorted out
            // by [`is_device_prefix`] at the one call site that cares.
            let unc_share = matches!(
                head,
                (Some('\\' | '/'), Some('\\' | '/'), Some(c)) if c != '\\' && c != '/'
            );
            drive || unc_share
        }
        TargetOs::MacOs | TargetOs::Linux => value.starts_with('/'),
    }
}

/// ONE refusal, category-only, in the `refuse` style the deleted `external_cmd`
/// established and that is this surface's house rule now: it names the rule and
/// **never echoes the path**.
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
    // THIS module's stance on roots, deliberately separate from the stricter
    // one detection takes at its own call site (`detect::locally_absolute`).
    // AMEND-6 (user ruling #26, `docs/contracts/P112-external-tool-detection.md`)
    // ACCEPTS `\\server\share\…` here — a browse is a deliberate one-time act,
    // so the network cost is paid knowingly — while detection, which has one
    // 1500 ms budget for the whole machine, keeps refusing it. The device
    // prefixes `\\?\` / `\\.\` stay refused on both paths.
    if !browsable_root(os, &value) {
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

/// A path as DISPLAY text (`DetectedTool::detail`), for EVERY row.
///
/// Not a browsed-row special case: a probe-derived path never passes through
/// [`validate_custom_program`] either, and `detect::executable_hit` performs no
/// character check — so a PATH directory whose *name* carries a bidi override
/// would otherwise yield a row whose label is trustworthy (the static catalog)
/// but whose subtitle reads as a different path than the one that launches.
///
/// Verbatim for anything [`validate_custom_program`] would accept: it refuses
/// these characters, and [`MAX_LEN`] bounds it at 512 *bytes*, so such a path
/// has at most 512 chars and the truncation cannot fire.
///
/// **The cap can fire on any row, not just the browsed one.** Two sources are
/// length-unbounded: a remembered browsed path that is gone or was hand-written
/// into `settings.json` (displayed *despite* failing validation, so the UI can
/// explain itself), and a **probe-derived** `Resolution::program` — a `PATH`
/// directory plus a program name, which never passes `MAX_LEN` and which
/// `is_file()` accepts at any length because `std` applies the `\\?\` prefix
/// internally. The cap is only a no-op for paths validation has *accepted*.
///
/// Truncation **appends an ellipsis**, so a subtitle showing a trustworthy
/// prefix cannot be mistaken for the whole path: the result is at most
/// `MAX_LEN` content chars + `…` (513 chars).
pub(crate) fn sanitize_detail(value: &str) -> String {
    let cleaned: String = if value.chars().any(is_disallowed_char) {
        value.chars().filter(|c| !is_disallowed_char(*c)).collect()
    } else {
        value.to_string()
    };
    // `char_indices().nth(MAX_LEN)` is a char boundary by construction, so the
    // slice can never split a multi-byte character.
    match cleaned.char_indices().nth(MAX_LEN) {
        Some((idx, _)) => format!("{}…", &cleaned[..idx]),
        None => cleaned,
    }
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
