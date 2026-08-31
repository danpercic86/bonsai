//! Public data types for the AI CLI driver, split out of `mod.rs` so the
//! process-driving logic and these plain structs/enums read separately. They
//! are re-exported from `crate::ai` (`pub use types::*`), so the public paths
//! (`crate::ai::RunOpts`, etc.) are unchanged.

use std::time::Duration;

use super::{DEFAULT_IDLE_TIMEOUT, DEFAULT_MAX_TURNS, DEFAULT_TIMEOUT};

/// Which CLI tools a streaming run may use (P68 §A/D10). `ReadOnly` is the
/// conflict default — the model must be able to look at the rest of the repo, but
/// NEVER write, edit or run a shell. `None` reproduces today's `--tools ""`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy {
    ReadOnly,
    None,
}

impl ToolPolicy {
    /// The exact `--tools` argument value (verified allowlist, spike §1.6).
    pub fn arg(self) -> &'static str {
        match self {
            ToolPolicy::ReadOnly => "Read,Grep,Glob",
            ToolPolicy::None => "",
        }
    }
}

/// Limits for ONE streaming run. Deliberately a separate parameter rather than a
/// `RunOpts` field (D6/A2) so the 13 `RunOpts::default()` sites are untouched.
#[derive(Debug, Clone)]
pub struct RunLimits {
    /// Kill after this long with no child output. `Duration::ZERO` = disabled.
    /// PAUSED while awaiting user input (D3).
    pub idle_timeout: Duration,
    /// Optional absolute cap. `None` = unbounded (the user's locked default).
    /// Also paused while awaiting user input (D3).
    pub hard_cap: Option<Duration>,
    /// Max `result` lines (turns) per run before a still-questioning model is
    /// failed. >= 1.
    pub max_turns: u32,
    /// Tool allowlist (D10).
    pub tools: ToolPolicy,
    /// `--max-budget-usd` when `Some`; omitted when `None`.
    pub max_budget_usd: Option<f64>,
    /// `--include-partial-messages`. Default false; unknown delta shapes degrade
    /// to `log` (spike §1.8).
    pub include_partial_messages: bool,
    /// Feed the first turn as a stream-json user message on an OPEN stdin so a
    /// second turn is possible (the interactive mechanism, spike §1.4). false =
    /// one-shot: positional prompt + payload on stdin, then EOF.
    pub interactive: bool,
}

impl Default for RunLimits {
    fn default() -> Self {
        RunLimits {
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            hard_cap: None,
            max_turns: DEFAULT_MAX_TURNS,
            tools: ToolPolicy::ReadOnly,
            max_budget_usd: None,
            include_partial_messages: false,
            interactive: true,
        }
    }
}

/// Knobs for one `run_claude` call. `Default` = subscription resolver defaults. (P13)
#[derive(Debug, Clone)]
pub struct RunOpts {
    /// `--model <alias>`; `None` => `DEFAULT_MODEL`. Aliases: sonnet|haiku|opus.
    pub model: Option<String>,
    /// Killed and mapped to `AiFailed("timed out …")` past this deadline.
    pub timeout: Duration,
    /// Appended via `--append-system-prompt`. Sets role + output contract.
    pub system_prompt: Option<String>,
    /// Reserved: `--json-schema <schema>` for structured output. `None` in v1
    /// (§9.1 locks reading `result` prose instead). Wired but unused so a later
    /// feature can opt in without changing the signature.
    pub json_schema: Option<String>,
}

impl Default for RunOpts {
    fn default() -> Self {
        RunOpts { model: None, timeout: DEFAULT_TIMEOUT, system_prompt: None, json_schema: None }
    }
}

/// A successful CLI text transform. `text` is the model's `result` field with a
/// single leading/trailing ``` fence stripped defensively (§3.3). (P13)
#[derive(Debug, Clone)]
pub struct AiResult {
    pub text: String,
    pub cost_usd: Option<f64>,
    pub session_id: Option<String>,
}

/// Cheap health status. NEVER errors — a missing/broken CLI yields
/// `{ installed:false, .. }`, not an `Err`. Wire type mirrored in TS (§7). (P13)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAvailability {
    /// `claude --version` spawned and exited 0.
    pub installed: bool,
    /// v1: reported EQUAL to `installed` (subscription auth is NOT verified in a
    /// cheap probe — a real auth check would cost a billable call). Actual
    /// logged-out state surfaces as `AiFailed` on the first resolve (§9 note).
    pub logged_in: bool,
    /// Parsed from `--version` stdout when installed, else `None`.
    pub version: Option<String>,
    /// Human one-liner for the settings UI ("Claude Code 2.1.220 ready" /
    /// "Claude Code CLI not found on PATH").
    pub detail: String,
}
