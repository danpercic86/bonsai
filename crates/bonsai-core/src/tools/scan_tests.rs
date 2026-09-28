//! Scan-assembly and selection tests (P112 AC13 `present`, AC12's stale
//! selection, and the bundle normalisation half of AC4).
//!
//! **No test here calls `tool_scan` / `refresh_tool_scan` / `picked`**: those
//! populate the process-global scan cache and would spawn `reg.exe` for the
//! `AppPaths` rungs. Everything goes through the explicit-parameter halves
//! (`scan_from_rows`, `picked_from`) with hand-built rows, which is also what
//! makes every OS assertable from one machine.

use std::fs;
use std::path::{Path, PathBuf};

use crate::external::TargetOs;

use super::catalog::{self, Recipe, CUSTOM_ID};
use super::fake::FakeToolEnv;
use super::{
    picked_from, scan_from_rows, DetectedTool, Resolution, ToolEntry, ToolKind, ToolSource,
};

const AT_MS: u64 = 1_700_000_000_000;

fn row(
    kind: ToolKind,
    id: &str,
    os: TargetOs,
    res: Resolution,
) -> (&'static ToolEntry, Resolution) {
    let entry = catalog::find_for(kind, id, os).expect("catalog row (AC8 pins totality)");
    (entry, res)
}

fn exe_res(path: &str, source: ToolSource) -> Resolution {
    Resolution {
        program: path.to_string(),
        bundle: None,
        source,
    }
}

fn built_in_res(program: &str) -> Resolution {
    Resolution {
        program: program.to_string(),
        bundle: None,
        source: ToolSource::BuiltIn,
    }
}

fn bundle_res(bundle: &str) -> Resolution {
    Resolution {
        program: "open".to_string(),
        bundle: Some(bundle.to_string()),
        source: ToolSource::AppBundle,
    }
}

fn ids(rows: &[DetectedTool]) -> Vec<&str> {
    rows.iter().map(|r| r.id.as_str()).collect()
}

/// A file the HOST's browsed-program rules accept (`.exe` on Windows, the
/// execute bit on unix), so the `present: true` case is provable anywhere.
fn browsable_program(dir: &Path, stem: &str) -> PathBuf {
    let path = if cfg!(windows) {
        dir.join(format!("{stem}.exe"))
    } else {
        dir.join(stem)
    };
    fs::write(&path, b"stub").expect("write stub program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("set the execute bit");
    }
    path
}

mod detail;
mod present;
mod selection;
