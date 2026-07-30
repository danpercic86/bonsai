//! `bonsai-mcp` — a standalone stdio MCP server exposing Bonsai's differentiated
//! Git surface (precomputed commit graph, structured diffs, the conflict trio,
//! stashes) to AI assistants. Register with:
//!
//! ```text
//! claude mcp add bonsai -- <abs path>/bonsai-mcp.exe --repo <repo> [--allow-write]
//! ```
//!
//! Startup opens+validates the `--repo` path once (non-bare git repo required)
//! and then serves JSON-RPC over stdio. Mutation tools (P14c) are gated behind
//! `--allow-write`; P14b registers only the read set.

mod server;

use std::path::PathBuf;
use std::process::ExitCode;

use rmcp::ServiceExt;

use server::BonsaiServer;

/// Parsed startup configuration.
struct ServerConfig {
    repo: PathBuf,
    allow_write: bool,
}

/// Parse `--repo <path>` (required) and `--allow-write` (flag) from argv.
///
/// Hand-rolled to avoid a heavy CLI dependency. Returns a usage string on any
/// malformed / missing argument.
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<ServerConfig, String> {
    let mut repo: Option<PathBuf> = None;
    let mut allow_write = false;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--repo" => {
                let value = it
                    .next()
                    .ok_or_else(|| "--repo requires a path argument".to_string())?;
                repo = Some(PathBuf::from(value));
            }
            "--allow-write" => allow_write = true,
            "-h" | "--help" => {
                return Err("usage: bonsai-mcp --repo <path> [--allow-write]".to_string());
            }
            other => {
                return Err(format!(
                    "unexpected argument: {other}\nusage: bonsai-mcp --repo <path> [--allow-write]"
                ));
            }
        }
    }

    let repo = repo.ok_or_else(|| {
        "missing required --repo <path>\nusage: bonsai-mcp --repo <path> [--allow-write]"
            .to_string()
    })?;

    Ok(ServerConfig { repo, allow_write })
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
        Ok(cfg) => cfg,
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
