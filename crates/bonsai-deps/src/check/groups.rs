//! §5.1 step 3: one parse per discovered manifest.
//!
//! - `catch_unwind` around each `adapter.parse` ⇒ `Internal` (panics only;
//!   memory is bounded by the §4.2 caps, not by this) `[P125b review]`.
//! - The `MAX_PARSE_BYTES` budget (manifest + lockfile bytes, summed over
//!   the check): once a group would cross it, that group and every later
//!   one is `LimitExceeded` without being parsed `[LOW-3]`.
//! - The `MAX_MANIFESTS` overflow is one extra `LimitExceeded` group.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use crate::adapter::{adapter_for_manifest, file_name, ParseError, ParseInput, ParsedGroup};
use crate::catalog::not_tracked_label;
use crate::discover::FileSet;
use crate::fsread::{safe_read, ReadOutcome, MAX_FILE_BYTES};
use crate::model::{Ecosystem, ParseErrorKind};

use super::{CancelToken, CheckError};

/// One group before classification.
pub(crate) enum GroupInput {
    Parsed {
        manifest_path: String,
        lockfile_path: Option<String>,
        eco: Ecosystem,
        group: ParsedGroup,
    },
    Failed {
        manifest_path: String,
        lockfile_path: Option<String>,
        eco: Option<Ecosystem>,
        err: ParseError,
    },
    NotTracked {
        path: String,
        label: &'static str,
    },
}

fn read_kind(o: &ReadOutcome) -> Option<ParseErrorKind> {
    match o {
        ReadOutcome::Content(_) => None,
        ReadOutcome::TooLarge => Some(ParseErrorKind::TooLarge),
        ReadOutcome::NotAFile => Some(ParseErrorKind::NotAFile),
        // Vanished since discovery, or EACCES.
        ReadOutcome::Absent | ReadOutcome::Unreadable => Some(ParseErrorKind::Unreadable),
    }
}

/// Runs `parse`, turning a panic into `Internal`.
pub(crate) fn guarded_parse(
    file: &str,
    parse: impl FnOnce() -> Result<ParsedGroup, ParseError>,
) -> Result<ParsedGroup, ParseError> {
    catch_unwind(AssertUnwindSafe(parse))
        .unwrap_or_else(|_| Err(ParseError::new(file, ParseErrorKind::Internal)))
}

/// Tracks the per-check parse budget.
pub(crate) struct Budget {
    left: u64,
    tripped: bool,
}

impl Budget {
    pub(crate) fn new(max: u64) -> Self {
        Budget {
            left: max,
            tripped: false,
        }
    }

    /// Takes `n` bytes; false (and false forever after) once exhausted.
    pub(crate) fn take(&mut self, n: u64) -> bool {
        if self.tripped || n > self.left {
            self.tripped = true;
            return false;
        }
        self.left -= n;
        true
    }
}

fn parse_one(wd: &Path, files: &FileSet, m: &str, budget: &mut Budget) -> Option<GroupInput> {
    let adapter = adapter_for_manifest(file_name(m))?;
    let eco = adapter.ecosystem();
    let lockfile_path = adapter.lockfile_for(m, files);
    let fail = |file: &str, kind| GroupInput::Failed {
        manifest_path: m.to_string(),
        lockfile_path: lockfile_path.clone(),
        eco: Some(eco),
        err: ParseError::new(file, kind),
    };
    if budget.tripped {
        return Some(fail(m, ParseErrorKind::LimitExceeded));
    }
    let manifest = safe_read(wd, m, MAX_FILE_BYTES);
    if let Some(kind) = read_kind(&manifest) {
        return Some(fail(m, kind));
    }
    let ReadOutcome::Content(manifest) = manifest else {
        return Some(fail(m, ParseErrorKind::Unreadable));
    };
    let lock = match &lockfile_path {
        Some(p) => match safe_read(wd, p, MAX_FILE_BYTES) {
            ReadOutcome::Content(t) => Some((p.as_str(), t)),
            other => {
                return Some(fail(
                    p,
                    read_kind(&other).unwrap_or(ParseErrorKind::Unreadable),
                ))
            }
        },
        None => None,
    };
    let bytes = manifest.len() as u64 + lock.as_ref().map_or(0, |(_, t)| t.len() as u64);
    if !budget.take(bytes) {
        return Some(fail(m, ParseErrorKind::LimitExceeded));
    }
    let input = ParseInput {
        manifest_path: m,
        manifest: &manifest,
        lockfile: lock.as_ref().map(|(p, t)| (*p, t.as_str())),
        files,
    };
    Some(match guarded_parse(m, || adapter.parse(&input)) {
        Ok(group) => GroupInput::Parsed {
            manifest_path: m.to_string(),
            lockfile_path: lockfile_path.clone(),
            eco,
            group,
        },
        Err(err) => GroupInput::Failed {
            manifest_path: m.to_string(),
            lockfile_path: lockfile_path.clone(),
            eco: Some(eco),
            err,
        },
    })
}

/// Every group of the check, manifests (sorted) first.
pub(crate) fn parse_all(
    wd: &Path,
    files: &FileSet,
    max_bytes: u64,
    cancel: &CancelToken,
) -> Result<Vec<GroupInput>, CheckError> {
    let mut budget = Budget::new(max_bytes);
    let mut out = Vec::new();
    for m in files.manifests() {
        if cancel.is_cancelled() {
            return Err(CheckError::Cancelled);
        }
        out.extend(parse_one(wd, files, m, &mut budget));
    }
    for p in files.not_tracked() {
        if let Some(label) = not_tracked_label(file_name(p)) {
            out.push(GroupInput::NotTracked {
                path: p.clone(),
                label,
            });
        }
    }
    if files.overflow > 0 {
        let file = format!("<{} more manifests>", files.overflow);
        out.push(GroupInput::Failed {
            manifest_path: file.clone(),
            lockfile_path: None,
            eco: None,
            err: ParseError::new(&file, ParseErrorKind::LimitExceeded),
        });
    }
    Ok(out)
}
