//! P91 §7.2 — resolve the home directory that [`super::scrub_home`] masks, and
//! do it **fail-closed** (2026-09-11 auditor MUST-FIX).
//!
//! ONE concern: turning an `AppHandle` into `Option<folded home string>`. The
//! result is handed to `writer::WriterConfig::home_mask` at session start, so
//! every file states in its own header whether masking was active
//! (`homeMasking`) — there is no process-global to go silently unset.
//!
//! # Why a fallback exists at all
//!
//! The first source is Tauri's own resolver (`path().home_dir()`). When it fails
//! the previous code simply left masking OFF and printed to stderr, which in a
//! release GUI build goes nowhere: raw mode would then write
//! `C:\Users\<account>\…` into every part of an export zip that a user mails to a
//! third party, with nothing in the file admitting it. §7.1 says over-redaction
//! is the deliberate failure direction, so the fallback derives a candidate from
//! the app CONFIG directory, which is under the home directory on all three
//! platforms:
//!
//! | OS | `app_config_dir()` | containers to strip |
//! |---|---|---|
//! | Windows | `<home>\AppData\Roaming\com.bonsai.app` | id, `Roaming`, `AppData` |
//! | macOS | `<home>/Library/Application Support/com.bonsai.app` | id, `Application Support`, `Library` |
//! | Linux | `<home>/.config/com.bonsai.app` | id, `.config` |
//!
//! **Calibration, stated honestly:** both resolvers read largely the same inputs
//! (`$HOME`/passwd on unix, the known folders on Windows), so the window where
//! the fallback actually fires is narrow — `XDG_CONFIG_HOME` pointing outside an
//! unset `$HOME` is about it. And when BOTH fail there is still no masking: what
//! makes that honest rather than silent is the `homeMasking: false` stamp, not
//! this module.
//!
//! # Why stripping by NAME and not by depth
//!
//! Counting ancestors (`nth(3)` on Windows, `nth(2)` on Linux) mis-fires the
//! moment a layout differs, and over-stripping is the dangerous direction: a
//! candidate of `C:\Users` masks `C:\Users\jane\x` to `<home>\jane\x`, i.e. the
//! account name survives behind a token that claims otherwise. Stripping only
//! segments we RECOGNISE stops early on an unexpected layout (a useless but
//! harmless candidate) instead of climbing past the home directory, and
//! `scrub_home::normalize_home` refuses the shared parents as a second gate.

use std::path::{Path, PathBuf};

use tauri::Manager;

use super::scrub_home::normalize_home;

/// Directory names that sit between the home directory and the app config dir,
/// lowercased. The bundle identifier is stripped unconditionally (it is always
/// the last segment), so it is not listed here.
const APP_CONTAINERS: &[&str] = &[
    "appdata",             // Windows
    "roaming",             // Windows
    "local",               // Windows, if the resolver ever returns LocalAppData
    "library",             // macOS
    "application support", // macOS
    ".config",             // Linux / XDG default
];

/// Most levels between the app config dir and the home dir on any platform
/// (macOS: identifier → `Application Support` → `Library`). A bound, so a
/// pathological name chain cannot walk to the filesystem root.
const MAX_STRIP: usize = 3;

/// The folded home string for this session, or `None` when neither source
/// yields a usable one (⇒ the caller stamps `homeMasking: false`).
///
/// Cheap: at most two path lookups, called once per sink start.
pub fn resolve_home_mask(app: &tauri::AppHandle) -> Option<String> {
    if let Ok(home) = app.path().home_dir() {
        if let Some(folded) = normalize_home(&home) {
            return Some(folded);
        }
    }
    let config_dir = app.path().app_config_dir().ok()?;
    normalize_home(&strip_app_containers(&config_dir)?)
}

/// Walk `config_dir` up past the bundle identifier and any recognised app
/// container, returning the first directory that is neither — the home-directory
/// candidate. `None` when the walk would leave a filesystem root (`/`, `C:`) or
/// run out of segments.
///
/// Pure (no filesystem access), and the walk runs on a `/`-folded copy rather
/// than on `Path` components ON PURPOSE: `Path::parent`/`file_name` split only on
/// the HOST separator, so a Windows layout is a single opaque component on
/// unix — which would make "all three layouts are asserted on any host" (the
/// test module's own claim) impossible to keep. The returned candidate goes
/// straight into [`normalize_home`], which folds separators again anyway, so
/// handing back `C:/Users/jane` for `C:\Users\jane` changes nothing downstream.
pub(super) fn strip_app_containers(config_dir: &Path) -> Option<PathBuf> {
    let folded = config_dir.to_string_lossy().replace('\\', "/");
    // The last segment is the bundle identifier (`com.bonsai.app`), which cannot
    // be matched by name — drop exactly one level for it.
    let mut dir = parent_of(&folded)?;
    for _ in 0..MAX_STRIP {
        if !APP_CONTAINERS.contains(&last_segment(dir)?.to_lowercase().as_str()) {
            break;
        }
        dir = parent_of(dir)?;
    }
    Some(PathBuf::from(dir))
}

/// `a/b/c` → `a/b`, ignoring a trailing separator. `None` when the parent would
/// be a ROOT (`""` for `/x`, `c:` for `c:/x`) — a root is never a home candidate
/// and `normalize_home` would refuse it anyway.
fn parent_of(path: &str) -> Option<&str> {
    let (parent, _) = path.trim_end_matches('/').rsplit_once('/')?;
    if parent.is_empty() || parent.ends_with(':') {
        return None;
    }
    Some(parent)
}

/// The last path segment of a value already produced by [`parent_of`] (so it
/// carries no trailing separator).
fn last_segment(path: &str) -> Option<&str> {
    path.rsplit('/').next().filter(|seg| !seg.is_empty())
}
