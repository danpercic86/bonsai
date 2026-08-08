//! External-tool launch (P49): open a filesystem path in the OS **terminal**,
//! **file manager**, or **editor**.
//!
//! Everything here is a *self-contained* `std::process::Command` spawn — no
//! plugin, no `open` crate (P49 D1). Two halves keep it testable on one machine:
//!
//! * **Pure builders** ([`parse_template`], [`terminal_ladder`], [`reveal_spec`],
//!   [`editor_ladder`]) produce [`LaunchSpec`]s from an explicit [`TargetOs`]
//!   param — never `cfg!` — so every OS branch runs in unit tests regardless of
//!   the host. They touch no filesystem and never spawn.
//! * A [`CommandRunner`] ([`SpawnRunner`] in production) turns a `LaunchSpec`
//!   into a real, detached child. Tests inject a fake runner to assert the
//!   fallback ladder without launching anything.
//!
//! Safety (P49 D2): a launch is always `program + [args…] + explicit cwd`. The
//! user template is tokenized and `{path}` is substituted **inside a single argv
//! token**, so a path with spaces or a shell metacharacter (`;`, `&&`, `|`) can
//! never break out into a second command — nothing is ever handed to a shell.

use crate::error::AppError;
use std::path::{Path, PathBuf};

/// Which OS to build argv for. [`host`](TargetOs::host) picks the running target
/// in production; tests pass each variant explicitly so all branches execute on
/// a single machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Windows,
    MacOs,
    Linux,
}

impl TargetOs {
    /// The host OS. Anything that is not Windows or macOS is treated as Linux
    /// (the "generic X11/Wayland desktop" ladder).
    pub fn host() -> TargetOs {
        #[cfg(target_os = "windows")]
        {
            TargetOs::Windows
        }
        #[cfg(target_os = "macos")]
        {
            TargetOs::MacOs
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            TargetOs::Linux
        }
    }
}

/// A fully-resolved child launch. Pure output of the builders; a
/// [`CommandRunner`] turns it into a real spawn. NEVER a shell command line —
/// `program` + separate `args` + explicit `cwd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Windows only: suppress the transient console window a `.cmd` shim (VS
    /// Code's `code.cmd`) or `explorer` would flash. MUST be `false` for
    /// terminals — we WANT that window. Ignored on macOS/Linux.
    pub hide_console: bool,
}

/// Injected so argv-building + ladder logic are testable without launching apps.
/// `Ok(())` = spawned (we do NOT wait; the child outlives Bonsai). `Err(msg)` =
/// this candidate failed, so [`launch_first`] tries the next ladder entry (or
/// surfaces the error if it was the last).
pub trait CommandRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String>;
}

/// Production runner: builds a `std::process::Command`, sets program/args/cwd,
/// applies `CREATE_NO_WINDOW` iff `spec.hide_console` (Windows), then `spawn()`
/// without waiting — the child is detached.
pub struct SpawnRunner;

impl CommandRunner for SpawnRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String> {
        let program = resolve_program(&spec.program)?;
        let mut cmd = std::process::Command::new(program);
        cmd.args(&spec.args).current_dir(&spec.cwd);
        #[cfg(windows)]
        if spec.hide_console {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        // Spawn and drop the handle: we never wait — the launched app is
        // detached and outlives us. Only a spawn failure is a failure (an
        // `explorer` nonzero *exit* is irrelevant because we don't wait).
        cmd.spawn().map(|_child| ()).map_err(|e| e.to_string())
    }
}

/// PATHEXT-aware program resolution — promoted to [`crate::procutil`] so the
/// AI CLI driver shares it (audit §2.7); the semantics for the ladder are
/// unchanged (unresolvable name → `Err` → next ladder entry).
use crate::procutil::resolve_program;

// ---- pure builders (no fs, no spawn) ------------------------------------------

/// Small constructor keeping the ladder tables terse.
fn spec(program: &str, args: &[&str], path: &Path, hide_console: bool) -> LaunchSpec {
    LaunchSpec {
        program: program.to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
        cwd: path.to_path_buf(),
        hide_console,
    }
}

/// Split a template into argv tokens: whitespace-separated, honoring double
/// quotes (a quoted run keeps its spaces; the surrounding quotes are dropped).
/// The template is NEVER handed to a shell, so any `;`/`&&`/`|` becomes literal
/// token text.
fn tokenize(template: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut started = false;
    for c in template.chars() {
        if c == '"' {
            in_quote = !in_quote;
            started = true;
        } else if c.is_ascii_whitespace() && !in_quote {
            if started {
                tokens.push(std::mem::take(&mut cur));
                started = false;
            }
        } else {
            cur.push(c);
            started = true;
        }
    }
    if started {
        tokens.push(cur);
    }
    tokens
}

/// Tokenize `template`, substitute every literal `{path}` occurrence inside each
/// token with `path.display()` (so both a standalone `{path}` and an embedded
/// `--flag={path}` work), then take `token[0]` as `program` and the rest as
/// `args`, with `cwd = path`. `None` for an empty/whitespace-only template.
pub fn parse_template(template: &str, path: &Path, hide_console: bool) -> Option<LaunchSpec> {
    let tokens = tokenize(template);
    if tokens.is_empty() {
        return None;
    }
    let p = path.display().to_string();
    let mut it = tokens.into_iter().map(|t| t.replace("{path}", &p));
    let program = it.next()?;
    let args: Vec<String> = it.collect();
    Some(LaunchSpec {
        program,
        args,
        cwd: path.to_path_buf(),
        hide_console,
    })
}

/// Ordered terminal candidates. A non-empty template ⇒ exactly that one spec;
/// an empty template ⇒ the per-OS auto ladder. All `hide_console = false` — a
/// terminal window MUST be visible.
pub fn terminal_ladder(os: TargetOs, template: &str, path: &Path) -> Vec<LaunchSpec> {
    if let Some(parsed) = parse_template(template, path, false) {
        return vec![parsed];
    }
    let p = path.display().to_string();
    match os {
        TargetOs::Windows => vec![
            spec("wt", &["-d", &p], path, false),
            spec("powershell", &[], path, false),
            spec("cmd", &["/K"], path, false),
        ],
        TargetOs::MacOs => vec![spec("open", &["-a", "Terminal", &p], path, false)],
        TargetOs::Linux => vec![
            spec("gnome-terminal", &[&format!("--working-directory={p}")], path, false),
            spec("konsole", &["--workdir", &p], path, false),
            spec("x-terminal-emulator", &[], path, false),
        ],
    }
}

/// The single reveal-in-file-manager spec (not configurable). Opens the
/// directory itself in the OS file manager (`hide_console = true`).
pub fn reveal_spec(os: TargetOs, path: &Path) -> LaunchSpec {
    let p = path.display().to_string();
    match os {
        TargetOs::Windows => spec("explorer", &[&p], path, true),
        TargetOs::MacOs => spec("open", &[&p], path, true),
        TargetOs::Linux => spec("xdg-open", &[&p], path, true),
    }
}

/// Ordered editor candidates. A non-empty template ⇒ exactly that one spec; an
/// empty template ⇒ the per-OS VS Code auto ladder. All `hide_console = true`.
pub fn editor_ladder(os: TargetOs, template: &str, path: &Path) -> Vec<LaunchSpec> {
    if let Some(parsed) = parse_template(template, path, true) {
        return vec![parsed];
    }
    let p = path.display().to_string();
    match os {
        TargetOs::Windows | TargetOs::Linux => vec![
            spec("code", &[&p], path, true),
            spec("code-insiders", &[&p], path, true),
        ],
        TargetOs::MacOs => vec![
            spec("open", &["-a", "Visual Studio Code", &p], path, true),
            spec("open", &["-a", "Visual Studio Code - Insiders", &p], path, true),
            spec("code", &[&p], path, true),
        ],
    }
}

// ---- thin orchestration -------------------------------------------------------

/// Try each spec in order; the first `Ok` wins. If all fail, return
/// [`AppError::ExternalToolFailed`] naming the last candidate + its error.
/// `what` is a human label ("terminal" | "file manager" | "editor").
pub fn launch_first(
    runner: &dyn CommandRunner,
    ladder: &[LaunchSpec],
    what: &str,
) -> Result<(), AppError> {
    let mut last_err: Option<(String, String)> = None;
    for spec in ladder {
        match runner.run(spec) {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some((spec.program.clone(), e)),
        }
    }
    Err(AppError::ExternalToolFailed(match last_err {
        Some((prog, e)) => format!("could not launch {what} ({prog}): {e}"),
        None => format!("no {what} command is configured"),
    }))
}

/// Launch a terminal at `path` (empty `template` ⇒ per-OS auto-detect). The
/// caller guarantees `path` exists (the command layer does the fs precheck).
pub fn open_in_terminal(
    runner: &dyn CommandRunner,
    os: TargetOs,
    template: &str,
    path: &Path,
) -> Result<(), AppError> {
    launch_first(runner, &terminal_ladder(os, template, path), "terminal")
}

/// Reveal `path` (a directory) in the OS file manager.
pub fn reveal_in_file_manager(
    runner: &dyn CommandRunner,
    os: TargetOs,
    path: &Path,
) -> Result<(), AppError> {
    launch_first(runner, std::slice::from_ref(&reveal_spec(os, path)), "file manager")
}

/// Open `path` in the configured editor (empty `template` ⇒ VS Code auto-detect).
pub fn open_in_editor(
    runner: &dyn CommandRunner,
    os: TargetOs,
    template: &str,
    path: &Path,
) -> Result<(), AppError> {
    launch_first(runner, &editor_ladder(os, template, path), "editor")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn p() -> PathBuf {
        PathBuf::from("/tmp/work")
    }

    // ---- parse_template ----

    #[test]
    fn parse_template_standalone_path_token() {
        let s = parse_template("myterm {path}", &p(), false).expect("parsed");
        assert_eq!(s.program, "myterm");
        assert_eq!(s.args, vec!["/tmp/work".to_string()]);
        assert_eq!(s.cwd, p());
        assert!(!s.hide_console);
    }

    #[test]
    fn parse_template_embedded_path_in_flag() {
        let s = parse_template("gnome-terminal --working-directory={path}", &p(), false)
            .expect("parsed");
        assert_eq!(s.program, "gnome-terminal");
        assert_eq!(s.args, vec!["--working-directory=/tmp/work".to_string()]);
    }

    #[test]
    fn parse_template_quoted_token_keeps_spaces_and_strips_quotes() {
        // A quoted program name AND a quoted arg with spaces both survive as one
        // token each, with the surrounding quotes removed.
        let s = parse_template("\"my editor\" \"C:\\Program Files\\x\"", &p(), true)
            .expect("parsed");
        assert_eq!(s.program, "my editor");
        assert_eq!(s.args, vec!["C:\\Program Files\\x".to_string()]);
        assert!(s.hide_console);
    }

    #[test]
    fn parse_template_substitutes_path_with_spaces_into_one_arg() {
        let path = PathBuf::from("/tmp/my repo");
        let s = parse_template("code {path}", &path, true).expect("parsed");
        assert_eq!(s.program, "code");
        // Space preserved AND not split into two args.
        assert_eq!(s.args, vec!["/tmp/my repo".to_string()]);
    }

    #[test]
    fn parse_template_empty_or_whitespace_is_none() {
        assert!(parse_template("", &p(), false).is_none());
        assert!(parse_template("   \t  ", &p(), false).is_none());
    }

    #[test]
    fn parse_template_shell_metachars_stay_literal_args() {
        // A `;` (or `&&`, `|`) is NOT a command separator — it is one more argv
        // token, immune to Windows Terminal's `;` sub-command delimiter.
        let s = parse_template("wt {path} ; rm -rf /", &p(), false).expect("parsed");
        assert_eq!(s.program, "wt");
        assert_eq!(
            s.args,
            vec!["/tmp/work", ";", "rm", "-rf", "/"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    // ---- builder tables (per TargetOs, empty template = auto ladder) ----

    #[test]
    fn terminal_ladder_windows_auto() {
        assert_eq!(
            terminal_ladder(TargetOs::Windows, "", &p()),
            vec![
                spec("wt", &["-d", "/tmp/work"], &p(), false),
                spec("powershell", &[], &p(), false),
                spec("cmd", &["/K"], &p(), false),
            ]
        );
    }

    #[test]
    fn terminal_ladder_macos_auto() {
        assert_eq!(
            terminal_ladder(TargetOs::MacOs, "", &p()),
            vec![spec("open", &["-a", "Terminal", "/tmp/work"], &p(), false)]
        );
    }

    #[test]
    fn terminal_ladder_linux_auto() {
        assert_eq!(
            terminal_ladder(TargetOs::Linux, "", &p()),
            vec![
                spec("gnome-terminal", &["--working-directory=/tmp/work"], &p(), false),
                spec("konsole", &["--workdir", "/tmp/work"], &p(), false),
                spec("x-terminal-emulator", &[], &p(), false),
            ]
        );
    }

    #[test]
    fn terminal_ladder_template_overrides_to_single_spec() {
        // A user template yields exactly one candidate on every OS, hide_console
        // false (visible terminal).
        for os in [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux] {
            assert_eq!(
                terminal_ladder(os, "alacritty --working-directory {path}", &p()),
                vec![spec(
                    "alacritty",
                    &["--working-directory", "/tmp/work"],
                    &p(),
                    false
                )]
            );
        }
    }

    #[test]
    fn reveal_spec_per_os() {
        assert_eq!(
            reveal_spec(TargetOs::Windows, &p()),
            spec("explorer", &["/tmp/work"], &p(), true)
        );
        assert_eq!(
            reveal_spec(TargetOs::MacOs, &p()),
            spec("open", &["/tmp/work"], &p(), true)
        );
        assert_eq!(
            reveal_spec(TargetOs::Linux, &p()),
            spec("xdg-open", &["/tmp/work"], &p(), true)
        );
    }

    #[test]
    fn editor_ladder_windows_and_linux_auto() {
        let expected = vec![
            spec("code", &["/tmp/work"], &p(), true),
            spec("code-insiders", &["/tmp/work"], &p(), true),
        ];
        assert_eq!(editor_ladder(TargetOs::Windows, "", &p()), expected);
        assert_eq!(editor_ladder(TargetOs::Linux, "", &p()), expected);
    }

    #[test]
    fn editor_ladder_macos_auto() {
        assert_eq!(
            editor_ladder(TargetOs::MacOs, "", &p()),
            vec![
                spec("open", &["-a", "Visual Studio Code", "/tmp/work"], &p(), true),
                spec(
                    "open",
                    &["-a", "Visual Studio Code - Insiders", "/tmp/work"],
                    &p(),
                    true
                ),
                spec("code", &["/tmp/work"], &p(), true),
            ]
        );
    }

    #[test]
    fn editor_ladder_template_overrides_to_single_spec() {
        assert_eq!(
            editor_ladder(TargetOs::MacOs, "subl {path}", &p()),
            vec![spec("subl", &["/tmp/work"], &p(), true)]
        );
    }

    // ---- ladder fallback logic (FakeRunner — NEVER spawns) ----

    /// Records every `run` call and succeeds only for the programs in `succeed`.
    struct FakeRunner {
        succeed: Vec<String>,
        calls: RefCell<Vec<String>>,
    }

    impl FakeRunner {
        fn new(succeed: &[&str]) -> FakeRunner {
            FakeRunner {
                succeed: succeed.iter().map(|s| s.to_string()).collect(),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, spec: &LaunchSpec) -> Result<(), String> {
            self.calls.borrow_mut().push(spec.program.clone());
            if self.succeed.iter().any(|s| s == &spec.program) {
                Ok(())
            } else {
                Err(format!("mock: `{}` not found", spec.program))
            }
        }
    }

    #[test]
    fn first_candidate_fails_second_succeeds_picks_second() {
        // wt unresolvable ⇒ falls through to PowerShell, which succeeds; cmd is
        // never tried.
        let runner = FakeRunner::new(&["powershell"]);
        open_in_terminal(&runner, TargetOs::Windows, "", &p()).expect("second candidate wins");
        assert_eq!(*runner.calls.borrow(), vec!["wt", "powershell"]);
    }

    #[test]
    fn all_candidates_fail_errors_naming_last_program() {
        // wt → powershell → cmd all fail: ExternalToolFailed names the LAST
        // program (cmd) and the "terminal" label.
        let runner = FakeRunner::new(&[]);
        let err = open_in_terminal(&runner, TargetOs::Windows, "", &p())
            .expect_err("all candidates fail");
        assert!(matches!(err, AppError::ExternalToolFailed(_)));
        let msg = err.to_string();
        assert!(msg.contains("cmd"), "message names last program: {msg}");
        assert!(msg.contains("terminal"), "message carries the label: {msg}");
        assert_eq!(*runner.calls.borrow(), vec!["wt", "powershell", "cmd"]);
    }

    #[test]
    fn reveal_single_candidate_success_and_failure() {
        // Success: the one reveal spec runs.
        let ok = FakeRunner::new(&["explorer"]);
        reveal_in_file_manager(&ok, TargetOs::Windows, &p()).expect("reveal spawns");
        assert_eq!(*ok.calls.borrow(), vec!["explorer"]);

        // Failure: the single candidate fails ⇒ ExternalToolFailed naming it.
        let bad = FakeRunner::new(&[]);
        let err = reveal_in_file_manager(&bad, TargetOs::Windows, &p())
            .expect_err("reveal fails");
        assert!(matches!(err, AppError::ExternalToolFailed(_)));
        assert!(err.to_string().contains("explorer"));
    }

    #[test]
    fn editor_template_is_the_only_candidate_tried() {
        // A configured template short-circuits the auto ladder: only the
        // template program is attempted, and on failure it is what the error
        // names.
        let runner = FakeRunner::new(&[]);
        let err = open_in_editor(&runner, TargetOs::Windows, "my-editor {path}", &p())
            .expect_err("template program missing");
        assert!(matches!(err, AppError::ExternalToolFailed(_)));
        assert!(err.to_string().contains("my-editor"));
        assert_eq!(*runner.calls.borrow(), vec!["my-editor"]);
    }
}
