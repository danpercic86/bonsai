//! The config-file walks shared by every ecosystem loader (§4.4):
//! - the in-repo ancestor walk (`safe_read`, manifest dir → workdir);
//! - the `[Q7]` above-workdir walk (`HostEnv::read_above_workdir`,
//!   parent(workdir) → filesystem root, config files only);
//! - the user / system files (`HostEnv::read_user_file`).
//!
//! Every outcome other than Absent / Content is `Closed` `[HIGH-1 item 2]`.

use std::collections::BTreeSet;
use std::path::Path;

use crate::adapter::{dir_and_ancestors, file_name, join, parent_dir};
use crate::fsread::{path_is_clean, safe_read, ReadOutcome, MAX_CONFIG_BYTES};

use super::host_env::{HostEnv, UserConfig, UserRead};
use super::Closed;

/// The extractor callback: `(format tag, file text)`.
pub(super) type Extract<'a, T> = &'a mut dyn FnMut(T, &str) -> Result<(), Closed>;

/// `[HIGH-1 item 1]` every directory from each `manifest_name` manifest's
/// dir up to and including the workdir (`""`). An unclean manifest path is
/// `Closed`.
pub(super) fn manifest_dirs(
    manifests: &[String],
    manifest_name: &str,
) -> Result<BTreeSet<String>, Closed> {
    let mut dirs = BTreeSet::from([String::new()]);
    for m in manifests.iter().filter(|m| file_name(m) == manifest_name) {
        if !path_is_clean(m) {
            return Err(Closed);
        }
        dirs.extend(
            dir_and_ancestors(parent_dir(m))
                .into_iter()
                .map(str::to_string),
        );
    }
    Ok(dirs)
}

/// The in-repo walk: `files` (repo-relative, may contain `/`) in each dir.
pub(super) fn repo_walk<T: Copy>(
    workdir_canon: &Path,
    dirs: &BTreeSet<String>,
    files: &[(&str, T)],
    f: Extract<'_, T>,
) -> Result<(), Closed> {
    for dir in dirs {
        for (name, tag) in files {
            match safe_read(workdir_canon, &join(dir, name), MAX_CONFIG_BYTES) {
                ReadOutcome::Absent => {}
                ReadOutcome::Content(text) => f(*tag, &text)?,
                // [HIGH-1 item 2] symlink, FIFO, directory, oversize, EACCES.
                ReadOutcome::NotAFile | ReadOutcome::TooLarge | ReadOutcome::Unreadable => {
                    return Err(Closed)
                }
            }
        }
    }
    Ok(())
}

pub(super) fn apply_user_read<T>(r: UserRead, tag: T, f: Extract<'_, T>) -> Result<(), Closed> {
    match r {
        UserRead::Absent => Ok(()),
        UserRead::Content(text) => f(tag, &text),
        UserRead::Unreadable => Err(Closed),
    }
}

/// `[Q7]` every ancestor ABOVE the workdir, nearest first, to the root.
/// Config files only; manifests and lockfiles are never read here.
pub(super) fn above_walk<T: Copy>(
    workdir_canon: &Path,
    env: &dyn HostEnv,
    files: &[(&str, T)],
    f: Extract<'_, T>,
) -> Result<(), Closed> {
    for dir in workdir_canon.ancestors().skip(1) {
        for (name, tag) in files {
            apply_user_read(env.read_above_workdir(dir, name), *tag, f)?;
        }
    }
    Ok(())
}

pub(super) fn user_files<T: Copy>(
    env: &dyn HostEnv,
    files: &[(UserConfig, T)],
    f: Extract<'_, T>,
) -> Result<(), Closed> {
    for (which, tag) in files {
        apply_user_read(env.read_user_file(*which), *tag, f)?;
    }
    Ok(())
}
