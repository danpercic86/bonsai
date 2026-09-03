//! Producer-side containment gate for a submodule's absolute workdir path
//! (security audit 2026-09-03, findings HIGH-1 / HIGH-2 / MEDIUM-1 / LOW-3).
//!
//! `.gitmodules` `path` is attacker-controlled (a repo you cloned authored it).
//! `Path::join` silently DISCARDS the base for a rooted or UNC `rel` — measured
//! on this host with `git2` exactly as `list_submodules` uses it:
//!
//! | `.gitmodules` `path` | `submodules()` | `sm_workdir.join(path)` |
//! |---|---|---|
//! | `vendor/lib` (control) | admitted | `<workdir>/vendor/lib` (contained) |
//! | `/Windows/System32`    | admitted | `D:/Windows/System32` (escapes to drive root) |
//! | `//host/share`         | admitted | `//host/share` (**base discarded**) |
//! | `\\host\share`         | admitted | `\\host\share` (**base discarded**) |
//! | `..\escape` / `../escape` / `vendor/../../escape` | **dropped by libgit2** | — |
//! | `C:/Windows` / `C:\Windows` | **dropped by libgit2** | — |
//! | `a;b`, `a b`, `payload.exe`, `.git/hooks` | admitted | contained |
//!
//! So libgit2 already rejects `..` and drive-letter paths; the residual admitted
//! escapes are **rooted** (`/x`) and **UNC** (`//h/s`, `\\h\s`). An unchecked
//! `abs_path` from those reaches `p.exists()` in the launch command (an
//! SMB/WebDAV callout → NetNTLMv2 disclosure) and the external-tool ladder
//! (`wt -d <path>`, where `wt` splits `;` in one argv token — CONFIRMED on this
//! host). This function is the single layer that stops them, at the producer.

use std::path::{Component, Path};

/// Absolute, containment-checked workdir path for a submodule declared at
/// repo-relative `rel` under superproject workdir `sm_workdir`.
///
/// `None` ⇒ the declared path is unsafe; the caller records an INERT row
/// (`abs_path: None` → wire `absPath: null`) so the malformed submodule is still
/// VISIBLE to the user but no external tool / open-in-tab can target it. Prefer
/// this over dropping the row: hiding a submodule git itself reports is its own
/// correctness bug.
///
/// Safe ⇔ `rel` is relative and made ONLY of plain (`Normal`) components — no
/// `RootDir`/`Prefix` (rooted or UNC escape), no `ParentDir` (`..`), no
/// `CurDir`. All-`Normal` + relative makes the lexical join provably contained
/// without touching the filesystem, which keeps an UNINITIALIZED submodule (a
/// legitimate state where the target does not exist yet) working. When the
/// target DOES exist we additionally canonicalize and re-check containment, so a
/// symlinked, already-checked-out submodule dir cannot point back out.
pub fn contained_abs_path(sm_workdir: &Path, rel: &Path) -> Option<String> {
    if rel.as_os_str().is_empty() {
        return None;
    }
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    // All-Normal + relative ⇒ this lexical join cannot escape `sm_workdir`.
    let joined = sm_workdir.join(rel);
    // Symlink hardening for an already-checked-out submodule: canonicalize both
    // and require containment. `canonicalize` fails (and `\\?\`-prefixes on
    // Windows) for a not-yet-checked-out path — that case relies on the lexical
    // guarantee above, so a failure here is only fatal when the target exists.
    if joined.exists() {
        let canon_root = sm_workdir.canonicalize().ok()?;
        let canon_join = joined.canonicalize().ok()?;
        if !canon_join.starts_with(&canon_root) {
            return None;
        }
    }
    Some(joined.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn wd() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(r"D:\repo")
        } else {
            PathBuf::from("/repo")
        }
    }

    #[test]
    fn control_relative_path_is_contained() {
        let got = contained_abs_path(&wd(), Path::new("vendor/lib"));
        assert!(got.is_some(), "a plain relative submodule path must be admitted");
        let s = got.unwrap();
        assert!(s.contains("vendor"), "{s}");
        assert!(s.starts_with(wd().to_string_lossy().as_ref()), "{s}");
    }

    #[test]
    fn rooted_path_is_rejected() {
        // libgit2 hands `sm.path()` back verbatim for a rooted value.
        assert_eq!(contained_abs_path(&wd(), Path::new("/Windows/System32")), None);
    }

    #[test]
    fn parent_traversal_is_rejected() {
        assert_eq!(contained_abs_path(&wd(), Path::new("../escape")), None);
        assert_eq!(contained_abs_path(&wd(), Path::new("vendor/../../escape")), None);
    }

    #[test]
    fn empty_path_is_rejected() {
        assert_eq!(contained_abs_path(&wd(), Path::new("")), None);
    }

    #[cfg(windows)]
    #[test]
    fn unc_and_drive_paths_are_rejected() {
        assert_eq!(contained_abs_path(&wd(), Path::new(r"\\host\share")), None);
        assert_eq!(contained_abs_path(&wd(), Path::new("//host/share")), None);
        assert_eq!(contained_abs_path(&wd(), Path::new(r"C:\Windows")), None);
    }

    #[test]
    fn nested_relative_path_is_contained() {
        // A `-rf`-bearing component stays contained (one argv token downstream).
        assert!(contained_abs_path(&wd(), Path::new("vendor/-rf")).is_some());
    }
}
