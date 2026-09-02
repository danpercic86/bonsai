# Contributing to Bonsai

Thanks for taking an interest. This document covers how to get set up, how the codebase is
organized, and the rules that keep it coherent.

## Getting set up

See [Build from source](README.md#build-from-source) in the README for the full prerequisite list:
Rust **1.97** (pinned by [`rust-toolchain.toml`](rust-toolchain.toml), so rustup fetches it for
you), **Node 22+**, pnpm, plus a per-OS toolchain for libgit2 and the native webview:

- **Windows** — MSVC build tools ("Desktop development with C++"); WebView2.
- **macOS** — Xcode Command Line Tools (`xcode-select --install`); system WebKit.
- **Linux** — `build-essential libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
  libssl-dev` on Debian/Ubuntu (what CI installs, plus `build-essential`, which the CI runner already has), or the equivalent elsewhere. It must be
  webkit2gtk **4.1** — Tauri v2 does not link against 4.0.

In short:

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

The frontend toolchain is pinned in `package.json` (currently ESLint 10, Vite 8, TypeScript 6,
vitest 4, jsdom 30). **TypeScript 7 is deliberately not adopted**: `typescript-eslint` 8.68
hard-errors against the TypeScript 7 API, so the lint gate would break — leave TypeScript on 6
until that is fixed upstream.

## Read the contract first

Every feature in Bonsai was designed before it was written, and the design is on disk in
[`docs/contracts/`](docs/contracts) — interfaces, algorithm pseudocode, and acceptance criteria.
Contracts for in-flight work sit at the top level (~30 files); once a milestone's manual checklist
is confirmed its contract moves to [`docs/contracts/archive/`](docs/contracts/archive) (~160 files),
so grepping the live directory only costs you current work.
[`docs/contracts/INDEX.md`](docs/contracts/INDEX.md) is the one-line-per-file index. Before changing
an area, read its contract. It will tell you what the code is supposed to do, what the edge cases
are, and which invariants it was written to preserve.

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

**[TESTING.md](TESTING.md) is the reference** for all five test tiers — Rust unit/integration,
Rust property + corrupt-repo, frontend unit/component (vitest), end-to-end (Playwright), and
lint/typecheck/build — including how to run each one and the environment traps: the Windows
`TMP`/`TEMP` guidance, `BONSAI_REQUIRE_GIT_STRICT=1`, and why `cargo test` and `cargo clippy` must
never run concurrently. Read it once; it is deliberately not repeated here.

Three things that are specific to contributing:

**Cross-check against the real `git` CLI.** Bonsai's Git operations are validated by comparing
their results to real `git` on scratch repositories. If you add or change a Git operation, add a
test that compares Bonsai's output to what `git` itself produces. See the existing suites in
[`crates/bonsai-core/tests/`](crates/bonsai-core/tests) for the pattern.

**Never edit application code to make a test pass.** If a test fails, either the code is wrong or
the test's expectation is wrong — fix whichever it actually is.

**Performance-sensitive graph work must keep the perf gate green.** It is `#[ignore]`d by default,
so CI does not run it — you do:

```bash
cargo test --release --test perf_gate -- --ignored --nocapture
```

This builds a 31,000-commit synthetic fixture (via git2, cached between runs) and requires the full
layout in under 500 ms.

Some behaviour genuinely cannot be verified by any automated tier — native OS dialogs, real forge
tokens, real signing keys, the updater, and scroll feel on a large repo. Those are covered by the
`*-user-checklist.md` files in `docs/contracts/`, run by hand in `pnpm tauri dev`.

## Guardrails for experimentation

Do all Git experimentation in a **scratch repository you create yourself** (init a temp folder with
fixture history, or use `bonsai_core::fixture`). **Never** run `push`, `reset`, `rebase`, `clean`,
or `branch -D` against a real repository while testing.

## Commit messages

Bonsai uses [Conventional Commits](https://www.conventionalcommits.org/), with an optional scope:

```
feat(graph): show ahead/behind chip on the current branch
fix(ui): let op-banner actions wrap instead of squeezing the text
```

The types in use are `feat`, `fix`, `chore`, `docs`, `refactor`, and `test`. Scopes are free-form
and name the area touched (`graph`, `ui`, `ipc`, `test`, `lint-size`, …).

You will also see `wip(<milestone>): …` throughout the history — that is the maintainer's marker
for an in-progress sub-increment of a milestone, not a type for contributed PRs.

## File-size ratchet

`pnpm lint:size` enforces the project's ~500-line-per-file limit as a **ratchet**, not a flat gate
([`scripts/check-file-size.mjs`](scripts/check-file-size.mjs)):

- a file **not** in the baseline may never exceed **500 lines**;
- a file **in** the baseline may never grow past its recorded line count;
- shrinking a baselined file always passes, and is reported as reclaimed.

So a new file must land under 500 lines, and touching an already-oversized file must not make it
bigger. If you are adding to a large file, split the new work into its own module instead — that
is the whole point of the rule. Only re-baseline
([`scripts/file-size-baseline.json`](scripts/file-size-baseline.json), via
`node scripts/check-file-size.mjs --update-baseline`) after a deliberate reduction, never to
paper over growth.

## Pull requests

- Keep changes focused. One concern per PR.
- Prefer small, compiling increments over large drops.
- Run the CI gates locally before opening. These are exactly what
  [`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs, so passing here means passing there:

  ```bash
  cargo test --workspace
  cargo clippy --workspace --all-targets -- -D warnings   # run SEPARATELY from cargo test
  pnpm lint:ci        # eslint, --max-warnings 50 (the tree reports 42 warnings, 0 errors)
  pnpm lint:size      # file-size ratchet
  pnpm test           # vitest (node + jsdom projects)
  pnpm test:e2e       # Playwright vs the mock harness
  pnpm build          # tsc + vite — the typecheck gate
  ```

  Note the `-- -D warnings` on clippy: without it a warning passes locally and fails CI. CI also
  runs `cargo deny check` and `pnpm audit --audit-level high`, which only dependency changes
  affect.
- Describe what you verified and how — especially anything that needed the native window, since
  automated checks can't cover that.
- If your change adds or alters user-visible behavior, say whether it needs a manual checklist
  update.

## Good first contributions

- **macOS and Linux polish.** All three platforms build, test, and are exercised in CI, and the
  maintainer has now run the app on macOS — but Windows still has by far the most real-world use.
  Platform-specific rough edges (shortcut labels, font fallback, dialog behavior, packaging) are
  good, self-contained fixes.
- **Screenshots.** [`docs/assets/screenshots/`](docs/assets/screenshots) has a small set captured
  against the mock harness; more views, and light/dark pairs, are welcome. See
  [its README](docs/assets/screenshots/README.md) for how they are regenerated.
- **The open items in [`TODO.md`](TODO.md)**, under "SPUN-OUT ITEMS" — each is a real, scoped bug
  with the diagnosis already written down. Current candidates include the `CommandPalette`
  highlight resetting on `actions` array identity, `NumberSlider` clamping mid-typing so a field's
  own minimum is hard to type, and adding a `rustfmt.toml` + `cargo fmt --check` gate (the repo has
  never been formatted, so that one wants its own commit).

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE).
