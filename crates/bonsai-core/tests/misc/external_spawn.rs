//! T2 Area 9 (public-surface half) — the external launcher **hands a spec to the
//! runner unchanged**, and program resolution never consults the current
//! directory.
//!
//! The whole external-tool surface (`open in terminal/file-manager/editor`)
//! builds a `LaunchSpec { program, args, cwd }` and spawns it WITHOUT a shell. A
//! `FakeRunner` captures the spec so no real app is launched; `resolve_program`
//! hit/miss is checked directly (it resolves a path, it does not spawn).
//!
//! Updated 2026-09-14 (P112 §4/§7): there is no user-supplied program STRING
//! left at all — the launchers take `Option<&PickedTool>`, whose program is a
//! catalog literal, a probe-derived absolute path, or the one path a native
//! dialog the backend opened returned. The shape validator (`external_cmd`) and
//! its module are deleted; `safe_cwd` moved to `procutil`. The argv-assembly and
//! ladder-table cases live in `src/external_spawn_tests.rs` / `external_tests.rs`
//! / `external_picked_tests.rs`. What remains here is exactly what a caller
//! outside the crate can reach.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use bonsai_core::external::{launch_first, CommandRunner, LaunchSpec, SpawnRunner};
use bonsai_core::procutil::{resolve_program, safe_cwd};

/// Records every spec it is asked to run and always "succeeds" (never spawns).
struct FakeRunner {
    seen: RefCell<Vec<LaunchSpec>>,
}
impl FakeRunner {
    fn new() -> FakeRunner {
        FakeRunner {
            seen: RefCell::new(Vec::new()),
        }
    }
}
impl CommandRunner for FakeRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String> {
        self.seen.borrow_mut().push(spec.clone());
        Ok(())
    }
}

/// `launch_first` hands the FIRST spec to the runner UNCHANGED — the hostile
/// path arrives at the (fake) spawn seam as one arg, proving no reassembly.
///
/// The spec is built as a literal rather than through `program_spec` (now
/// `pub(crate)`): what this case is about is the RUNNER seam, and a literal
/// states the input without depending on a builder at all.
#[test]
fn launch_first_delivers_spec_unchanged() {
    let path = PathBuf::from(r#"/tmp/a b;c & d"#);
    let ladder = vec![LaunchSpec {
        program: "term".to_string(),
        args: vec![path.display().to_string()],
        cwd: safe_cwd(),
        hide_console: false,
        wait_for_exit: false,
    }];
    let runner = FakeRunner::new();
    launch_first(&runner, &ladder, "terminal").expect("fake run ok");
    let seen = runner.seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].args, vec![path.display().to_string()]);
    assert_eq!(
        seen[0].cwd,
        safe_cwd(),
        "LOW-1: the neutral cwd survives the seam"
    );
}

/// `resolve_program` resolves an existing program to a path (hit) — no spawn
/// either way. The "miss" half is platform-specific BY DESIGN (procutil.rs):
/// Windows resolves eagerly against `PATH`/`PATHEXT`, so a nonsense name
/// errors here; non-Windows hands the bare name to `Command` unchanged and
/// defers "not found" to `spawn()`'s `NotFound`, so `resolve_program` itself
/// always succeeds there.
#[test]
fn resolve_program_hit_and_miss() {
    let known: &str = if cfg!(windows) { "cmd" } else { "sh" };
    match resolve_program(known) {
        Ok(p) => assert!(is_nonempty(&p), "{known} resolved to a path"),
        Err(e) => eprintln!("note: {known} not resolvable in this env: {e}"),
    }
    let miss = resolve_program("bonsai-definitely-not-a-real-tool-xyz123");
    if cfg!(windows) {
        assert!(
            miss.is_err(),
            "a nonsense program name must fail to resolve"
        );
    } else {
        assert!(
            miss.is_ok(),
            "non-Windows defers not-found to spawn(), not resolve_program"
        );
    }
}

fn is_nonempty(p: &Path) -> bool {
    !p.as_os_str().is_empty()
}

/// A trivial child that exits with `code`, using the host's own shell so the
/// test runs on Windows and POSIX alike. `wait` selects the two runner modes.
fn exit_spec(code: i32, wait: bool) -> LaunchSpec {
    let (program, args) = if cfg!(windows) {
        ("cmd", vec!["/C".to_string(), format!("exit {code}")])
    } else {
        ("sh", vec!["-c".to_string(), format!("exit {code}")])
    };
    LaunchSpec {
        program: program.to_string(),
        args,
        cwd: std::env::current_dir().expect("cwd"),
        hide_console: true, // no console flash during the test run
        wait_for_exit: wait,
    }
}

/// (c) With `wait_for_exit`, a NON-ZERO rung is a failure, so the ladder
/// advances to the next candidate — the bug that made the macOS fallback
/// unreachable. Proven by the pair: `[exit 1]` alone errors (naming the exit
/// status), while `[exit 1, exit 0]` succeeds, which is only possible if the
/// second rung ran.
#[test]
fn nonzero_exit_rung_falls_through_to_next_rung() {
    let runner = SpawnRunner;

    let only_failing = vec![exit_spec(1, true)];
    let err = launch_first(&runner, &only_failing, "editor")
        .expect_err("a non-zero exit must fail when we wait for it");
    let msg = err.to_string();
    assert!(
        msg.contains("status 1"),
        "error names the exit status: {msg}"
    );

    let with_fallback = vec![exit_spec(1, true), exit_spec(0, true)];
    launch_first(&runner, &with_fallback, "editor")
        .expect("the zero-exit rung after the failing one wins");
}

/// The detached path is UNCHANGED: without `wait_for_exit` the exit status is
/// never observed, so a non-zero-exiting child still counts as launched (this
/// is what keeps Windows `explorer` working).
#[test]
fn detached_spawn_ignores_nonzero_exit() {
    let runner = SpawnRunner;
    launch_first(&runner, &[exit_spec(1, false)], "file manager")
        .expect("a detached spawn succeeds regardless of the child's exit code");
}
