//! `bonsai-mcp` — a standalone stdio MCP server exposing Bonsai's differentiated
//! Git surface (precomputed commit graph, structured diffs, the conflict trio,
//! stashes) to AI assistants. Register with:
//!
//! ```text
//! claude mcp add bonsai -- <abs path>/bonsai-mcp.exe [--repo <repo>] [--allow-write]
//! ```
//!
//! Startup opens+validates the repo path once (non-bare git repo required) and
//! then serves JSON-RPC over stdio. `--repo` is optional: when omitted the
//! server uses the current working directory (so a `.mcp.json` entry at the repo
//! root needs no path). Mutation tools (P14c) are gated behind `--allow-write`;
//! P14b registers only the read set.

use std::path::PathBuf;
use std::process::ExitCode;

use rmcp::ServiceExt;

use bonsai_mcp::server::BonsaiServer;

/// The single usage string, reused by the help path and every arg error.
const USAGE: &str = "usage: bonsai-mcp [--repo <path>] [--allow-write]";

/// Parsed startup configuration.
#[derive(Debug)]
struct ServerConfig {
    repo: PathBuf,
    allow_write: bool,
}

/// Outcome of parsing argv: a ready-to-run config, or an explicit help request
/// (which is NOT an error — it prints usage to stdout and exits successfully).
#[derive(Debug)]
enum ParseOutcome {
    Config(ServerConfig),
    Help,
}

/// Parse `--repo <path>` (optional) and `--allow-write` (flag) from argv.
///
/// Hand-rolled to avoid a heavy CLI dependency. When `--repo` is omitted the
/// current working directory is used (convenient for a `.mcp.json` entry living
/// at the repo root, which the client launches with cwd = repo root).
///
/// `-h`/`--help` returns [`ParseOutcome::Help`] (usage → stdout, success exit);
/// any malformed argument returns an `Err(usage)` (→ stderr, failure exit).
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<ParseOutcome, String> {
    let mut repo: Option<PathBuf> = None;
    let mut allow_write = false;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--repo" => {
                let value = it
                    .next()
                    .ok_or_else(|| format!("--repo requires a path argument\n{USAGE}"))?;
                repo = Some(PathBuf::from(value));
            }
            "--allow-write" => allow_write = true,
            "-h" | "--help" => return Ok(ParseOutcome::Help),
            other => {
                return Err(format!("unexpected argument: {other}\n{USAGE}"));
            }
        }
    }

    let repo = match repo {
        Some(path) => path,
        None => std::env::current_dir().map_err(|e| {
            format!("--repo not given and the current directory is unavailable: {e}")
        })?,
    };

    Ok(ParseOutcome::Config(ServerConfig { repo, allow_write }))
}

/// Validate the `--repo` path as a non-bare git repository and return its
/// canonical workdir path (from `read_repo_info`).
fn validate_repo(path: &std::path::Path) -> Result<PathBuf, String> {
    let info = bonsai_core::git::repo::read_repo_info(path)
        .map_err(|e| format!("cannot open --repo {}: {e}", path.display()))?;
    if !info.is_repo {
        return Err(format!("--repo is not a git repository: {}", path.display()));
    }
    if info.bare {
        return Err(format!("--repo is a bare repository (unsupported): {}", path.display()));
    }
    Ok(PathBuf::from(info.path))
}

#[tokio::main]
async fn main() -> ExitCode {
    let cfg = match parse_args(std::env::args().skip(1)) {
        Ok(ParseOutcome::Config(cfg)) => cfg,
        // `--help`: usage to STDOUT, success exit (a help request is not a failure).
        Ok(ParseOutcome::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };

    let workdir = match validate_repo(&cfg.repo) {
        Ok(wd) => wd,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };

    let server = BonsaiServer::new(workdir, cfg.allow_write);

    let service = match server.serve(rmcp::transport::stdio()).await {
        Ok(service) => service,
        Err(e) => {
            eprintln!("failed to start MCP stdio service: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = service.waiting().await {
        eprintln!("MCP service terminated with error: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience: parse a slice of string args.
    fn parse(args: &[&str]) -> Result<ParseOutcome, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    fn config(args: &[&str]) -> ServerConfig {
        match parse(args) {
            Ok(ParseOutcome::Config(c)) => c,
            other => panic!("expected Config, got {}", describe(&other)),
        }
    }

    fn describe(r: &Result<ParseOutcome, String>) -> String {
        match r {
            Ok(ParseOutcome::Config(_)) => "Ok(Config)".into(),
            Ok(ParseOutcome::Help) => "Ok(Help)".into(),
            Err(e) => format!("Err({e})"),
        }
    }

    #[test]
    fn no_args_defaults_to_cwd_read_only() {
        let cfg = config(&[]);
        assert!(!cfg.allow_write, "no --allow-write => read-only");
        assert_eq!(
            cfg.repo,
            std::env::current_dir().expect("cwd available in test"),
            "omitted --repo defaults to the current directory"
        );
    }

    #[test]
    fn repo_flag_sets_the_path() {
        let cfg = config(&["--repo", "some/where"]);
        assert_eq!(cfg.repo, PathBuf::from("some/where"));
        assert!(!cfg.allow_write);
    }

    #[test]
    fn allow_write_flag_is_recognized_order_independent() {
        assert!(config(&["--allow-write", "--repo", "r"]).allow_write);
        assert!(config(&["--repo", "r", "--allow-write"]).allow_write);
    }

    #[test]
    fn repo_without_value_is_error() {
        let err = parse(&["--repo"]).expect_err("missing value must error");
        assert!(err.contains("requires a path"), "got: {err}");
        assert!(err.contains(USAGE), "error must include usage: {err}");
    }

    #[test]
    fn help_flags_return_help_outcome() {
        assert!(matches!(parse(&["-h"]), Ok(ParseOutcome::Help)));
        assert!(matches!(parse(&["--help"]), Ok(ParseOutcome::Help)));
        // Help wins even alongside other args parsed before it.
        assert!(matches!(
            parse(&["--allow-write", "--help"]),
            Ok(ParseOutcome::Help)
        ));
    }

    #[test]
    fn unknown_argument_is_error_with_usage() {
        let err = parse(&["--nope"]).expect_err("unknown arg must error");
        assert!(err.contains("unexpected argument: --nope"), "got: {err}");
        assert!(err.contains(USAGE), "error must include usage: {err}");
    }

    #[test]
    fn validate_repo_on_non_repo_dir_is_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = validate_repo(dir.path()).expect_err("non-repo must error");
        assert!(
            err.contains("not a git repository") || err.contains("cannot open"),
            "expected a clean not-a-repo error, got: {err}"
        );
    }

    #[test]
    fn validate_repo_on_nonexistent_path_is_error() {
        let missing = std::path::Path::new("this/path/does/not/exist/at/all");
        assert!(
            validate_repo(missing).is_err(),
            "a nonexistent --repo must return an error, not panic"
        );
    }
}
