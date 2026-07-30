# bonsai-mcp

A standalone **stdio MCP server** that exposes Bonsai's differentiated Git surface —
the precomputed commit-graph layout, structured typed diffs, working-directory status,
and the ours/theirs/base conflict trio — to AI assistants (Claude Code is the reference
consumer). It is a thin adapter over `bonsai-core`; it adds no Git logic of its own.

> **MCP SDK version.** Pinned to **`rmcp` 3.0.1** (resolved at implement time, `2026-07`).
> `JsonSchema` derives use the `rmcp::schemars` re-export (schemars 1.2.2) to avoid version
> skew with the SDK. If you bump `rmcp`, re-verify the macro surface (`#[tool_router]`,
> `#[tool]`, `#[tool_handler(router = ...)]`, `Parameters<T>`, `CallToolResult`,
> `ServerCapabilities::builder().enable_tools()`), which has shifted across minor versions.

## Status

- **P14b (this increment):** server skeleton + the 12 read-only tools. `--allow-write` is
  parsed and stored but registers no mutation tools yet.
- **P14c:** mutation tools + `--allow-write` gating + conflict/merge/rebase lifecycle.
- **P14d:** scripted stdio integration test + full `claude mcp add` wiring docs.

## Usage

```text
bonsai-mcp --repo <path-to-a-non-bare-git-repo> [--allow-write]
```

On startup the server opens and validates `--repo` via
`bonsai_core::git::repo::read_repo_info` (must be a git repo and non-bare); on failure it
prints to stderr and exits non-zero **before** serving. The canonical workdir path is used
for every tool call. Transport is stdio JSON-RPC.

## Read tools (P14b)

All names are `bonsai_`-prefixed; all outputs are the exact `bonsai_core` serde types
(camelCase JSON). Tool-domain errors surface as `CallToolResult { isError: true }` carrying
the `AppError` `{ kind, message }` discriminant.

| Tool | Args | Output |
|---|---|---|
| `bonsai_get_graph` | — | `GraphLayout` |
| `bonsai_get_status` | — | `StatusSnapshot` |
| `bonsai_list_branches` | — | `BranchesSnapshot` |
| `bonsai_get_commit_diff` | `oid` | `CommitDiff` |
| `bonsai_get_commit_file_diff` | `oid`, `path`, `origPath?` | `FileDiff` |
| `bonsai_get_workdir_file_diff` | `path`, `origPath?`, `staged` | `FileDiff` |
| `bonsai_compare_with_head` | `oid` | `CompareDiff` |
| `bonsai_compare_with_head_file_diff` | `oid`, `path`, `origPath?` | `FileDiff` |
| `bonsai_get_op_state` | — | `RepoOpState` |
| `bonsai_list_conflicts` | — | `Vec<ConflictEntry>` |
| `bonsai_get_conflict` | `path` | `ConflictFile` |
| `bonsai_list_stashes` | — | `Vec<StashEntry>` |

## Registering with Claude Code

Full `claude mcp add` wiring lands in P14d. Preview:

```text
claude mcp add bonsai -- <abs path>\bonsai-mcp.exe --repo <your repo> [--allow-write]
```
