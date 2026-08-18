//! T2 Area 9 — external launcher argv assembly is INJECTION-SAFE.
//!
//! The whole external-tool surface (`open in terminal/file-manager/editor`)
//! builds a `LaunchSpec { program, args, cwd }` and spawns it WITHOUT a shell.
//! The safety property under test: a repo path containing shell metacharacters,
//! quotes, a newline, a leading dash, or unicode is substituted into a SINGLE
//! argv token — never split, never shell-interpreted. A `FakeRunner` captures
//! the spec so no real app is launched. `resolve_program` hit/miss is checked
//! directly (it resolves a path, it does not spawn).

mod common;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use bonsai_core::external::{
    editor_ladder, launch_first, parse_template, reveal_spec, terminal_ladder, CommandRunner,
    LaunchSpec, SpawnRunner, TargetOs,
};
use bonsai_core::procutil::resolve_program;

/// Records every spec it is asked to run and always "succeeds" (never spawns).
struct FakeRunner {
    seen: RefCell<Vec<LaunchSpec>>,
}
impl FakeRunner {
    fn new() -> FakeRunner {
        FakeRunner { seen: RefCell::new(Vec::new()) }
    }
}
impl CommandRunner for FakeRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String> {
        self.seen.borrow_mut().push(spec.clone());
        Ok(())
    }
}

/// Every hostile path substitutes into ONE argv token, verbatim — the program
/// stays `editor`, and the metacharacters never become extra args or shell ops.
#[test]
fn hostile_path_becomes_one_argv_token() {
    let hostile = [
        r#"C:\proj\a & b"#,
        r#"C:\proj\"quoted""#,
        r#"C:\proj\a^b%PATH%!x"#,
        r#"/home/me/a;rm -rf ~"#,
        r#"/home/me/$(reboot)"#,
        "/home/me/line\nbreak",
        "-rf --no-preserve-root",       // leading dash
        "/tmp/café/Ünïcode/日本語/Ж",   // unicode
    ];
    for raw in hostile {
        let path = PathBuf::from(raw);
        let spec = parse_template("editor {path}", &path, false)
            .unwrap_or_else(|| panic!("template must parse for {raw:?}"));
        assert_eq!(spec.program, "editor", "program never becomes the path");
        assert_eq!(spec.args.len(), 1, "path is exactly ONE arg for {raw:?}: {:?}", spec.args);
        assert_eq!(spec.args[0], path.display().to_string(), "arg is the path verbatim");
        assert_eq!(spec.cwd, path);
    }
}

/// An embedded `--flag={path}` keeps the path in the SAME token even when the
/// path holds spaces/metacharacters.
#[test]
fn embedded_path_flag_stays_one_token() {
    let path = PathBuf::from(r#"C:\a b & c\repo"#);
    let spec = parse_template("code --folder-uri={path} --new", &path, true).expect("parse");
    assert_eq!(spec.program, "code");
    assert_eq!(
        spec.args,
        vec![
            format!("--folder-uri={}", path.display()),
            "--new".to_string()
        ]
    );
    assert!(spec.hide_console, "hide_console threaded through");
}

/// An unbalanced quote in the template must not panic; it just yields a spec (or
/// None for an empty template) with the remainder as one token.
#[test]
fn unbalanced_quote_template_no_panic() {
    let path = PathBuf::from("/tmp/x");
    // Trailing open-quote: the unterminated run is still flushed as a token.
    let spec = parse_template(r#"editor "{path}"#, &path, false).expect("parse");
    assert_eq!(spec.program, "editor");
    assert_eq!(spec.args.len(), 1);
    assert!(spec.args[0].contains("/tmp/x"));
    // A whitespace-only template collapses to None (no program token).
    assert!(parse_template("   ", &path, false).is_none());
    // An empty-quoted template yields a single empty token (program == "") — not
    // None, but harmless: it fails to resolve at the spawn seam, never panics.
    let empty_quote = parse_template("\"\"", &path, false).expect("one empty token");
    assert_eq!(empty_quote.program, "");
}

/// `launch_first` hands the FIRST spec to the runner UNCHANGED — the hostile
/// path arrives at the (fake) spawn seam as one arg, proving no reassembly.
#[test]
fn launch_first_delivers_spec_unchanged() {
    let path = PathBuf::from(r#"/tmp/a b;c & d"#);
    let ladder = vec![parse_template("term {path}", &path, false).expect("parse")];
    let runner = FakeRunner::new();
    launch_first(&runner, &ladder, "terminal").expect("fake run ok");
    let seen = runner.seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].args, vec![path.display().to_string()]);
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
        assert!(miss.is_err(), "a nonsense program name must fail to resolve");
    } else {
        assert!(miss.is_ok(), "non-Windows defers not-found to spawn(), not resolve_program");
    }
}

fn is_nonempty(p: &Path) -> bool {
    !p.as_os_str().is_empty()
}

// ---------------------------------------------------------------- F-MAC-1
// The macOS editor ladder used to no-op silently: `/usr/bin/open` ALWAYS
// spawns successfully and reports "Unable to find application" through its
// EXIT CODE, so a spawn-only runner made rung #1 win forever and a Mac without
// VS Code got nothing. `LaunchSpec::wait_for_exit` marks exactly the macOS
// `open` rungs as "wait and judge the exit status"; everything else keeps the
// detached-spawn semantics (Windows `explorer` exits non-zero on success).

/// (a) Both macOS `open -a` editor rungs carry `wait_for_exit`; the `code` CLI
/// fallback does not.
#[test]
fn macos_editor_open_rungs_wait_for_exit() {
    let path = PathBuf::from("/tmp/work");
    let ladder = editor_ladder(TargetOs::MacOs, "", &path);
    assert_eq!(ladder.len(), 3, "open -a VS Code, open -a Insiders, code");
    assert_eq!(ladder[0].program, "open");
    assert!(ladder[0].wait_for_exit, "rung 1 must judge open's exit code");
    assert_eq!(ladder[1].program, "open");
    assert!(ladder[1].wait_for_exit, "rung 2 must judge open's exit code");
    assert_eq!(ladder[2].program, "code");
    assert!(!ladder[2].wait_for_exit, "the plain CLI rung stays detached");
}

/// (b) No Windows or Linux spec ever waits — editor, terminal, or reveal.
#[test]
fn windows_linux_specs_never_wait_for_exit() {
    let path = PathBuf::from("/tmp/work");
    for os in [TargetOs::Windows, TargetOs::Linux] {
        for spec in editor_ladder(os, "", &path)
            .into_iter()
            .chain(terminal_ladder(os, "", &path))
            .chain(std::iter::once(reveal_spec(os, &path)))
        {
            assert!(
                !spec.wait_for_exit,
                "{os:?} `{}` must stay a detached spawn",
                spec.program
            );
        }
    }
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
    assert!(msg.contains("status 1"), "error names the exit status: {msg}");

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
