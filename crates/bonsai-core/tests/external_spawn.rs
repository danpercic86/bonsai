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

use bonsai_core::external::{launch_first, parse_template, CommandRunner, LaunchSpec};
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
