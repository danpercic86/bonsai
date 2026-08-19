//! Unit tests for [`super`] — the pure half of P71 R2 (§7 M-8).
//!
//! HERMETIC BY CONSTRUCTION: every assertion goes through a table-backed
//! [`FakeWinEnv`], so **no test mutates `std::env`** (the suite runs in-process
//! and in parallel — an env write is a cross-test hazard) and every case
//! executes identically on any host OS.

use super::*;
use crate::winenv::fake::FakeWinEnv;

/// No `%VAR%` in the input ⇒ the env is never consulted.
fn no_env() -> FakeWinEnv {
    FakeWinEnv::default()
}

// ---- merge_path ---------------------------------------------------------------

#[test]
fn merge_path_no_op_when_every_entry_is_already_present() {
    // Registry entries all present (in a different order, different case) =>
    // nothing to add => None, so the caller never calls set_var.
    assert_eq!(
        merge_path(
            r"C:\Windows\system32;C:\Windows",
            r"C:\Users\dev\bin",
            r"C:\Users\DEV\BIN;C:\windows;C:\WINDOWS\SYSTEM32",
            &no_env(),
        ),
        None
    );
}

#[test]
fn merge_path_appends_the_missing_user_entry() {
    let process = r"C:\Windows\system32;C:\Program Files\dotnet";
    let (merged, added) = merge_path(
        r"C:\Windows\system32",
        r"C:\Users\dev\AppData\Local\Programs\Git\cmd",
        process,
        &no_env(),
    )
    .expect("a missing entry must produce a merge");

    assert_eq!(added, vec![r"C:\Users\dev\AppData\Local\Programs\Git\cmd"]);
    assert_eq!(
        merged,
        format!(r"{process};C:\Users\dev\AppData\Local\Programs\Git\cmd")
    );
}

#[test]
fn merge_path_orders_the_process_path_first_then_system_then_user() {
    let process = r"C:\msiexec-only";
    let (merged, added) = merge_path(r"C:\sys1;C:\sys2", r"C:\usr1;C:\usr2", process, &no_env())
        .expect("four missing entries");

    assert_eq!(added, vec![r"C:\sys1", r"C:\sys2", r"C:\usr1", r"C:\usr2"]);
    assert_eq!(merged, format!(r"{process};C:\sys1;C:\sys2;C:\usr1;C:\usr2"));
}

/// **Regression guard for the prepend→append reversal (contract §5.5).**
///
/// Prepend put the whole recovered block ahead of the inherited PATH, which in
/// the msiexec case (inherited PATH ≈ the machine PATH) placed the
/// user-writable `…\Microsoft\WindowsApps` ahead of `C:\Windows\System32` for
/// this process AND every child it spawns. Append cannot: a recovered entry
/// never precedes an inherited one. Do not re-flip this.
#[test]
fn recovered_entries_never_precede_inherited_ones_append_reversal_p71() {
    let process = r"C:\Windows\system32;C:\Windows;C:\Program Files\Git\cmd";
    let (merged, added) = merge_path(
        r"C:\Windows\system32;C:\Windows",
        r"C:\Users\dev\AppData\Local\Microsoft\WindowsApps;C:\Users\dev\bin",
        process,
        &no_env(),
    )
    .expect("two missing user entries");

    // The inherited block is emitted first, byte-for-byte...
    assert!(
        merged.starts_with(process),
        "the inherited PATH must come first, verbatim"
    );
    // ...and every recovered entry lands strictly after it.
    let tail = &merged[process.len()..];
    for entry in &added {
        assert!(
            tail.contains(entry.as_str()),
            "{entry} must be appended, not prepended"
        );
    }
    // The concrete shadowing case the reversal exists to prevent.
    let system32 = merged
        .find(r"C:\Windows\system32")
        .expect("system32 stays on the PATH");
    let windows_apps = merged
        .find(r"WindowsApps")
        .expect("WindowsApps is recovered");
    assert!(
        system32 < windows_apps,
        "a user-writable recovered dir must never precede System32"
    );
}

#[test]
fn merge_path_copies_the_process_path_through_verbatim() {
    // Deliberately hostile: duplicated entries, an entry that also appears in
    // the registry, an EMPTY component, and an unsorted order. None of it may
    // be reordered, deduplicated, or dropped — it is emitted byte-for-byte.
    let process = r"C:\dup;C:\b;C:\dup;;C:\a";
    let (merged, added) =
        merge_path(r"C:\sys", r"C:\b", process, &no_env()).expect("the system entry is missing");

    assert_eq!(added, vec![r"C:\sys"]);
    assert_eq!(merged, format!(r"{process};C:\sys"));
    assert!(
        merged.starts_with(process),
        "process PATH must survive intact"
    );
}

#[test]
fn merge_path_does_not_add_a_separator_after_a_trailing_one() {
    // A trailing `;` is already an empty ("current directory") component; we
    // must not turn it into an interior one by adding a second separator.
    let (merged, _) = merge_path(r"C:\sys", "", r"C:\keep;", &no_env()).expect("one missing entry");
    assert_eq!(merged, r"C:\keep;C:\sys");
}

#[test]
fn merge_path_compares_case_insensitively_and_ignores_trailing_separators() {
    // `C:\Tools\` (registry, trailing backslash) == `c:\tools` (process,
    // lowercase) => nothing missing.
    assert_eq!(merge_path(r"C:\Tools\", "", r"c:\tools", &no_env()), None);
    // A trailing FORWARD slash is trimmed too, and surrounding whitespace.
    assert_eq!(merge_path("", r"  C:\Tools/  ", r"C:\TOOLS", &no_env()), None);
    // Only TRAILING separators are normalized: an interior `/` is a different
    // string, so the entry is treated as missing (deliberately conservative —
    // appending a duplicate spelling is harmless, dropping a real entry is
    // not).
    assert!(merge_path(r"C:/Tools", "", r"C:\Tools", &no_env()).is_some());
}

#[test]
fn merge_path_ignores_empty_registry_segments_and_deduplicates_its_own_additions() {
    // Empty components must never be introduced (an empty PATH entry means
    // "current directory"), and a directory listed in BOTH hives is appended
    // once.
    let (merged, added) = merge_path(r";C:\shared;;", r"C:\shared\;C:\extra", r"C:\keep", &no_env())
        .expect("two distinct missing entries");

    assert_eq!(added, vec![r"C:\shared", r"C:\extra"]);
    assert_eq!(merged, r"C:\keep;C:\shared;C:\extra");
}

#[test]
fn merge_path_handles_an_empty_process_path_without_a_leading_separator() {
    let (merged, added) = merge_path(r"C:\sys", "", "", &no_env()).expect("one missing entry");
    assert_eq!(added, vec![r"C:\sys"]);
    assert_eq!(merged, r"C:\sys");
}

#[test]
fn merge_path_drops_every_segment_the_guards_reject() {
    // `%UNSET%\tools` cannot expand, `\rel` and `tools` and `..` are not
    // absolute: all four are dropped and never reach `added`. Only the one
    // sound entry survives.
    let (merged, added) = merge_path(
        r"%UNSET%\tools;\rel;tools;..",
        r"C:\good",
        r"C:\keep",
        &no_env(),
    )
    .expect("the one absolute entry survives");
    assert_eq!(added, vec![r"C:\good"]);
    assert_eq!(merged, r"C:\keep;C:\good");
}

#[test]
fn merge_path_is_none_when_every_registry_segment_is_rejected() {
    assert_eq!(
        merge_path(r"%UNSET%\tools;.;", r"\drive-relative", r"C:\keep", &no_env()),
        None
    );
}

// ---- expand_segment -----------------------------------------------------------

#[test]
fn expand_segment_resolves_machine_scope_names_from_the_process_env() {
    // SystemRoot / ProgramFiles are identical for every process on the box, so
    // the inherited value is fine (contract §5.3.1).
    let env = FakeWinEnv::default()
        .with_var("SystemRoot", r"C:\Windows")
        .with_var("ProgramFiles", r"C:\Program Files");
    assert_eq!(
        expand_segment(r"%SystemRoot%\system32", &env).as_deref(),
        Some(r"C:\Windows\system32")
    );
    assert_eq!(
        expand_segment(r"%ProgramFiles%\Git\cmd", &env).as_deref(),
        Some(r"C:\Program Files\Git\cmd")
    );
    assert!(env.read_process_var("SystemRoot"));
}

#[test]
fn expand_segment_resolves_profile_vars_from_the_volatile_block_not_the_process_env() {
    // THE §5.3.1 case: under msiexec the inherited %LOCALAPPDATA% points into
    // the SYSTEM profile. The profile block must win, and the foreign value
    // must not even be consulted.
    let env = FakeWinEnv::default()
        .with_profile_var("LOCALAPPDATA", r"C:\Users\dev\AppData\Local")
        .with_var(
            "LOCALAPPDATA",
            r"C:\Windows\system32\config\systemprofile\AppData\Local",
        );

    assert_eq!(
        expand_segment(r"%LOCALAPPDATA%\Programs\Git\cmd", &env).as_deref(),
        Some(r"C:\Users\dev\AppData\Local\Programs\Git\cmd")
    );
    assert!(
        !env.read_process_var("LOCALAPPDATA"),
        "the inherited (systemprofile) value must never be consulted"
    );
}

#[test]
fn expand_segment_falls_back_to_the_process_env_when_the_profile_block_is_unreadable() {
    let env = FakeWinEnv::default().with_var("APPDATA", r"C:\Users\dev\AppData\Roaming");
    assert_eq!(
        expand_segment(r"%APPDATA%\npm", &env).as_deref(),
        Some(r"C:\Users\dev\AppData\Roaming\npm")
    );
    assert!(env.read_process_var("APPDATA"));
}

#[test]
fn expand_segment_matches_profile_var_names_case_insensitively() {
    let env = FakeWinEnv::default()
        .with_profile_var("USERPROFILE", r"C:\Users\dev")
        .with_var("USERPROFILE", r"C:\Windows\system32\config\systemprofile");
    assert_eq!(
        expand_segment(r"%userprofile%\bin", &env).as_deref(),
        Some(r"C:\Users\dev\bin")
    );
    assert!(!env.read_process_var("userprofile"));
}

#[test]
fn expand_segment_drops_a_segment_with_an_unresolvable_name() {
    // NOT expanded to empty: `%NOPE%\bin` must not become the drive-relative
    // `\bin` (contract §5.3.2).
    assert_eq!(expand_segment(r"%NOPE%\bin", &no_env()), None);
    // An empty registry value counts as unresolvable too.
    let blank = FakeWinEnv::default().with_var("BLANK", "   ");
    assert_eq!(expand_segment(r"%BLANK%\bin", &blank), None);
}

#[test]
fn expand_segment_leaves_an_unterminated_percent_literal() {
    let env = FakeWinEnv::default().with_var("SystemRoot", r"C:\Windows");
    assert_eq!(
        expand_segment(r"%SystemRoot%\100%dir", &env).as_deref(),
        Some(r"C:\Windows\100%dir")
    );
    assert_eq!(expand_segment("%", &env).as_deref(), Some("%"));
    // ...and the absolute guard is what rejects the residue.
    assert!(!is_absolute_windows_path("%"));
}

#[test]
fn expand_segment_does_not_recurse() {
    // A self-referential value must expand exactly once — no loop, no
    // unbounded growth.
    let env = FakeWinEnv::default().with_var("Path", "%Path%");
    assert_eq!(expand_segment("%Path%", &env).as_deref(), Some("%Path%"));
    assert_eq!(expand_segment("", &env), None);
    assert_eq!(expand_segment("   ", &env), None);
    // `%%` is the empty name: unresolvable, so the segment is dropped.
    assert_eq!(expand_segment(r"C:\a%%b", &env), None);
}

// ---- is_absolute_windows_path -------------------------------------------------

#[test]
fn is_absolute_windows_path_accepts_only_fully_qualified_paths() {
    for ok in [
        r"C:\Tools",
        r"c:/tools",
        r"\\srv\share",
        r"//srv/share",
        r"Z:\",
        r"  C:\Tools  ",
    ] {
        assert!(is_absolute_windows_path(ok), "{ok} must be accepted");
    }
    for bad in [
        r"\tools",     // drive-relative: resolves against the current drive root
        r"C:tools",    // drive-current
        "tools",       // bare relative
        ".",           // the CWD — an arbitrary user-chosen repository
        "..",          //
        "",            //
        r"%VAR%\bin",  // unexpanded residue
        r"\\",         // no share
        "C:",          // drive with no root
    ] {
        assert!(!is_absolute_windows_path(bad), "{bad:?} must be rejected");
    }
}

// ---- reg.exe parsing ----------------------------------------------------------

#[test]
fn parse_reg_query_table() {
    let expand = "\r\nHKEY_CURRENT_USER\\Environment\r\n    Path    REG_EXPAND_SZ    %USERPROFILE%\\bin;C:\\Tools\r\n\r\n";
    assert_eq!(
        parse_reg_query(expand, "Path").as_deref(),
        Some(r"%USERPROFILE%\bin;C:\Tools")
    );

    // The stored casing differs between hives / Windows builds, so the value
    // name must match case-INsensitively (this is the one behavioural
    // difference from `gitbin::parse_reg_query`).
    let upper = "    PATH    REG_SZ    C:\\Windows\r\n";
    assert_eq!(parse_reg_query(upper, "Path").as_deref(), Some(r"C:\Windows"));

    // A path containing spaces survives; only the surrounding padding is cut.
    let spaced = "    Path    REG_SZ    C:\\Program Files\\Git\\cmd\r\n";
    assert_eq!(
        parse_reg_query(spaced, "Path").as_deref(),
        Some(r"C:\Program Files\Git\cmd")
    );

    // A value name that is a PREFIX of another must not cross-match.
    let prefixed = "    PathExt    REG_SZ    .COM;.EXE\r\n    Path    REG_SZ    C:\\right\r\n";
    assert_eq!(parse_reg_query(prefixed, "Path").as_deref(), Some(r"C:\right"));

    // Defensive: empty, localized error text, wrong type, empty data, a bare
    // name, and raw garbage all yield None rather than a panic.
    assert_eq!(parse_reg_query("", "Path"), None);
    assert_eq!(parse_reg_query("ERROR: The system was unable to find", "Path"), None);
    assert_eq!(parse_reg_query("    Path    REG_DWORD    0x1\r\n", "Path"), None);
    assert_eq!(parse_reg_query("    Path    REG_SZ    \r\n", "Path"), None);
    assert_eq!(parse_reg_query("Path", "Path"), None);
    assert_eq!(parse_reg_query("random \u{fffd} garbage \0 output", "Path"), None);
}

#[test]
fn parse_reg_values_reads_a_whole_block() {
    // The `HKCU\Volatile Environment` shape: many values, one spawn.
    let block = concat!(
        "\r\nHKEY_CURRENT_USER\\Volatile Environment\r\n",
        "    APPDATA    REG_SZ    C:\\Users\\dev\\AppData\\Roaming\r\n",
        "    LOCALAPPDATA    REG_SZ    C:\\Users\\dev\\AppData\\Local\r\n",
        "    HOMEPATH    REG_SZ    \\Users\\dev\r\n",
        "    SESSIONNAME    REG_SZ    Console\r\n",
        "\r\n",
    );
    let values = parse_reg_values(block);
    assert_eq!(
        values
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        vec!["APPDATA", "LOCALAPPDATA", "HOMEPATH", "SESSIONNAME"]
    );
    assert_eq!(values[1].1, r"C:\Users\dev\AppData\Local");
    assert!(parse_reg_values("ERROR: The system was unable to find").is_empty());
}
