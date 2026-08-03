# Bonsai — architecture reference

> Moved out of `CLAUDE.md` to keep the always-loaded orchestrator file small. This content is
> derivable from the tree and from `docs/contracts/`; kept here for quick reference only.

## Project layout

```
src/                  React frontend (Vite)
  ipc/                typed wrappers over invoke()/listen(); mock.ts (VITE_MOCK_IPC harness)
  graph/              canvas renderer for the precomputed layout
  components/
src-tauri/            Rust backend
  src/lib.rs          builder, generate_handler!, .manage(state)
  src/commands.rs     #[tauri::command] API surface
  src/git/            git2 wrappers: status, log, diff, commit, branches, remotes
  src/graph.rs        lane/edge layout engine
  src/watcher.rs      notify -> emit("repo-changed")
  Cargo.toml
  tauri.conf.json
docs/contracts/       architect's per-milestone contract files (M<N>-<slug>.md)
TODO.md               running milestone status + current step (session resume source of truth)
```

## Reference contracts (starting point — the per-milestone contracts in `docs/contracts/` are authoritative)

```rust
struct GraphNode { id: String, lane: u32, row: u32, parents: Vec<String>, refs: Vec<RefLabel> }
struct GraphEdge { from: String, to: String, lane: u32 }
struct GraphLayout { nodes: Vec<GraphNode>, edges: Vec<GraphEdge>, lane_count: u32 }
// commands: open_repo(path), get_status(), get_graph(), get_diff(path),
//           stage(path), unstage(path), commit(msg), list_branches(),
//           checkout(name), create_branch(name), fetch(), pull(), push()
// events: "repo-changed"    channels: streamed large diffs / batched log
```
