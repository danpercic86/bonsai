//! The P70 startup preflight: the ONE place in the app that ever executes
//! `git --version`.
//!
//! Split out of [`super`] (`gitbin.rs`) so both files stay under the 500-line
//! limit: resolution is a hot-path, execution-free ladder; this is an
//! off-hot-path, execution-BASED validation with its own wire type. Re-exported
//! from `gitbin` so callers keep using `gitbin::check_availability()`.

use super::{git_command, git_not_found_message, refresh_git_bin, GitBinSource};


/// Result of the one-shot startup preflight (P70 §4.1). Mirrors
/// `ai::AiAvailability`'s contract: a missing git is a NORMAL result
/// (`found: false`), never an error.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitAvailability {
    /// A git executable was resolved AND `--version` exited 0.
    pub found: bool,
    /// The path actually tried. Populated whenever a CANDIDATE resolved, even
    /// when it turned out to be unrunnable (UI ratified decision D1 — the
    /// banner keys its "found but unrunnable" variant off `path !== null`);
    /// `None` only when the ladder fell back to the bare name.
    pub path: Option<String>,
    /// e.g. `"2.47.1.windows.1"`, parsed from `git version <X>`. `None` when
    /// not found or unparseable.
    pub version: Option<String>,
    /// Which rung produced the path — reported even when the candidate failed.
    pub source: GitBinSource,
    /// Human one-liner. Found: `"Git 2.47.1 — /usr/bin/git (path)"`.
    /// Not found: the full [`git_not_found_message`] text.
    pub detail: String,
}

/// Lower-camel wire name of a rung, reused in [`GitAvailability::detail`] so the
/// diagnostic line and the serialized `source` always agree.
fn source_label(source: GitBinSource) -> &'static str {
    match source {
        GitBinSource::Override => "override",
        GitBinSource::Path => "path",
        GitBinSource::Registry => "registry",
        GitBinSource::WellKnown => "wellKnown",
        GitBinSource::Fallback => "fallback",
    }
}

/// Parse the version token out of `git --version` stdout (`git version 2.47.1`).
/// Defensive: unknown/localized/empty output yields `None`, never a panic.
fn parse_git_version(stdout: &str) -> Option<String> {
    let line = stdout.lines().next()?.trim();
    let mut tokens = line.split_whitespace();
    // The `git version <X>` PREFIX must be present: without this check any
    // three-word output (a localized string, an error, a shim's banner) would
    // be reported as a version number.
    if tokens.next() != Some("git") || tokens.next() != Some("version") {
        return None;
    }
    let token = tokens.next()?;
    (!token.is_empty()).then(|| token.to_string())
}

/// **Blocking.** Re-runs the ladder ([`refresh_git_bin`]), then — only when a
/// candidate resolved — executes `<path> --version` EXACTLY ONCE. This is the
/// only place in the app that ever runs `--version`: resolution itself never
/// executes a candidate (D4), so this preflight is what catches a resolved but
/// corrupt/unrunnable `git.exe`.
///
/// NEVER returns `Err`: an unresolvable or unspawnable git is
/// `{ found: false, .. }`.
pub fn check_availability() -> GitAvailability {
    let bin = refresh_git_bin();
    let label = source_label(bin.source);
    if !bin.found() {
        return GitAvailability {
            found: false,
            path: None,
            version: None,
            source: bin.source,
            detail: git_not_found_message(),
        };
    }
    let path = bin.path.display().to_string();
    let out = git_command()
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let version = parse_git_version(&String::from_utf8_lossy(&o.stdout));
            let detail = match &version {
                Some(v) => format!("Git {v} — {path} ({label})"),
                None => format!("Git — {path} ({label})"),
            };
            GitAvailability {
                found: true,
                path: Some(path),
                version,
                source: bin.source,
                detail,
            }
        }
        // Spawn error OR non-zero exit: the path is reported anyway so the UI
        // can say WHICH program it failed to run.
        _ => GitAvailability {
            found: false,
            path: Some(path),
            version: None,
            source: bin.source,
            detail: git_not_found_message(),
        },
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    // The version parser is the only pure part of the preflight: defensive over
    // localized / truncated / empty output, never a panic.
    #[test]
    fn parse_git_version_table() {
        let cases: [(&str, Option<&str>); 7] = [
            ("git version 2.47.1\n", Some("2.47.1")),
            ("git version 2.47.1.windows.1\r\n", Some("2.47.1.windows.1")),
            ("git version 2.39.5 (Apple Git-154)\n", Some("2.39.5")),
            ("  git version 2.47.1  \n", Some("2.47.1")),
            ("", None),
            ("git version\n", None),
            ("totally unexpected output", None),
        ];
        for (stdout, want) in cases {
            assert_eq!(
                parse_git_version(stdout).as_deref(),
                want,
                "stdout {stdout:?}"
            );
        }
    }

    // Every rung has a stable wire label, and it matches the serde camelCase
    // rendering of `GitBinSource` (the detail line and `source` must agree).
    #[test]
    fn source_label_matches_the_wire_name() {
        for source in [
            GitBinSource::Override,
            GitBinSource::Path,
            GitBinSource::Registry,
            GitBinSource::WellKnown,
            GitBinSource::Fallback,
        ] {
            let json = serde_json::to_string(&source).expect("serialize");
            assert_eq!(json, format!("\"{}\"", source_label(source)));
        }
    }

    // The preflight NEVER errors: on this host it either reports a real git or
    // an honest not-found, and the two shapes are internally consistent.
    #[test]
    fn check_availability_is_internally_consistent() {
        let a = check_availability();
        if a.found {
            assert!(a.path.is_some(), "a found git must report its path");
            assert_ne!(a.source, GitBinSource::Fallback);
            assert!(a.detail.starts_with("Git "), "{}", a.detail);
            assert!(
                a.detail.contains(source_label(a.source)),
                "{}",
                a.detail
            );
        } else {
            // Not found: the detail IS the honest §3.3 copy, and `path` is
            // populated only when a candidate actually resolved (UI D1).
            assert_eq!(a.detail, git_not_found_message());
            assert_eq!(a.version, None);
            assert_eq!(a.path.is_none(), a.source == GitBinSource::Fallback);
        }
    }
}
