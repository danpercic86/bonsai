//! `[Q9]` `[Q9-narrowed]` the FOLLOWING read for registry config files
//! outside the workdir. Symlinks are followed ONLY for:
//! - the fixed user / system config files (`HostEnv::read_user_file`);
//! - an above-workdir walk entry whose ancestor directory IS one of the
//!   caller's follow roots (`$HOME`, `$CARGO_HOME`) — see
//!   [`read_above_workdir_config`].
//!
//! Every other above-workdir entry (a parent monorepo / superproject dir) is
//! read NO-FOLLOW: a symlink there ⇒ `NotAFile` ⇒ the ecosystem fails
//! closed (audit Q9-1: a hostile superproject could otherwise aim a link at
//! an automounter path such as `/net/<host>`). Inside the workdir, reads
//! stay no-follow (`safe_read`, MEDIUM-3).
//!
//! Resolution is component by component (bounded hops, no recursion) with
//! `read_link`, so every link TARGET is checked BEFORE it is touched: a
//! Windows UNC / `\\?\UNC\` / device-namespace target and (on Windows) a DOS
//! device name are rejected unopened. A link whose own target is missing
//! (dangling), a loop, an intermediate component that is not a directory,
//! or a final target that is not a regular file after open ⇒ not
//! `Content`, and the caller fails the ecosystem closed. The final
//! component is opened with O_NOFOLLOW / FILE_FLAG_OPEN_REPARSE_POINT and
//! the handle is compared with the resolved target `[Q9-5]`. The read is
//! `take(cap + 1)` bounded; extraction stays key-filtered (the caller).
//!
//! A workdir that itself lives on a UNC share (`\\?\UNC\…`) fails the whole
//! above-workdir walk closed — intentional (never touch a network path).

use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::open_nofollow::{open_nofollow, same_file};
use super::{read_handle, ReadOutcome};

/// Symlink hops followed before giving up (Linux `MAXSYMLINKS` is 40).
const MAX_HOPS: u32 = 40;

/// `[Q9]` a path that would reach the network or a device namespace:
/// `\\server\share`, `//server/share`, `\\?\UNC\…`, `\\.\…`, `\??\…`, or any
/// other `\\?\` form that is not a verbatim drive (`\\?\C:\…`). Checked on
/// the TEXT, so it runs (and is tested) on every OS.
pub fn is_network_path(p: &str) -> bool {
    let b = p.as_bytes();
    let sep = |c: u8| c == b'\\' || c == b'/';
    if b.starts_with(b"\\??\\") {
        return true;
    }
    if !(b.len() >= 2 && sep(b[0]) && sep(b[1])) {
        return false;
    }
    // `\\?\X:\…` (a verbatim local drive) is the only double-separator
    // form allowed; `canonicalize` produces it for every local path.
    let verbatim_disk =
        b.len() >= 6 && b[2] == b'?' && sep(b[3]) && b[4].is_ascii_alphabetic() && b[5] == b':';
    !verbatim_disk
}

/// `[Q9-4]` a Windows DOS device name: the stem (before the first `.`,
/// trailing spaces / dots trimmed) is case-insensitively `CON`, `PRN`,
/// `AUX`, `NUL`, `COM0`–`COM9`, `LPT0`–`LPT9` (incl. the superscript
/// `¹²³` forms), `CONIN$` or `CONOUT$`. A pure predicate, tested on every
/// OS; the readers enforce it on Windows only (`aux` is a normal Unix name).
pub fn is_dos_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let stem = stem.trim_end_matches([' ', '.']).to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let Some(rest) = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
    else {
        return false;
    };
    let mut chars = rest.chars();
    let digit = chars
        .next()
        .is_some_and(|c| c.is_ascii_digit() || matches!(c, '¹' | '²' | '³'));
    digit && chars.next().is_none()
}

fn rejected_name(name: &std::ffi::OsStr) -> bool {
    cfg!(windows) && is_dos_device_name(&name.to_string_lossy())
}

fn net(p: &Path) -> bool {
    is_network_path(&p.to_string_lossy())
}

/// One work item of the resolver.
enum Item {
    Name(OsString),
    /// End of a link's target text: components after it are the original
    /// path again (so a later NotFound is Absent, not dangling) `[Q9-2]`.
    EndLink,
}

/// Pushes `p`'s components (reversed) onto the work stack; an absolute `p`
/// first resets `out` to its root / prefix. A rooted-but-not-absolute
/// target (Windows `\x`, `C:x`) has no fixed meaning ⇒ `false`.
fn push_components(p: &Path, out: &mut PathBuf, stack: &mut Vec<Item>) -> bool {
    let rooted = p
        .components()
        .next()
        .is_some_and(|c| matches!(c, Component::Prefix(_) | Component::RootDir));
    if rooted && !p.is_absolute() {
        return false;
    }
    let mut normal = Vec::new();
    let mut root = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => root.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => normal.push(Item::Name(OsString::from(".."))),
            Component::Normal(s) => normal.push(Item::Name(s.to_os_string())),
        }
    }
    if rooted {
        *out = root;
    }
    stack.extend(normal.into_iter().rev());
    true
}

/// Resolves every symlink of `path` (absolute), checking each target text
/// before following it. `follow == false` ⇒ any symlink is `NotAFile`.
fn resolve(path: &Path, follow: bool) -> Result<PathBuf, ReadOutcome> {
    if net(path) {
        return Err(ReadOutcome::NotAFile);
    }
    let mut out = PathBuf::new();
    let mut stack = Vec::new();
    if !push_components(path, &mut out, &mut stack) {
        return Err(ReadOutcome::Unreadable);
    }
    let mut hops = 0u32;
    // How many link targets are being consumed right now.
    let mut in_target = 0u32;
    while let Some(item) = stack.pop() {
        let part = match item {
            Item::EndLink => {
                in_target = in_target.saturating_sub(1);
                continue;
            }
            Item::Name(p) => p,
        };
        if part == ".." {
            out.pop();
            continue;
        }
        if rejected_name(&part) {
            return Err(ReadOutcome::NotAFile);
        }
        let cand = out.join(&part);
        let md = match fs::symlink_metadata(&cand) {
            // Missing in the original path ⇒ Absent; missing while
            // consuming a link's target ⇒ dangling ⇒ fail closed.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && in_target == 0 => {
                return Err(ReadOutcome::Absent)
            }
            Err(_) => return Err(ReadOutcome::Unreadable),
            Ok(md) => md,
        };
        if md.file_type().is_symlink() {
            if !follow {
                return Err(ReadOutcome::NotAFile);
            }
            hops += 1;
            if hops > MAX_HOPS {
                return Err(ReadOutcome::Unreadable);
            }
            let target = fs::read_link(&cand).map_err(|_| ReadOutcome::Unreadable)?;
            if net(&target) {
                return Err(ReadOutcome::NotAFile);
            }
            stack.push(Item::EndLink);
            in_target += 1;
            if !push_components(&target, &mut out, &mut stack) {
                return Err(ReadOutcome::Unreadable);
            }
            continue;
        }
        let more = stack.iter().any(|i| matches!(i, Item::Name(_)));
        if more && !md.is_dir() {
            // An intermediate component that is a file / device.
            return Err(ReadOutcome::NotAFile);
        }
        out = cand;
    }
    if net(&out) {
        return Err(ReadOutcome::NotAFile);
    }
    Ok(out)
}

fn read_resolved(dir: &Path, rel: &str, cap: u64, follow: bool) -> ReadOutcome {
    if !super::path_is_clean(rel) {
        return ReadOutcome::NotAFile;
    }
    let target = match resolve(&dir.join(rel), follow) {
        Ok(t) => t,
        Err(o) => return o,
    };
    let before = match fs::symlink_metadata(&target) {
        Ok(md) if md.is_file() => md,
        Ok(_) => return ReadOutcome::NotAFile,
        Err(_) => return ReadOutcome::Unreadable,
    };
    // [Q9-5] the resolved target is opened WITHOUT following a link in its
    // last component, and must be the very file `lstat` saw.
    let Ok(f) = open_nofollow(&target) else {
        return ReadOutcome::Unreadable;
    };
    match f.metadata() {
        Ok(md) if md.is_file() && same_file(&before, &md) => read_handle(f, cap),
        Ok(_) => ReadOutcome::NotAFile,
        Err(_) => ReadOutcome::Unreadable,
    }
}

/// `[Q9]` the following read of `dir/rel` for the FIXED user / system
/// config files (`rel` may be `.cargo/config.toml`).
pub fn read_config_follow(dir: &Path, rel: &str, cap: u64) -> ReadOutcome {
    read_resolved(dir, rel, cap, true)
}

/// `[Q9-narrowed]` the above-workdir walk read: symlinks are followed only
/// when `dir` equals one of `follow_roots` (the canonical `$HOME` /
/// `$CARGO_HOME`); anywhere else a symlinked component ⇒ `NotAFile`.
pub fn read_above_workdir_config(
    dir: &Path,
    rel: &str,
    cap: u64,
    follow_roots: &[PathBuf],
) -> ReadOutcome {
    let follow = follow_roots.iter().any(|r| r.as_path() == dir);
    read_resolved(dir, rel, cap, follow)
}

/// `[P125e]` the FOLLOWING resolution of a fixed user / system config
/// DIRECTORY, for the two places the real `HostEnv` needs a directory rather
/// than a file: listing the NuGet machine-wide `Config/*.config` set, and
/// vetting `$HOME` / `$CARGO_HOME` before they are canonicalized as follow
/// roots. Same resolver as [`read_config_follow`]: every link target is
/// checked BEFORE it is touched (a network / UNC / device-namespace target
/// is rejected unopened), so the caller's later `read_dir` / `canonicalize`
/// only ever walks a local path. `Ok` only for an existing directory; a
/// missing one is `Err(Absent)`; a relative path, a dangling link or a
/// non-directory target is not `Ok`.
pub fn resolve_config_dir(dir: &Path) -> Result<PathBuf, ReadOutcome> {
    if net(dir) {
        return Err(ReadOutcome::NotAFile);
    }
    if !dir.is_absolute() {
        return Err(ReadOutcome::Unreadable);
    }
    let out = resolve(dir, true)?;
    match fs::symlink_metadata(&out) {
        Ok(md) if md.is_dir() => Ok(out),
        Ok(_) => Err(ReadOutcome::NotAFile),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ReadOutcome::Absent),
        Err(_) => Err(ReadOutcome::Unreadable),
    }
}

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;
