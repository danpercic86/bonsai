# bonsai-mcp

A standalone **stdio MCP server** that exposes Bonsai's differentiated Git surface —
the precomputed commit-graph layout, structured typed diffs, working-directory status,
and the ours/theirs/base conflict trio — to AI assistants (Claude Code is the reference
consumer). It is a thin adapter over `bonsai-core`; it adds no Git logic of its own.

It deliberately does **not** mirror the basic `git` commands (`status`/`add`/`commit`/…) an
AI can already run through the shell. Its value is the structured data those commands don't
hand you cheaply: the lane/edge commit **graph topology**, **typed diffs/hunks**, and the
**structured conflict model** (separated `ours`/`theirs`/base) that makes AI conflict
resolution reliable.

> **MCP SDK version.** Pinned to **`rmcp` 3.0.1** (resolved at implement time, `2026-07`).
> `JsonSchema` derives use the `rmcp::schemars` re-export (schemars 1.2.2) to avoid version
> skew with the SDK. If you bump `rmcp`, re-verify the macro surface (`#[tool_router]`,
> `#[tool]`, `#[tool_handler(router = ...)]`, `Parameters<T>`, `CallToolResult`,
> `ServerCapabilities::builder().enable_tools()`), which has shifted across minor versions.

## Usage

```text
bonsai-mcp [--repo <path-to-a-non-bare-git-repo>] [--allow-write]
```

`--repo` is optional: when omitted the server uses the **current working directory**, so a
`.mcp.json` entry at the repo root needs no path (the client launches the server with its
cwd set to the project root). Note that the path is used as-is (no upward discovery), so a
bare `bonsai-mcp` only finds a repo when the cwd is the repo root itself.

On startup the server opens and validates the resolved repo path via
`bonsai_core::git::repo::read_repo_info` (must be a git repo and non-bare); on failure it
prints to stderr and exits non-zero **before** serving. The canonical workdir path is used
for every tool call. Transport is stdio JSON-RPC.

### Safety model — read-only by default

Without `--allow-write` the server registers **only the 14 read tools**; the 20 mutation
tools are genuinely not registered, so `tools/list` is truthful and a mutation call returns a
JSON-RPC `-32602` ("tool not found") with no side effect. Pass `--allow-write` to opt in to
mutations. Even then, every mutation goes through Bonsai's safety rails (fast-forward-only
merges, never-force checkout/push, unmerged-delete blocked, autostash) — a *safer* git than a
raw shell. Network operations (`fetch`/`pull`/`push`) and the in-app AI conflict helper are
intentionally **not** exposed (see the P14 contract §7.2).

## Read tools (always available)

All names are `bonsai_`-prefixed; all outputs are the exact `bonsai_core` serde types
(camelCase JSON). Tool-domain errors surface as `CallToolResult { isError: true }` carrying
the `AppError` `{ kind, message }` discriminant, so a client can branch on `kind`
(e.g. `checkoutConflict`, `unmergedConflicts`, `emptyMessage`).

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
| `bonsai_list_repos` | — | `[OpenRepoSummary]` |
| `bonsai_select_repo` | `repoId` | `OpenRepoSummary` |

The last two are repo-management (not git-mutation) tools, always registered. In the
standalone stdio server they report/act on the single `--repo` (`bonsai_select_repo` is
rejected as a single-repo server); the embedded HTTP server (P16) uses them for per-session
selection across the app's open tabs.

## Mutation tools (only with `--allow-write`)

| Tool | Args | Output |
|---|---|---|
| `bonsai_stage` | `paths` | — |
| `bonsai_unstage` | `paths` | — |
| `bonsai_commit` | `message` | `CommitResult` |
| `bonsai_resolve_conflict_text` | `path`, `content` | — |
| `bonsai_resolve_conflict` | `path`, `resolution` (`ours`\|`theirs`\|`markResolved`) | — |
| `bonsai_merge_branch` | `name` | `MergeOutcome` |
| `bonsai_commit_merge` | `message` | `CommitResult` |
| `bonsai_abort_merge` | — | — |
| `bonsai_rebase_branch` | `onto` | `RebaseOutcome` |
| `bonsai_rebase_continue` | — | `RebaseOutcome` |
| `bonsai_rebase_skip` | — | `RebaseOutcome` |
| `bonsai_rebase_abort` | — | — |
| `bonsai_create_branch` | `name` | — |
| `bonsai_create_branch_here` | `name`, `oid` | `CreateBranchHereResult` |
| `bonsai_checkout_branch` | `name` | — |
| `bonsai_delete_branch` | `name` | — |
| `bonsai_create_stash` | `message?`, `includeUntracked` | `CreateStashResult` |
| `bonsai_apply_stash` | `index` | `ApplyStashOutcome` |
| `bonsai_pop_stash` | `index` | `ApplyStashOutcome` |
| `bonsai_drop_stash` | `index` | — |

### The conflict-resolution loop (the headline workflow)

```text
bonsai_merge_branch { name }          → MergeOutcome::conflicts { paths }
bonsai_get_conflict { path }          → { kind, ours, theirs, base, ... }   # per conflicted path
bonsai_resolve_conflict_text { path, content }   # AI-authored merged text, written + staged
bonsai_commit_merge { message }       → CommitResult
```

An integration test (`tests/mcp_stdio.rs`) drives this whole loop over stdio and asserts the
resulting **tree oid equals** a hand-resolved `git` CLI merge of the same history.

## Registering with Claude Code

There are two ways to use Bonsai's Git surface from Claude Code:

1. **Embedded HTTP server inside the Bonsai app** (recommended) — one server that
   targets the repos you already have open as tabs, live-updating the GUI as the AI
   acts. See "Embedded HTTP server (inside Bonsai)" below.
2. **Standalone stdio server** (this crate's binary) — one server bound to a single
   `--repo`, no running GUI required. See "Standalone stdio server" below.

### Embedded HTTP server (inside Bonsai)

The Bonsai desktop app hosts the same tool layer over **streamable-HTTP**, bound to
`127.0.0.1:<port>`. Enable it from the app, then register the printed one-liner:

1. In Bonsai, open **Settings → "AI access (MCP server)"** and turn the server **on**
   (a one-time consent dialog explains that an external AI client will be able to read
   — and, if you also enable write, modify — **any repo open in Bonsai**).
2. Leave **"Allow AI to modify this repo"** OFF for read-only (the default: 14 read
   tools). Turn it ON to expose the 20 mutation tools (34 total). Flipping this toggle
   **bounces** the server (drops live sessions; the token/port stay stable), so clients
   reconnect and re-negotiate the new tool set.
3. Copy the ready-made registration line shown in Settings and run it, e.g.:

   ```bash
   claude mcp add bonsai --transport http \
     --header "Authorization: Bearer <token>" \
     http://127.0.0.1:<port>/mcp
   ```

   The `<port>` is persisted across app runs and the `<token>` is a persisted
   32-byte (base64url) bearer, so this one-time registration keeps working.

**Targeting a repo.** Unlike the stdio server (one fixed `--repo`), the embedded server
serves whatever tabs are open. Each MCP session is **seeded** from the focused tab, then:

- `bonsai_list_repos` enumerates the open tabs (each with a HEAD summary and a `selected`
  flag), and
- `bonsai_select_repo({ repoId })` re-points **this session** to any open tab.

Git tools resolve the selected repo at call time; if nothing is selected (or the selected
tab was closed) they return a clean `{ kind: "noRepo" }` error asking you to select one.

**Security.** The endpoint requires the bearer token on **every** request (missing/wrong
→ `401`), rejects any request carrying an `Origin` header (`403`, no browser is ever a
legitimate client), and requires an exact loopback `Host` (`127.0.0.1:<port>` /
`localhost:<port>` → else `403`); no CORS headers are ever emitted. See the P16 contract
§8 and the `src-tauri/src/mcp.rs` integration test (`http_integration`) for the proof.

### Standalone stdio server

Build the binary, then register it (stdio):

```bash
cargo build -p bonsai-mcp --release
```

Read-only (safe — recommended first):

```bash
claude mcp add bonsai -- "D:/Repos/Playground/bonsai/target/release/bonsai-mcp.exe" --repo "D:/path/to/your/repo"
```

With mutations enabled:

```bash
claude mcp add bonsai -- "D:/Repos/Playground/bonsai/target/release/bonsai-mcp.exe" --repo "D:/path/to/your/repo" --allow-write
```

Then, in a Claude Code session in that project, the `bonsai_*` tools appear in `/mcp` and are
callable. Use a forward-slash or double-backslash-quoted path for `--repo` on Windows. One
server instance serves one repo (its `--repo`); add multiple named servers for multiple repos.

## Building & testing

```bash
cargo build -p bonsai-mcp
cargo test -p bonsai-mcp    # spawns the built binary and drives it over stdio
```
