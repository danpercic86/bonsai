//! Optional AI helper (P24e §6.8): translate ONE existing instruction file into
//! another agent/tool's flavor via the local `claude` CLI. Reuses the shipped
//! `crate::ai::run_claude` text-transform driver. WRITES NOTHING — returns the
//! proposed text; the user reviews it and pastes it into a profile target.
//!
//! Pure `crate::ai` + string assembly; blocking. The command layer enforces the
//! consent gate and wraps this in `spawn_blocking`.

use std::path::Path;

use crate::ai::{self, RunOpts};
use crate::error::AppError;

/// System prompt (via `--append-system-prompt`): role + strict output contract.
/// Verbatim and deliberately collapsed to a SINGLE line — on Windows the `claude`
/// CLI is a `.cmd` shim and Rust's `Command` REFUSES an argv argument containing a
/// newline. Multi-line content (the source file) flows ONLY through the stdin
/// payload. (P24e)
const GENERATE_SYSTEM_PROMPT: &str = "You are rewriting an AI-agent instruction file for a different agent/tool. Preserve the guidance and meaning exactly; adapt only tone, headings, and format conventions to the target tool. Output ONLY the instruction file body — no preamble, no explanation, no code fences.";

/// The model's proposed translated instruction file. Serialized camelCase
/// (mirrored in the TS `AiGeneratedAsset`). NOT written anywhere. (P24e §6.8)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGeneratedAsset {
    pub target_agent: String,
    /// Proposed content (fence-stripped by `run_claude`). NOT written to disk.
    pub content: String,
}

/// Blocking. Translate `source_content` into `target_agent`'s instruction-file
/// flavor. Builds a single-line `-p` prompt (naming the target agent + optional
/// `guidance`), pipes the multi-line `source_content` via stdin, and calls
/// `ai::run_claude`. Returns the proposed text — WRITES NOTHING.
///
/// `cwd` is the child's working dir (the repo workdir). `opts` carry model +
/// timeout; the system prompt is set here.
pub fn generate_asset(
    workdir: &Path,
    source_content: &str,
    target_agent: &str,
    guidance: Option<&str>,
    opts: RunOpts,
) -> Result<AiGeneratedAsset, AppError> {
    // Single-line `-p` prompt (see the system-prompt note re: the `.cmd` shim
    // refusing newlines). The source file flows via stdin only.
    let mut prompt = format!(
        "Rewrite the AI-agent instruction file provided on standard input for the target tool/agent: {target_agent}. Keep the guidance identical; adapt tone and format conventions to that tool. Output only the instruction file body."
    );
    if let Some(g) = guidance {
        let g = g.trim();
        if !g.is_empty() {
            // Collapse any newlines in user guidance to spaces so the argv stays
            // single-line (the `.cmd` shim rejects embedded newlines).
            let g_single: String = g.split_whitespace().collect::<Vec<_>>().join(" ");
            prompt.push_str(" Additional guidance: ");
            prompt.push_str(&g_single);
        }
    }

    let result = ai::run_claude(
        workdir,
        &prompt,
        Some(source_content),
        RunOpts {
            system_prompt: Some(GENERATE_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    Ok(AiGeneratedAsset {
        target_agent: target_agent.to_string(),
        content: result.text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::{Mutex, MutexGuard};

    const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";
    const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";

    /// Serialize env-mutating tests (`BONSAI_CLAUDE_BIN` / `BONSAI_STUB_MODE` are
    /// process-global and the stub inherits them). Mirrors `ai::mod::tests`.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Windows runs the `.cmd` stub directly (`Command::new` routes `.cmd`
    /// through cmd.exe automatically). macOS/Linux use the POSIX `.sh` twin,
    /// with the executable bit forced on at test time — git doesn't reliably
    /// preserve the mode bit across clones/platforms. Mirrors `ai::mod::tests`.
    fn stub_path() -> std::path::PathBuf {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        if cfg!(windows) {
            fixtures.join("claude_stub.cmd")
        } else {
            let path = fixtures.join("claude_stub.sh");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&path) {
                    let mut perms = meta.permissions();
                    perms.set_mode(perms.mode() | 0o111);
                    let _ = std::fs::set_permissions(&path, perms);
                }
            }
            path
        }
    }

    fn set_mode(mode: &str) {
        std::env::set_var(CLAUDE_BIN_ENV, stub_path());
        std::env::set_var(STUB_MODE_ENV, mode);
    }

    /// The success stub returns `MERGED_BODY_OK`; `generate_asset` surfaces that as
    /// the proposed content and echoes the requested target agent. Proves the
    /// run_claude reuse + wiring (single-line argv, stdin payload).
    #[test]
    fn generate_asset_success_returns_stub_text() {
        let _g = env_lock();
        set_mode("success");
        let res = generate_asset(
            Path::new("."),
            "# CLAUDE.md\n\nBe concise.\n",
            "GitHub Copilot",
            Some("prefer bullet points"),
            RunOpts::default(),
        )
        .expect("success stub should yield Ok");
        assert_eq!(res.target_agent, "GitHub Copilot");
        assert_eq!(res.content, "MERGED_BODY_OK");
    }

    /// An error envelope from the CLI maps to `AiFailed` (the gate/CLI-missing
    /// path is enforced at the command layer, tested there / by the shared
    /// `ai::mod` gate pattern).
    #[test]
    fn generate_asset_cli_error_maps_to_ai_failed() {
        let _g = env_lock();
        set_mode("error");
        let err = generate_asset(
            Path::new("."),
            "source",
            "Gemini CLI",
            None,
            RunOpts::default(),
        )
        .expect_err("is_error envelope should map to Err");
        assert!(
            matches!(err, AppError::AiFailed(_)),
            "expected AiFailed, got {err:?}"
        );
    }

    /// Wire shape: camelCase `targetAgent` / `content` (mirrors the TS type).
    #[test]
    fn generated_asset_wire_shape_is_camel_case() {
        let v = serde_json::to_value(AiGeneratedAsset {
            target_agent: "Cursor".to_string(),
            content: "# Cursor rules".to_string(),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "targetAgent": "Cursor", "content": "# Cursor rules" })
        );
    }
}
