# Contributing to Bonsai

Thanks for taking an interest. This document covers how to get set up, how the codebase is
organized, and the rules that keep it coherent.

## Getting set up

See [Getting started](README.md#getting-started) in the README for prerequisites. In short:

```bash
pnpm install
pnpm tauri dev
```

The first build vendors and compiles libgit2 — expect ten-plus minutes. It is not stuck.

For frontend work you do **not** need the Rust toolchain at all:

```bash
pnpm dev:mock
```

This runs the React app on `http://localhost:1420` against the stateful mock backend in
[`src/ipc/mock.ts`](src/ipc/mock.ts), with fixture data including a 20k-commit graph.

## Read the contract first

Every feature in Bonsai was designed before it was written, and the design is on disk in
[`docs/contracts/`](docs/contracts) — 72 documents with interfaces, algorithm pseudocode, and
acceptance criteria. Before changing an area, read its contract. It will tell you what the code is
supposed to do, what the edge cases are, and which invariants it was written to preserve.

Many contracts have a companion `*-user-checklist.md` — the manual smoke test for that feature in
the native app.

## Architecture rules

These are not stylistic preferences. Changes that break them will be asked to change.

1. **Rust owns all Git logic and all commit-graph layout math.** React renders precomputed data; it
   never computes lanes, edges, or diff structure.
2. **IPC carries compact, precomputed payloads.** Commands are request/response, events are small
   push signals, channels stream large or incremental data.
3. **git2 is blocking.** Every heavy call goes through `spawn_blocking`. Never block the Tauri
   command thread.
4. **The graph renders on canvas, virtualized to visible rows.** No DOM-per-commit.
5. **The `notify` watcher is never the only refresh path.** It misses events on Windows, so it is
   always paired with a manual refresh button and a rescan on window focus, and its events are
   debounced (~300 ms) because Git operations fire event storms.
6. **The mock IPC layer must stay in sync.** If you change an IPC command's signature or types,
   update [`src/ipc/mock.ts`](src/ipc/mock.ts) and [`src/ipc/types.ts`](src/ipc/types.ts) in the
   same change. A broken harness blocks all frontend verification.
7. **Destructive Git operations require explicit UI confirmation.** No exceptions.
8. **Business logic belongs in `crates/bonsai-core`**, which is Tauri-free and unit-testable.
   `src-tauri/src/commands.rs` should be a thin wrapper — that's what makes the core testable
   without a Tauri runtime.

## Testing

Bonsai's Git operations are validated by **cross-checking against the real `git` CLI** on scratch
repositories. If you add or change a Git operation, add a test that compares Bonsai's output to
what `git` itself produces. See the existing suites in
[`crates/bonsai-core/tests/`](crates/bonsai-core/tests) for the pattern.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
pnpm build
```

Performance-sensitive graph work must keep the perf gate green:

```bash
cargo test --release --test perf_gate -- --ignored --nocapture
```

This builds a 31,000-commit synthetic fixture (via git2, cached between runs) and requires the full
layout in under 500 ms.

**Never edit application code to make a test pass.** If a test fails, either the code is wrong or
the test's expectation is wrong — fix whichever it actually is.

### Frontend testing

There is currently no frontend test runner. UI changes are verified through the mock harness plus
the manual checklists in `docs/contracts/`. **Introducing a proper frontend test setup is a very
welcome contribution** — please open an issue to discuss the approach first.

## Guardrails for experimentation

Do all Git experimentation in a **scratch repository you create yourself** (init a temp folder with
fixture history, or use `bonsai_core::fixture`). **Never** run `push`, `reset`, `rebase`, `clean`,
or `branch -D` against a real repository while testing.

## Pull requests

- Keep changes focused. One concern per PR.
- Prefer small, compiling increments over large drops.
- Run `cargo test`, `cargo clippy`, and `pnpm build` before opening.
- Describe what you verified and how — especially anything that needed the native window, since
  automated checks can't cover that.
- If your change adds or alters user-visible behavior, say whether it needs a manual checklist
  update.

## Good first contributions

- **A verified macOS / Linux build.** The workspace is cross-platform and icon assets for all three
  platforms are configured, but no full build-bundle-and-run pass has been done outside Windows.
- **CI.** There is no `.github/workflows` directory at all. `cargo test` + `cargo clippy` +
  `pnpm build` on push would be immediately valuable.
- **Frontend tests.** See above.

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE).
