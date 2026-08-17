//! The streaming argv (LOCKED, P68 §3.4) — split out of [`super::session`] so the
//! flag set is asserted by a pure test instead of only at a native checkpoint:
//! dropping `--verbose` (the CLI hard-errors without it), altering `--tools`, or
//! leaking repo content into argv would otherwise be invisible to the suite.

use std::path::Path;
use std::process::{Command, Stdio};

use super::{resolve_bin, RunLimits, RunOpts, DEFAULT_MODEL};

/// Build the `claude` command for one streaming run.
///
/// BatBadBut-class caveat, same as `run_claude`: on Windows `bin` resolves to the
/// npm `claude.cmd` shim and argv text reaching a `.cmd` is re-expanded by
/// cmd.exe, and Rust refuses to pass an argument containing a newline to a batch
/// file. INVARIANT (keep truthful, asserted by `argv_never_contains_a_newline`):
/// every element below is a Bonsai-controlled constant, a vetted model alias, a
/// SINGLE-LINE system prompt, or a decimal number — repo content, payloads and
/// user replies flow exclusively through stdin (D13), which is also why this
/// function takes NO payload/reply parameter.
pub(super) fn build_command(
    cwd: &Path,
    prompt: &str,
    opts: &RunOpts,
    limits: &RunLimits,
) -> Command {
    let bin = resolve_bin();
    let model = opts.model.clone().unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let mut cmd = Command::new(&bin);
    cmd.current_dir(cwd);
    if limits.interactive {
        cmd.arg("-p");
    } else {
        cmd.arg("-p").arg(prompt);
    }
    // `--verbose` is REQUIRED by stream-json (spike §1.1) — the CLI hard-errors
    // without it.
    cmd.arg("--verbose").arg("--output-format").arg("stream-json");
    if limits.interactive {
        // stdin stays open, so a second turn is possible (spike §1.4).
        cmd.arg("--input-format").arg("stream-json").arg("--replay-user-messages");
    }
    if limits.include_partial_messages {
        cmd.arg("--include-partial-messages");
    }
    cmd.arg("--safe-mode")
        .arg("--tools")
        .arg(limits.tools.arg())
        .arg("--no-session-persistence")
        .arg("--model")
        .arg(&model);
    if let Some(sp) = &opts.system_prompt {
        // The SINGLE-LINE half of the invariant above, enforced at the point of use:
        // a multi-line element makes Rust refuse the spawn outright on Windows
        // (`.cmd` shim, see `git::ai_resolve`'s prompt constants). Debug-only — a
        // release build must degrade, not abort, and the spawn error is honest.
        debug_assert!(
            !sp.contains('\n') && !sp.contains('\r'),
            "system prompt must be single-line (Windows .cmd argv rule): {sp:?}"
        );
        cmd.arg("--append-system-prompt").arg(sp);
    }
    if let Some(budget) = limits.max_budget_usd {
        cmd.arg("--max-budget-usd").arg(format!("{budget:.4}"));
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ToolPolicy;

    const PROMPT: &str = "resolve this conflict";

    /// The argv as strings. `get_args()` excludes the program itself, so these
    /// assertions are independent of `resolve_bin()`/PATH.
    fn argv(prompt: &str, opts: &RunOpts, limits: &RunLimits) -> Vec<String> {
        build_command(Path::new("."), prompt, opts, limits)
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn interactive() -> RunLimits {
        RunLimits { interactive: true, ..RunLimits::default() }
    }

    fn one_shot() -> RunLimits {
        RunLimits { interactive: false, ..RunLimits::default() }
    }

    /// Value that follows `flag`, if the flag is present at all.
    fn value_after(args: &[String], flag: &str) -> Option<String> {
        let i = args.iter().position(|a| a == flag)?;
        args.get(i + 1).cloned()
    }

    #[test]
    fn argv_always_passes_verbose_and_the_locked_flag_set() {
        let args = argv(PROMPT, &RunOpts::default(), &interactive());
        // stream-json is unusable without it (spike §1.1).
        assert!(args.iter().any(|a| a == "--verbose"), "argv: {args:?}");
        assert_eq!(value_after(&args, "--output-format").as_deref(), Some("stream-json"));
        assert!(args.iter().any(|a| a == "--safe-mode"), "argv: {args:?}");
        assert!(args.iter().any(|a| a == "--no-session-persistence"), "argv: {args:?}");
        // RunOpts::default() => DEFAULT_MODEL.
        assert_eq!(value_after(&args, "--model").as_deref(), Some("sonnet"));
    }

    #[test]
    fn argv_interactive_uses_a_bare_p_and_the_stream_json_input_flags() {
        let args = argv(PROMPT, &RunOpts::default(), &interactive());
        // D13: the prompt travels on stdin, so `-p` takes NO value here.
        assert_eq!(args.first().map(String::as_str), Some("-p"));
        assert_eq!(args.get(1).map(String::as_str), Some("--verbose"));
        assert!(!args.iter().any(|a| a == PROMPT), "prompt leaked into argv: {args:?}");
        assert_eq!(value_after(&args, "--input-format").as_deref(), Some("stream-json"));
        assert!(args.iter().any(|a| a == "--replay-user-messages"), "argv: {args:?}");
    }

    #[test]
    fn argv_one_shot_passes_the_prompt_positionally_and_omits_the_input_flags() {
        let args = argv(PROMPT, &RunOpts::default(), &one_shot());
        assert_eq!(args.first().map(String::as_str), Some("-p"));
        assert_eq!(args.get(1).map(String::as_str), Some(PROMPT));
        // Nothing can be sent to a closed stdin, so these must NOT appear.
        assert!(!args.iter().any(|a| a == "--input-format"), "argv: {args:?}");
        assert!(!args.iter().any(|a| a == "--replay-user-messages"), "argv: {args:?}");
    }

    #[test]
    fn argv_tools_matches_the_policy_verbatim() {
        let read_only =
            argv(PROMPT, &RunOpts::default(), &RunLimits { tools: ToolPolicy::ReadOnly, ..interactive() });
        assert_eq!(value_after(&read_only, "--tools").as_deref(), Some("Read,Grep,Glob"));
        let none =
            argv(PROMPT, &RunOpts::default(), &RunLimits { tools: ToolPolicy::None, ..interactive() });
        // Present, with an EMPTY value — that is how the CLI is told "no tools".
        assert_eq!(value_after(&none, "--tools").as_deref(), Some(""));
    }

    #[test]
    fn argv_budget_is_four_decimals_and_absent_when_unset() {
        let unset = argv(PROMPT, &RunOpts::default(), &interactive());
        assert!(!unset.iter().any(|a| a == "--max-budget-usd"), "argv: {unset:?}");
        let set = argv(
            PROMPT,
            &RunOpts::default(),
            &RunLimits { max_budget_usd: Some(0.5), ..interactive() },
        );
        assert_eq!(value_after(&set, "--max-budget-usd").as_deref(), Some("0.5000"));
        let tiny = argv(
            PROMPT,
            &RunOpts::default(),
            &RunLimits { max_budget_usd: Some(0.00004), ..interactive() },
        );
        assert_eq!(value_after(&tiny, "--max-budget-usd").as_deref(), Some("0.0000"));
    }

    #[test]
    fn argv_partial_messages_flag_is_opt_in() {
        let off = argv(PROMPT, &RunOpts::default(), &interactive());
        assert!(!off.iter().any(|a| a == "--include-partial-messages"), "argv: {off:?}");
        let on = argv(
            PROMPT,
            &RunOpts::default(),
            &RunLimits { include_partial_messages: true, ..interactive() },
        );
        assert!(on.iter().any(|a| a == "--include-partial-messages"), "argv: {on:?}");
    }

    #[test]
    fn argv_carries_the_system_prompt_only_when_set() {
        let none = argv(PROMPT, &RunOpts::default(), &interactive());
        assert!(!none.iter().any(|a| a == "--append-system-prompt"), "argv: {none:?}");
        let opts = RunOpts { system_prompt: Some("be terse".to_string()), ..RunOpts::default() };
        let some = argv(PROMPT, &opts, &interactive());
        assert_eq!(value_after(&some, "--append-system-prompt").as_deref(), Some("be terse"));
    }

    /// The `.cmd`-shim invariant: Rust REFUSES to pass an argument containing a
    /// newline to a batch file, so a multi-line element would break every Windows
    /// npm install. Repo content and replies are stdin-only (D13), so with
    /// Bonsai's single-line prompts this must hold for every element.
    #[test]
    fn argv_never_contains_a_newline() {
        let opts = RunOpts {
            system_prompt: Some("single line system prompt".to_string()),
            model: Some("sonnet".to_string()),
            ..RunOpts::default()
        };
        for limits in [
            RunLimits { max_budget_usd: Some(1.25), include_partial_messages: true, ..interactive() },
            RunLimits { tools: ToolPolicy::None, ..one_shot() },
        ] {
            for a in argv(PROMPT, &opts, &limits) {
                assert!(!a.contains('\n') && !a.contains('\r'), "multi-line argv element: {a:?}");
            }
        }
    }
}
