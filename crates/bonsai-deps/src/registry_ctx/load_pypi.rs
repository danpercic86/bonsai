//! The PyPI half of `RegistryContext::load` (§4.4, E3 pip + uv / Poetry
//! rows). Same fail-closed shape as `load_cargo`: every step is
//! `Result<_, Closed>`, and the only fail-OPEN value is
//! `PypiSignals::into_open`, called after this whole function returned `Ok`.
//!
//! Files:
//! - repo walk (each PyPI manifest dir → workdir): `uv.toml`, the index
//!   definitions of `pyproject.toml`, and `{.venv,venv}/pip.{conf,ini}`;
//! - user / system: `$PIP_CONFIG_FILE`, the pip user + global files, the uv
//!   user + system `uv.toml`. pip skips its user files when
//!   `PIP_CONFIG_FILE` exists; they are read anyway (a superset);
//! - env: `PIP_*` / `UV_*` index variables;
//! - `requirements*.txt` includes: every `-r` / `-c` target is followed
//!   (inside the workdir only, `safe_read`, depth ≤ [`MAX_INCLUDE_DEPTH`],
//!   cycles skipped). An index option found in an INCLUDED file, or in any
//!   file that includes another, makes every PyPI dep private (pip's index
//!   options are global to the run, and the included deps form their own
//!   groups); a file with no include keeps its options per file in the
//!   adapter (E2). A remote, absolute, escaping or unreadable include ⇒
//!   `Closed`.

use std::collections::BTreeSet;
use std::path::Path;

use crate::adapter::{file_name, parent_dir};
use crate::eco::pypi::{is_requirements, PYPROJECT};
use crate::eco::requirements::{logical_lines, option, Opt};
use crate::fsread::{path_is_clean, safe_read, ReadOutcome, MAX_FILE_BYTES};

use super::host_env::{HostEnv, UserConfig};
use super::pip_cfg::{keep_pypi_env, pip_ini, pypi_env_pair, pyproject, uv_toml, PypiSignals};
use super::walk::{manifest_dirs_by, repo_walk, user_files};
use super::Closed;

/// Include nesting beyond this ⇒ `Closed`.
pub(crate) const MAX_INCLUDE_DEPTH: usize = 8;

#[derive(Clone, Copy)]
enum Format {
    PipIni,
    UvToml,
    Pyproject,
}

/// Repo-walk files. `[P125c2 review]` the in-repo virtualenv's pip config
/// (pip's SITE kind, `sys.prefix/pip.{conf,ini}`) for the two usual venv
/// dir names — a config read, so discovery's `.venv` / `venv` skip does not
/// apply; `safe_read` (no-follow) as for every repo file. A venv outside
/// the repo (`$VIRTUAL_ENV`) is not reachable here (P125e).
const REPO_FILES: &[(&str, Format)] = &[
    ("uv.toml", Format::UvToml),
    (PYPROJECT, Format::Pyproject),
    (".venv/pip.conf", Format::PipIni),
    (".venv/pip.ini", Format::PipIni),
    ("venv/pip.conf", Format::PipIni),
    ("venv/pip.ini", Format::PipIni),
];

const USER_FILES: &[(UserConfig, Format)] = &[
    (UserConfig::PipConfigFile, Format::PipIni),
    (UserConfig::PipUser, Format::PipIni),
    (UserConfig::PipSystem, Format::PipIni),
    (UserConfig::UvUser, Format::UvToml),
    (UserConfig::UvSystem, Format::UvToml),
];

fn extract(sig: &mut PypiSignals, fmt: Format, text: &str) -> Result<(), Closed> {
    match fmt {
        Format::PipIni => pip_ini(sig, text),
        Format::UvToml => uv_toml(sig, text),
        Format::Pyproject => pyproject(sig, text),
    }
}

fn is_pypi_manifest(name: &str) -> bool {
    name == PYPROJECT || is_requirements(name)
}

/// Resolves an include path against the including file's dir, lexically.
/// Remote, absolute or escaping (`..` above the workdir) ⇒ `Closed`.
pub(crate) fn resolve_include(from_dir: &str, target: &str) -> Result<String, Closed> {
    let t = target.trim().trim_matches('"');
    let absolute = t.starts_with(['/', '\\', '~'])
        || t.get(1..2) == Some(":")
        || t.contains("://")
        || t.is_empty();
    if absolute {
        return Err(Closed);
    }
    let mut parts: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in t.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop().ok_or(Closed)?;
            }
            s => parts.push(s),
        }
    }
    let rel = parts.join("/");
    if path_is_clean(&rel) {
        Ok(rel)
    } else {
        Err(Closed)
    }
}

/// Follows every include of the `requirements*.txt` manifests.
fn includes(
    sig: &mut PypiSignals,
    workdir_canon: &Path,
    manifests: &[String],
) -> Result<(), Closed> {
    // Keyed by (path, included?): a top-level manifest that is ALSO
    // included elsewhere must be re-read as an include, or its index
    // options (global to the includer's run) would be skipped.
    let mut seen: BTreeSet<(String, bool)> = BTreeSet::new();
    let mut stack: Vec<(String, usize)> = manifests
        .iter()
        .filter(|m| is_requirements(file_name(m)))
        .map(|m| (m.clone(), 0))
        .collect();
    while let Some((path, depth)) = stack.pop() {
        if !seen.insert((path.clone(), depth > 0)) {
            continue;
        }
        let text = match safe_read(workdir_canon, &path, MAX_FILE_BYTES) {
            ReadOutcome::Content(t) => t,
            // A top-level manifest that vanished reports in its own group;
            // an include that cannot be read hides options ⇒ Closed.
            ReadOutcome::Absent if depth == 0 => continue,
            _ => return Err(Closed),
        };
        let opts: Vec<Opt> = logical_lines(&text)
            .iter()
            .filter(|l| l.text.starts_with('-'))
            .map(|l| option(&l.text))
            .collect();
        // [P125c2 review] an includer's index options also route the files
        // it includes (their deps form their own groups, routed Default by
        // the adapter), so they apply globally at ANY depth once the file
        // includes anything. An included file's options always do.
        let global = depth > 0 || opts.iter().any(|o| matches!(o, Opt::Include(_)));
        for o in opts {
            match o {
                Opt::Include(target) => {
                    if depth + 1 > MAX_INCLUDE_DEPTH {
                        return Err(Closed);
                    }
                    let rel = resolve_include(parent_dir(&path), &target)?;
                    stack.push((rel, depth + 1));
                }
                Opt::Index(v) if global => sig.index(&v)?,
                Opt::NoIndex if global => sig.private_unknown(),
                _ => {}
            }
        }
    }
    Ok(())
}

fn env_vars(sig: &mut PypiSignals, env: &dyn HostEnv) -> Result<(), Closed> {
    let pip = env.vars_with_prefix_ci("PIP_", keep_pypi_env);
    let uv = env.vars_with_prefix_ci("UV_", keep_pypi_env);
    for (k, v) in pip.iter().chain(&uv) {
        pypi_env_pair(sig, k, v)?;
    }
    Ok(())
}

/// All PyPI signals, or `Closed` at the first failing step.
pub(super) fn load(
    workdir_canon: &Path,
    manifests: &[String],
    env: &dyn HostEnv,
) -> Result<PypiSignals, Closed> {
    let mut sig = PypiSignals::default();
    let dirs = manifest_dirs_by(manifests, is_pypi_manifest)?;
    {
        let mut f = |fmt: Format, text: &str| extract(&mut sig, fmt, text);
        repo_walk(workdir_canon, &dirs, REPO_FILES, &mut f)?;
        user_files(env, USER_FILES, &mut f)?;
    }
    includes(&mut sig, workdir_canon, manifests)?;
    env_vars(&mut sig, env)?;
    Ok(sig)
}
