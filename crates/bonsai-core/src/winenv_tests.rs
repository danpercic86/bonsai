//! Unit tests for [`super`] — the seam, the plan, and the apply half of P71 R2
//! (§7 M-8).
//!
//! HERMETIC BY CONSTRUCTION: every assertion goes through the table-backed
//! [`FakeWinEnv`], whose `set_path` records the value instead of calling
//! `std::env::set_var`. **No test mutates process state** (the suite runs
//! in-process and in parallel — an env write would race `gitbin`/`procutil`,
//! which read `PATH` in this same binary), and every case, including the
//! `applied: true` branch, executes identically on any host OS.

use super::fake::{rehydrate, FakeWinEnv};
use super::*;

// ---- plan_rehydration ---------------------------------------------------------

#[test]
fn plan_rehydration_expands_and_merges_both_hives() {
    let env = FakeWinEnv::default()
        .with_system_path(r"%SystemRoot%\system32")
        .with_user_path(r"%APPDATA%\npm")
        .with_var("SystemRoot", r"C:\Windows")
        .with_profile_var("APPDATA", r"C:\Users\dev\AppData\Roaming")
        .with_var("PATH", r"C:\Windows\system32;C:\installer-only");

    let (merged, added) = plan_rehydration(&env).expect("the npm shim dir is missing");
    assert_eq!(added, vec![r"C:\Users\dev\AppData\Roaming\npm"]);
    assert_eq!(
        merged,
        r"C:\Windows\system32;C:\installer-only;C:\Users\dev\AppData\Roaming\npm"
    );
}

#[test]
fn plan_rehydration_is_none_when_nothing_is_missing() {
    let env = FakeWinEnv::default()
        .with_system_path(r"C:\Windows")
        .with_user_path(r"C:\Users\dev\bin")
        .with_var("PATH", r"C:\users\dev\bin\;C:\windows");
    assert_eq!(plan_rehydration(&env), None);
}

#[test]
fn plan_rehydration_is_none_when_neither_registry_value_can_be_read() {
    // Malformed / missing / non-zero-exit `reg.exe` all surface as `None` from
    // the seam.
    let env = FakeWinEnv::default().with_var("PATH", r"C:\installer-only");
    assert_eq!(plan_rehydration(&env), None);
}

#[test]
fn plan_rehydration_tolerates_one_unreadable_hive() {
    // Only HKCU readable: the user entries are still recovered.
    let env = FakeWinEnv::default()
        .with_user_path(r"C:\Users\dev\bin")
        .with_var("PATH", r"C:\installer-only");
    let (merged, added) = plan_rehydration(&env).expect("user hive alone is enough");
    assert_eq!(added, vec![r"C:\Users\dev\bin"]);
    assert_eq!(merged, r"C:\installer-only;C:\Users\dev\bin");
}

#[test]
fn plan_rehydration_treats_an_all_empty_registry_value_as_nothing_to_do() {
    let env = FakeWinEnv::default()
        .with_system_path("")
        .with_user_path("   ")
        .with_var("PATH", r"C:\installer-only");
    assert_eq!(plan_rehydration(&env), None);
}

/// An unreadable process `PATH` must abort the whole rehydration.
///
/// `std::env::var` returns `Err` for BOTH "unset" and "set but not valid
/// Unicode" (an unpaired surrogate Windows holds happily). Treating that as an
/// EMPTY PATH would replace an inherited PATH we merely could not decode with
/// the registry entries alone — worse than the bug being fixed.
#[test]
fn plan_rehydration_is_none_when_the_process_path_cannot_be_read() {
    let env = FakeWinEnv::default()
        .with_system_path(r"C:\Windows")
        .with_user_path(r"C:\Users\dev\bin");
    assert_eq!(plan_rehydration(&env), None);
}

// ---- rehydrate_path (apply, through the seam) ---------------------------------

#[test]
fn rehydrate_path_applies_and_reports_the_added_entries() {
    // The `applied: true` branch — assertable on every host OS precisely
    // because the write goes through the seam.
    let env = FakeWinEnv::default()
        .with_system_path(r"C:\Windows\system32")
        .with_user_path(r"%LOCALAPPDATA%\Programs\Git\cmd")
        .with_profile_var("LOCALAPPDATA", r"C:\Users\dev\AppData\Local")
        .with_var("PATH", r"C:\Windows\system32;C:\installer-only");

    let (out, writes) = rehydrate(&env);
    assert!(out.applied);
    assert_eq!(out.added, vec![r"C:\Users\dev\AppData\Local\Programs\Git\cmd"]);
    assert_eq!(
        writes,
        vec![r"C:\Windows\system32;C:\installer-only;C:\Users\dev\AppData\Local\Programs\Git\cmd"]
    );
}

#[test]
fn rehydrate_path_reports_not_applied_when_the_write_is_refused() {
    // The production non-Windows path: `set_path` is a no-op, so the outcome
    // must not claim a write that never happened.
    let env = FakeWinEnv::default()
        .with_user_path(r"C:\Users\dev\bin")
        .with_var("PATH", r"C:\installer-only")
        .refusing_writes();
    let (out, writes) = rehydrate(&env);
    assert_eq!(out, PathRehydration::default());
    assert!(writes.is_empty());
}

#[test]
fn rehydrate_path_is_a_silent_no_op_when_the_registry_cannot_be_read() {
    let env = FakeWinEnv::default().with_var("PATH", r"C:\installer-only");
    let (out, writes) = rehydrate(&env);
    assert_eq!(out, PathRehydration::default());
    assert!(writes.is_empty());
}

#[test]
fn rehydrate_path_is_a_silent_no_op_when_nothing_is_missing() {
    let env = FakeWinEnv::default()
        .with_system_path(r"C:\Windows")
        .with_var("PATH", r"C:\Windows;C:\other");
    let (out, writes) = rehydrate(&env);
    assert!(!out.applied);
    assert!(out.added.is_empty());
    assert!(writes.is_empty());
}

#[test]
fn rehydrate_path_is_a_silent_no_op_on_garbage_registry_data() {
    // A value that expands to nothing but separators, plus a segment whose
    // variable does not resolve, must produce neither an empty PATH component
    // nor an "applied" result.
    let env = FakeWinEnv::default()
        .with_system_path(";;   ;;")
        .with_user_path("%UNSET%")
        .with_var("PATH", r"C:\installer-only");
    let (out, writes) = rehydrate(&env);
    assert_eq!(out, PathRehydration::default());
    assert!(writes.is_empty());
}

/// `std::env::set_var` PANICS on a value containing NUL — and this runs before
/// the first paint, so a panic would mean the app never opens. `reg.exe` output
/// is `String::from_utf8_lossy`'d and split on `\n` only, so an interior NUL
/// survives all the way here.
#[test]
fn rehydrate_path_refuses_a_merged_value_containing_a_nul_byte() {
    let env = FakeWinEnv::default()
        .with_user_path("C:\\Users\\dev\\b\0in")
        .with_var("PATH", r"C:\installer-only");
    let (out, writes) = rehydrate(&env);
    assert_eq!(out, PathRehydration::default());
    assert!(writes.is_empty(), "set_var must never see a NUL");
}

/// Same guarantee for the 32,767-unit Windows limit on one environment value:
/// `SetEnvironmentVariableW` fails, and `set_var` turns that failure into a
/// panic.
#[test]
fn rehydrate_path_refuses_an_over_long_merged_value() {
    let long_dir = format!(r"C:\{}", "x".repeat(20_000));
    let env = FakeWinEnv::default()
        .with_system_path(&long_dir)
        .with_user_path(&format!(r"C:\{}", "y".repeat(20_000)))
        .with_var("PATH", r"C:\installer-only");
    let (out, writes) = rehydrate(&env);
    assert_eq!(out, PathRehydration::default());
    assert!(writes.is_empty(), "set_var must never see an over-long value");
}

// ---- the real host seam -------------------------------------------------------

/// The only test that touches the real machine — and it is **inert by
/// construction**: [`plan_rehydration`] computes, it never writes.
///
/// Calling `rehydrate_path_once()` here instead would perform a real
/// `std::env::set_var` inside a multi-threaded test binary on any host whose
/// registry `PATH` holds an entry its process `PATH` lacks, racing every other
/// test that reads `PATH` (`gitbin`, `procutil`). This exercises the same
/// `reg.exe` spawn, parse and merge with none of that hazard.
#[test]
fn plan_rehydration_against_the_real_host_is_panic_free_and_consistent() {
    let first = plan_rehydration(&HostWinEnv::new());
    let second = plan_rehydration(&HostWinEnv::new());
    assert_eq!(first, second, "the plan must be deterministic");

    if let Some((merged, added)) = first {
        let process = std::env::var("PATH").unwrap_or_default();
        assert!(
            merged.starts_with(&process),
            "the inherited PATH must be emitted first, verbatim"
        );
        assert!(!added.is_empty());
        for entry in &added {
            assert!(
                is_absolute_windows_path(entry),
                "{entry} must be fully qualified"
            );
        }
    }
    // On non-Windows the host seam reads no registry at all, so there is
    // nothing to plan.
    if !cfg!(windows) {
        assert_eq!(plan_rehydration(&HostWinEnv::new()), None);
    }
}
