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
//!   into a real child — detached by default, or waited-on for the macOS
//!   `open` launchers (see [`LaunchSpec::wait_for_exit`]). Tests inject a fake
//!   runner to assert the fallback ladder without launching anything.
//!
//! Safety (P49 D2): a launch is always `program + [args…] + explicit cwd`. The
//! user template is tokenized and `{path}` is substituted **inside a single argv
//! token**, so a path with spaces or a shell metacharacter (`;`, `&&`, `|`) can
//! never break out into a second command — nothing is ever handed to a shell.
//!
//! ACCEPTED RESIDUAL RISKS (not exploitable from Bonsai's own inputs; the
//! template is USER-configured and `{path}` is a repo path the user already
//! opened — so this is self-inflicted at worst, never attacker-controlled):
//!  * **Windows `.cmd`/`.bat` shims** (e.g. VS Code's `code.cmd`): when the
//!    resolved program is a batch shim, Windows runs it via `cmd.exe`, which
//!    performs `%VAR%` environment-variable expansion on the argv it receives.
//!    A `{path}` (or template token) literally containing `%FOO%` would be
//!    expanded by that shim. We do NOT quote/escape `%` because there is no
//!    robust cross-shim escaping and the value is user-owned; the post-CVE
//!    (2024-24576) Rust argv-quoting still applies to the raw argument.
//!  * **Windows Terminal (`wt`) `;`**: `wt` treats `;` in ITS OWN argument
//!    parsing as a sub-command delimiter (independent of any shell). A template
//!    that puts a `;` in a `wt` argument can therefore start a second `wt`
//!    pane/tab. This is a `wt`-specific arg convention, not shell injection, and
//!    only reachable through the user's own terminal template — accepted.

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
    /// Wait for the child to EXIT and treat a non-zero status as a failure so
    /// the ladder advances (default `false` = detached spawn, exit ignored).
    ///
    /// Set ONLY for the macOS `/usr/bin/open` launchers. `open` hands the
    /// request to LaunchServices and returns immediately, so waiting costs
    /// milliseconds and does NOT block on the launched app's lifetime — but it
    /// is the only way to learn that `open -a "Visual Studio Code"` printed
    /// "Unable to find application", which it reports via its EXIT CODE, not by
    /// failing to spawn. Without this the first `open` rung always "succeeds"
    /// and the fallback ladder never runs (silent no-op, finding F-MAC-1).
    ///
    /// MUST stay `false` everywhere else: Windows `explorer` routinely exits
    /// non-zero after successfully handing off, and a real editor/terminal
    /// would keep us waiting for as long as the user keeps it open.
    pub wait_for_exit: bool,
}

/// Injected so argv-building + ladder logic are testable without launching apps.
/// `Ok(())` = launched — spawned and left detached, or (when
/// [`LaunchSpec::wait_for_exit`]) exited zero. `Err(msg)` = this candidate
/// failed, so [`launch_first`] tries the next ladder entry (or surfaces the
/// error if it was the last).
pub trait CommandRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String>;
}

/// Production runner: builds a `std::process::Command`, sets program/args/cwd,
/// applies `CREATE_NO_WINDOW` iff `spec.hide_console` (Windows), then either
/// `spawn()`s without waiting (default — the child is detached) or, iff
/// `spec.wait_for_exit`, `status()`s and reports a non-zero exit as an error.
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
        if spec.wait_for_exit {
            // macOS `open` only: it returns as soon as LaunchServices has taken
            // the request, so this does NOT wait on the launched app. Its exit
            // code is the ONLY signal that the app was not found, so a non-zero
            // status must be an `Err` for `launch_first` to try the next rung.
            let status = cmd.status().map_err(|e| e.to_string())?;
            return if status.success() {
                Ok(())
            } else {
                Err(match status.code() {
                    Some(code) => format!("exited with status {code}"),
                    None => "terminated by signal".to_string(),
                })
            };
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

/// Small constructor keeping the ladder tables terse. `wait_for_exit` is the
/// macOS-`open` flag documented on [`LaunchSpec::wait_for_exit`]; every other
/// entry passes `false`.
fn spec(
    program: &str,
    args: &[&str],
    path: &Path,
    hide_console: bool,
    wait_for_exit: bool,
) -> LaunchSpec {
    LaunchSpec {
        program: program.to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
        cwd: path.to_path_buf(),
        hide_console,
        wait_for_exit,
    }
}

/// A macOS `/usr/bin/open` ladder entry: same as [`spec`] but always
/// `wait_for_exit = true`, so `open`'s "Unable to find application" exit code
/// makes the ladder fall through instead of silently "succeeding".
fn open_spec(args: &[&str], path: &Path, hide_console: bool) -> LaunchSpec {
    spec("open", args, path, hide_console, true)
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
        // A user template is an arbitrary program (`subl`, `nvim`, a wrapper
        // script) that may run for the whole editing session — NEVER wait on
        // it, even if the user typed `open -a …`.
        wait_for_exit: false,
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
            spec("wt", &["-d", &p], path, false, false),
            spec("powershell", &[], path, false, false),
            spec("cmd", &["/K"], path, false, false),
        ],
        // Terminal.app ships with macOS so this rung effectively never fails,
        // but `open` still gets the wait flag: it is the uniform rule for every
        // `open` launcher, and it upgrades a hypothetical failure from a silent
        // no-op to a real error instead of leaving it invisible.
        TargetOs::MacOs => vec![open_spec(&["-a", "Terminal", &p], path, false)],
        TargetOs::Linux => vec![
            spec("gnome-terminal", &[&format!("--working-directory={p}")], path, false, false),
            spec("konsole", &["--workdir", &p], path, false, false),
            spec("x-terminal-emulator", &[], path, false, false),
        ],
    }
}

/// The single reveal-in-file-manager spec (not configurable). Opens the
/// directory itself in the OS file manager (`hide_console = true`).
pub fn reveal_spec(os: TargetOs, path: &Path) -> LaunchSpec {
    let p = path.display().to_string();
    match os {
        // Windows `explorer` MUST stay detached: it habitually exits non-zero
        // after a successful hand-off, so waiting on it would report a bogus
        // failure.
        TargetOs::Windows => spec("explorer", &[&p], path, true, false),
        TargetOs::MacOs => open_spec(&[&p], path, true),
        TargetOs::Linux => spec("xdg-open", &[&p], path, true, false),
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
            spec("code", &[&p], path, true, false),
            spec("code-insiders", &[&p], path, true, false),
        ],
        // The two `open -a` rungs MUST wait: `open` always spawns fine and
        // signals "Unable to find application" only through its exit code, so
        // without the flag rung #1 would always win and a Mac without VS Code
        // would get a silent no-op instead of falling through to `code`.
        TargetOs::MacOs => vec![
            open_spec(&["-a", "Visual Studio Code", &p], path, true),
            open_spec(&["-a", "Visual Studio Code - Insiders", &p], path, true),
            spec("code", &[&p], path, true, false),
        ],
    }
}

/// Accept ONLY a plain web URL (P72), so a launcher can never be handed a
/// protocol the OS would resolve to something else. Pure: no fs, no spawn.
///
/// Accepts: an `http://` or `https://` scheme, matched CASE-INSENSITIVELY, with
/// a non-empty host drawn only from `[A-Za-z0-9.:_%-]` plus `[`/`]` for IPv6.
///
/// Three of the rules below came from the P72 security audit. None was
/// exploitable as written, but each is one line and each closes a real class:
///  * **No userinfo** (LOW-3, the sharpest). `@` in the host is rejected, because
///    `https://github.com@evil.example/` passes every other check while the
///    browser navigates to `evil.example`. On the `PrDetailView` path the URL
///    comes from a forge API response, so a hostile or compromised forge could
///    make "Open in browser" open an attacker page under a trustworthy-looking
///    label. Phishing, not code execution — but this is exactly the surface where
///    destination honesty IS the security property.
///  * **No whitespace or control characters ANYWHERE** (LOW-2), not only in the
///    host. A raw newline or tab in the path is inert on Windows/macOS, but
///    `xdg-open` is a shell script whose `$BROWSER`-with-`%s` branch word-splits
///    unquoted, turning a space into extra argv tokens for the browser.
///  * **A 2048-byte cap** (LOW-2), so an over-long forge string fails here with a
///    clean category error instead of an OS "filename or extension is too long"
///    at spawn time.
///
/// The host rule is an ALLOW-list, not a deny-list of the characters someone has
/// thought of so far: a deny-list on a security boundary needs re-auditing every
/// time a new byte is considered.
/// Rejects: every other scheme (`file:`, `javascript:`, `data:`, `ms-msdt:`,
/// `vscode:`), a UNC `\\server\share` path, a bare host with no scheme, a scheme
/// with no host (`https://`, `http:///x`), an empty/whitespace-only string, a
/// host containing a space or a `\`, and any input whose first character is `-`
/// (so the URL can never be parsed as a FLAG by the launcher program).
///
/// Load-bearing, not decorative: `PrDetailView`'s URL comes from a forge API
/// response, i.e. from outside the app. No URL crate is added — this is a
/// deliberate allow-list on a string, matching the crate's
/// hand-rolled-over-dependency house style (base64, percent-encoding).
///
/// SECURITY: the error message is CATEGORY-ONLY and never echoes `url`. A
/// forge-supplied URL can be arbitrarily long and can carry markup or lookalike
/// text; rendering it in a toast would turn a rejected link into a UI-spoofing
/// surface. (A launch *failure* from [`launch_first`] keeps its existing wording
/// and names only the program — never the URL.)
pub fn validate_web_url(url: &str) -> Result<(), AppError> {
    // Generous for any real PR/settings URL; see the audit LOW-2 note above.
    const MAX_LEN: usize = 2048;

    if url.is_empty() || url.starts_with('-') || url.len() > MAX_LEN {
        return Err(AppError::ExternalToolFailed(
            "refused to open a malformed link".to_string(),
        ));
    }
    // The whitespace/control screen covers the WHOLE url, not just the host:
    // `xdg-open` is a shell script whose $BROWSER-with-%s branch word-splits
    // unquoted, so a space in the PATH becomes extra argv tokens for the
    // browser (audit LOW-2).
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AppError::ExternalToolFailed(
            "refused to open a malformed link".to_string(),
        ));
    }
    // ASCII-only lowering, so byte offsets below stay valid for the original.
    let lower = url.to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &url["https://".len()..]
    } else if lower.starts_with("http://") {
        &url["http://".len()..]
    } else {
        return Err(AppError::ExternalToolFailed(
            "refused to open a link that is not http or https".to_string(),
        ));
    };
    let host = match rest.find(['/', '?', '#']) {
        Some(end) => &rest[..end],
        None => rest,
    };
    // Allow-list, NOT a deny-list (see the doc comment). Excluding `@` is what
    // rejects `https://github.com@evil.example/` — the userinfo impersonation of
    // audit LOW-3; a backslash is excluded by the same rule.
    let host_ok = !host.is_empty()
        && host.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '%' | '[' | ']')
        });
    if !host_ok {
        return Err(AppError::ExternalToolFailed(
            "refused to open a link with no host".to_string(),
        ));
    }
    Ok(())
}

/// Ordered browser-launch candidates for `url` (P72). Pure — takes an explicit
/// [`TargetOs`], never `cfg!`, so every branch runs in unit tests on one host.
/// The caller MUST have validated `url` first ([`validate_web_url`]).
///
/// `cwd` is `"."` for every entry: no repo path is involved, and the app
/// process's own directory is always a valid one — this keeps [`LaunchSpec`]
/// non-optional and the P49 ladder equality tests untouched.
///
/// `cmd /c start` is explicitly NOT a rung: `start` is a `cmd.exe` builtin, so
/// using it means handing a string to a shell (the exact thing P49 D2 forbids),
/// `cmd` would apply its own parsing to `&`, `^` and `%VAR%`, and its `start`
/// builtin treats the first quoted token as a window *title*. `explorer` and
/// `rundll32` each take the URL as a single argv token with no shell involved.
///
/// **Deliberately NOT `pub`** (audit LOW-1). The `rundll32
/// url.dll,FileProtocolHandler` rung is a general ShellExecute dispatcher: handed
/// a `.exe`, a `.hta`, a UNC path or an `ms-msdt:` string it would launch it. The
/// only thing between that and arbitrary execution is that the caller validated
/// first — so the ladder is not exported, leaving [`open_url`] (which validates
/// unconditionally) as the sole way in. A doc comment is not a sufficient guard
/// for a primitive of that shape.
fn url_ladder(os: TargetOs, url: &str) -> Vec<LaunchSpec> {
    let cwd = PathBuf::from(".");
    match os {
        // Both Windows rungs stay detached (`wait_for_exit: false`) for the same
        // reason as `reveal_spec`: `explorer` habitually exits non-zero AFTER a
        // successful hand-off, so waiting would report a bogus failure and
        // pointlessly advance the ladder.
        TargetOs::Windows => vec![
            spec("explorer", &[url], &cwd, true, false),
            spec("rundll32", &["url.dll,FileProtocolHandler", url], &cwd, true, false),
        ],
        // `open` always spawns fine and reports a failure only through its exit
        // code, so it gets the documented `wait_for_exit` treatment.
        TargetOs::MacOs => vec![open_spec(&[url], &cwd, false)],
        TargetOs::Linux => vec![spec("xdg-open", &[url], &cwd, true, false)],
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

/// Validate `url`, then open it in the user's default browser via the first
/// candidate that launches (P72). Validation runs BEFORE any spawn, so a
/// rejected URL never reaches a process. `what` is `"browser"`, so a total
/// failure reads `could not launch browser (rundll32): …`.
pub fn open_url(runner: &dyn CommandRunner, os: TargetOs, url: &str) -> Result<(), AppError> {
    validate_web_url(url)?;
    launch_first(runner, &url_ladder(os, url), "browser")
}

#[cfg(test)]
#[path = "external_tests.rs"]
mod tests;
