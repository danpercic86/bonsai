//! Unit tests for [`super`] (P70 §6.1 items 1–11).
//!
//! HERMETIC BY CONSTRUCTION: every test drives the ladder through
//! [`FakeGitEnv`] and an EXPLICIT [`TargetOs`], so **no test mutates
//! `std::env`** (the suite runs in-process and in parallel — an env write is a
//! cross-test hazard) and both platform ladders execute on any host.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::*;

/// Recording fake: fixed vars / files / registry values, plus an ordered call
/// log so the "later rungs are reached ONLY when the earlier candidate is
/// absent" invariant can be asserted as ORDER, not just as a result.
#[derive(Default)]
struct FakeGitEnv {
    vars: HashMap<String, String>,
    files: HashSet<PathBuf>,
    registry: HashMap<(String, String), String>,
    path_hit: Option<PathBuf>,
    calls: RefCell<Vec<String>>,
}

impl FakeGitEnv {
    fn with_var(mut self, k: &str, v: &str) -> Self {
        self.vars.insert(k.to_string(), v.to_string());
        self
    }

    fn with_file(mut self, p: &str) -> Self {
        self.files.insert(PathBuf::from(p));
        self
    }

    fn with_registry(mut self, key: &str, value: &str, data: &str) -> Self {
        self.registry
            .insert((key.to_string(), value.to_string()), data.to_string());
        self
    }

    fn with_path_hit(mut self, p: &str) -> Self {
        self.path_hit = Some(PathBuf::from(p));
        self
    }

    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    fn registry_calls(&self) -> usize {
        self.calls().iter().filter(|c| c.starts_with("reg:")).count()
    }
}

impl GitEnv for FakeGitEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.calls.borrow_mut().push(format!("var:{key}"));
        self.vars.get(key).cloned()
    }

    fn is_file(&self, p: &Path) -> bool {
        self.calls
            .borrow_mut()
            .push(format!("file:{}", p.to_string_lossy()));
        self.files.contains(p)
    }

    fn resolve_on_path(&self, program: &str) -> Option<PathBuf> {
        self.calls.borrow_mut().push(format!("path:{program}"));
        self.path_hit.clone()
    }

    fn registry_string(&self, key: &str, value: &str) -> Option<String> {
        self.calls.borrow_mut().push(format!("reg:{key}\\{value}"));
        self.registry
            .get(&(key.to_string(), value.to_string()))
            .cloned()
    }
}

const HKCU: &str = r"HKCU\SOFTWARE\GitForWindows";
const HKLM: &str = r"HKLM\SOFTWARE\GitForWindows";
const WOW: &str = r"HKLM\SOFTWARE\WOW6432Node\GitForWindows";

// 1. Explicit override wins verbatim — even when PATH would resolve AND the
//    overridden file does not exist (a bad override must surface as an honest
//    launch failure, not be silently replaced).
#[test]
fn override_wins_verbatim_without_validation() {
    let env = FakeGitEnv::default()
        .with_var(GIT_BIN_ENV, "/x/y/git")
        .with_path_hit("/usr/bin/git");
    let bin = resolve_ladder_for(&env, TargetOs::Linux);
    assert_eq!(bin.source, GitBinSource::Override);
    assert_eq!(bin.path, PathBuf::from("/x/y/git"));
    assert!(bin.found());
    // Never validated, never fell through to PATH.
    assert_eq!(env.calls(), vec![format!("var:{GIT_BIN_ENV}")]);
}

// 1b. A trailing-whitespace override is still honored (trimmed).
#[test]
fn override_is_trimmed() {
    let env = FakeGitEnv::default().with_var(GIT_BIN_ENV, "  /x/y/git \n");
    let bin = resolve_ladder_for(&env, TargetOs::Linux);
    assert_eq!(bin.path, PathBuf::from("/x/y/git"));
    assert_eq!(bin.source, GitBinSource::Override);
}

// 2. Empty / whitespace-only override is ignored; the ladder continues.
#[test]
fn blank_override_is_ignored() {
    for blank in ["", "   ", "\t\n"] {
        let env = FakeGitEnv::default()
            .with_var(GIT_BIN_ENV, blank)
            .with_path_hit("/usr/bin/git");
        let bin = resolve_ladder_for(&env, TargetOs::Linux);
        assert_eq!(bin.source, GitBinSource::Path, "blank override {blank:?}");
        assert_eq!(bin.path, PathBuf::from("/usr/bin/git"));
    }
}

// 3. A PATH hit short-circuits: registry / well-known are NEVER consulted.
#[test]
fn path_hit_short_circuits_registry_and_well_known() {
    let env = FakeGitEnv::default()
        .with_path_hit(r"C:\tools\git\cmd\git.exe")
        .with_registry(HKCU, "InstallPath", r"C:\Users\dev\AppData\Local\Programs\Git")
        .with_var("LOCALAPPDATA", r"C:\Users\dev\AppData\Local");
    let bin = resolve_ladder_for(&env, TargetOs::Windows);
    assert_eq!(bin.source, GitBinSource::Path);
    assert_eq!(bin.path, PathBuf::from(r"C:\tools\git\cmd\git.exe"));
    assert_eq!(env.registry_calls(), 0, "registry must not be probed");
    assert_eq!(
        env.calls(),
        vec![format!("var:{GIT_BIN_ENV}"), "path:git".to_string()]
    );
    // A PATH-sourced binary needs no child-PATH repair.
    assert_eq!(bin.bin_dir(), None);
}

// 4. The exact field case: per-user install, PATH without git, HKCU InstallPath
//    present and `<install>\cmd\git.exe` on disk.
#[test]
fn hkcu_registry_hit_when_path_misses() {
    let install = r"C:\Users\dev\AppData\Local\Programs\Git";
    let exe = r"C:\Users\dev\AppData\Local\Programs\Git\cmd\git.exe";
    let env = FakeGitEnv::default()
        .with_registry(HKCU, "InstallPath", install)
        .with_file(exe);
    let bin = resolve_ladder_for(&env, TargetOs::Windows);
    assert_eq!(bin.source, GitBinSource::Registry);
    assert_eq!(bin.path, PathBuf::from(exe));
    // HKCU is probed FIRST, and the machine-wide keys are never reached.
    let calls = env.calls();
    assert_eq!(calls[2], format!("reg:{HKCU}\\InstallPath"));
    assert!(!calls.iter().any(|c| c.contains("HKLM")), "{calls:?}");
    assert_eq!(bin.bin_dir(), Some(Path::new(r"C:\Users\dev\AppData\Local\Programs\Git\cmd")));
}

// 4b. A trailing separator on InstallPath does not produce a doubled separator.
#[test]
fn registry_install_path_trailing_separator_is_trimmed() {
    let exe = r"C:\Program Files\Git\cmd\git.exe";
    let env = FakeGitEnv::default()
        .with_registry(HKLM, "InstallPath", r"C:\Program Files\Git\")
        .with_file(exe);
    let bin = resolve_ladder_for(&env, TargetOs::Windows);
    assert_eq!(bin.source, GitBinSource::Registry);
    assert_eq!(bin.path, PathBuf::from(exe));
}

// 5. HKCU present but its `cmd\git.exe` is NOT a file -> fall through HKLM ->
//    WOW6432Node -> well-known, in that exact order.
#[test]
fn registry_falls_through_when_candidate_is_absent() {
    let exe = r"C:\Users\dev\AppData\Local\Programs\Git\cmd\git.exe";
    let env = FakeGitEnv::default()
        .with_registry(HKCU, "InstallPath", r"C:\stale\hkcu\Git")
        .with_registry(HKLM, "InstallPath", r"C:\stale\hklm\Git")
        .with_registry(WOW, "InstallPath", r"C:\stale\wow\Git")
        .with_var("LOCALAPPDATA", r"C:\Users\dev\AppData\Local")
        .with_file(exe);
    let bin = resolve_ladder_for(&env, TargetOs::Windows);
    assert_eq!(bin.source, GitBinSource::WellKnown);
    assert_eq!(bin.path, PathBuf::from(exe));

    let calls = env.calls();
    let reg_order: Vec<&String> = calls.iter().filter(|c| c.starts_with("reg:")).collect();
    assert_eq!(
        reg_order,
        vec![
            &format!("reg:{HKCU}\\InstallPath"),
            &format!("reg:{HKLM}\\InstallPath"),
            &format!("reg:{WOW}\\InstallPath"),
        ]
    );
    // Every stale candidate WAS existence-checked before moving on.
    assert!(calls.contains(&r"file:C:\stale\hkcu\Git\cmd\git.exe".to_string()));
    assert!(calls.contains(&r"file:C:\stale\wow\Git\cmd\git.exe".to_string()));
}

// 6. Well-known ordering: LOCALAPPDATA first; the three ProgramFiles* vars are
//    consulted only AFTER it misses.
#[test]
fn well_known_localappdata_wins_and_program_files_come_after() {
    let local_exe = r"C:\Users\dev\AppData\Local\Programs\Git\cmd\git.exe";
    let env = FakeGitEnv::default()
        .with_var("LOCALAPPDATA", r"C:\Users\dev\AppData\Local")
        .with_var("ProgramFiles", r"C:\Program Files")
        .with_file(local_exe)
        .with_file(r"C:\Program Files\Git\cmd\git.exe");
    let bin = resolve_ladder_for(&env, TargetOs::Windows);
    assert_eq!(bin.source, GitBinSource::WellKnown);
    assert_eq!(bin.path, PathBuf::from(local_exe));
    assert!(
        !env.calls().contains(&"var:ProgramFiles".to_string()),
        "ProgramFiles must not be consulted after a LOCALAPPDATA hit"
    );
}

// 6b. LOCALAPPDATA miss -> ProgramFiles -> ProgramW6432 -> ProgramFiles(x86).
#[test]
fn well_known_var_probe_order() {
    let exe = r"C:\Program Files (x86)\Git\cmd\git.exe";
    let env = FakeGitEnv::default()
        .with_var("LOCALAPPDATA", r"C:\Users\dev\AppData\Local")
        .with_var("ProgramFiles", r"C:\Program Files")
        .with_var("ProgramW6432", r"C:\Program Files")
        .with_var("ProgramFiles(x86)", r"C:\Program Files (x86)")
        .with_file(exe);
    let bin = resolve_ladder_for(&env, TargetOs::Windows);
    assert_eq!(bin.path, PathBuf::from(exe));
    assert_eq!(bin.source, GitBinSource::WellKnown);

    let calls = env.calls();
    let var_order: Vec<&String> = calls
        .iter()
        .filter(|c| {
            c.starts_with("var:") && *c != &format!("var:{GIT_BIN_ENV}")
        })
        .collect();
    assert_eq!(
        var_order,
        vec![
            &"var:LOCALAPPDATA".to_string(),
            &"var:ProgramFiles".to_string(),
            &"var:ProgramW6432".to_string(),
            &"var:ProgramFiles(x86)".to_string(),
        ]
    );
}

// 7. Everything missing -> bare-name fallback, `found() == false`.
#[test]
fn total_miss_falls_back_to_bare_name() {
    for os in [TargetOs::Windows, TargetOs::Linux, TargetOs::MacOs] {
        let env = FakeGitEnv::default();
        let bin = resolve_ladder_for(&env, os);
        assert_eq!(bin.source, GitBinSource::Fallback, "{os:?}");
        assert_eq!(bin.path, PathBuf::from("git"));
        assert!(!bin.found());
        assert_eq!(bin.bin_dir(), None);
    }
}

// 8. Unix ladder ordering: /usr/bin -> /usr/local/bin -> /opt/homebrew/bin.
#[test]
fn unix_well_known_order() {
    let env = FakeGitEnv::default()
        .with_file("/usr/local/bin/git")
        .with_file("/opt/homebrew/bin/git");
    let bin = resolve_ladder_for(&env, TargetOs::MacOs);
    assert_eq!(bin.source, GitBinSource::WellKnown);
    assert_eq!(bin.path, PathBuf::from("/usr/local/bin/git"));
    let calls = env.calls();
    let files: Vec<&String> = calls.iter().filter(|c| c.starts_with("file:")).collect();
    assert_eq!(
        files,
        vec![
            &"file:/usr/bin/git".to_string(),
            &"file:/usr/local/bin/git".to_string(),
        ]
    );
    // The Unix ladder never touches the registry.
    assert_eq!(env.registry_calls(), 0);
}

// 8b. /usr/bin wins when present.
#[test]
fn unix_usr_bin_wins() {
    let env = FakeGitEnv::default()
        .with_file("/usr/bin/git")
        .with_file("/opt/homebrew/bin/git");
    let bin = resolve_ladder_for(&env, TargetOs::Linux);
    assert_eq!(bin.path, PathBuf::from("/usr/bin/git"));
    assert_eq!(bin.bin_dir(), Some(Path::new("/usr/bin")));
}

// 9. D4 — resolution NEVER executes a candidate. The `GitEnv` seam has no
//    execution capability at all, so the recorded call log can only contain
//    var / path / file / reg probes.
#[test]
fn ladder_never_executes_a_candidate() {
    let env = FakeGitEnv::default()
        .with_registry(HKCU, "InstallPath", r"C:\Users\dev\AppData\Local\Programs\Git")
        .with_file(r"C:\Users\dev\AppData\Local\Programs\Git\cmd\git.exe");
    let _ = resolve_ladder_for(&env, TargetOs::Windows);
    for call in env.calls() {
        let kind = call.split(':').next().unwrap_or_default();
        assert!(
            matches!(kind, "var" | "path" | "file" | "reg"),
            "unexpected ladder interaction: {call}"
        );
    }
}

// 10. reg.exe output parser: real-format blocks, both string types, prefix
//     value names, empty/garbage input — correct Some/None, never a panic.
#[cfg(windows)]
#[test]
fn parse_reg_query_table() {
    let real = "\r\nHKEY_CURRENT_USER\\SOFTWARE\\GitForWindows\r\n    \
                InstallPath    REG_SZ    C:\\Users\\dev\\AppData\\Local\\Programs\\Git\r\n\r\n";
    assert_eq!(
        parse_reg_query(real, "InstallPath").as_deref(),
        Some(r"C:\Users\dev\AppData\Local\Programs\Git")
    );

    let expand = "    InstallPath    REG_EXPAND_SZ    C:\\Program Files\\Git\r\n";
    assert_eq!(
        parse_reg_query(expand, "InstallPath").as_deref(),
        Some(r"C:\Program Files\Git")
    );

    // A value name that is a PREFIX of the queried one must not cross-match.
    let prefixed = "    Install    REG_SZ    C:\\wrong\r\n    InstallPath    REG_SZ    C:\\right\r\n";
    assert_eq!(parse_reg_query(prefixed, "InstallPath").as_deref(), Some(r"C:\right"));
    assert_eq!(parse_reg_query(prefixed, "Install").as_deref(), Some(r"C:\wrong"));

    // Path data containing spaces is preserved verbatim.
    let spaced = "    InstallPath    REG_SZ    C:\\Program Files (x86)\\Git\r\n";
    assert_eq!(
        parse_reg_query(spaced, "InstallPath").as_deref(),
        Some(r"C:\Program Files (x86)\Git")
    );

    // Misses / malformed input -> None, no panic.
    assert_eq!(parse_reg_query("", "InstallPath"), None);
    assert_eq!(parse_reg_query("ERROR: The system was unable to find", "InstallPath"), None);
    assert_eq!(parse_reg_query("    InstallPath    REG_DWORD    0x1\r\n", "InstallPath"), None);
    assert_eq!(parse_reg_query("    InstallPath    REG_SZ    \r\n", "InstallPath"), None);
    assert_eq!(parse_reg_query("InstallPath", "InstallPath"), None);
    assert_eq!(parse_reg_query("random \u{fffd} garbage \0 output", "InstallPath"), None);
}

// 11. bin_dir(): Some for Registry / WellKnown / Override-with-parent; None for
//     Path / Fallback / a parentless Override.
#[test]
fn bin_dir_is_only_set_for_non_path_rungs() {
    let cases = [
        (GitBinSource::Registry, r"C:\Git\cmd\git.exe", true),
        (GitBinSource::WellKnown, "/usr/local/bin/git", true),
        (GitBinSource::Override, "/opt/git/bin/git", true),
        (GitBinSource::Override, "git", false),
        (GitBinSource::Path, "/usr/bin/git", false),
        (GitBinSource::Fallback, "git", false),
    ];
    for (source, path, expect_some) in cases {
        let bin = GitBin {
            path: PathBuf::from(path),
            source,
        };
        assert_eq!(
            bin.bin_dir().is_some(),
            expect_some,
            "bin_dir for {source:?} {path}"
        );
    }
}

// ---- cache + factory + diagnostics --------------------------------------------
