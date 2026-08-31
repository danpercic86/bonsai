# Testing Bonsai

Bonsai has five test tiers. All run in CI (`.github/workflows/ci.yml`) on every push/PR to `main`.

## Quick reference

| Tier | Command | What it covers |
|---|---|---|
| Rust unit + integration | `cargo test --workspace` | git core, graph layout, diffs, status, sequencers, hooks, signing, MCP, Tauri command `_inner` seams |
| Rust property + corrupt-repo | (part of `cargo test`) | `proptest` invariants + a corrupt-`.git` matrix + race/lifecycle (`tests/prop_*.rs`, `corrupt_repo_cli.rs`, `race_lifecycle_cli.rs`) |
| Frontend unit + component | `pnpm test` | pure utils, all `repoWorkspace` hooks, React components (jsdom + Testing Library), the mock IPC layer |
| End-to-end | `pnpm test:e2e` | Playwright journeys driving the real React UI against the mock IPC harness |
| Lint / typecheck / build | `cargo clippy --workspace --all-targets -- -D warnings` · `pnpm build` | zero-warning Rust; `tsc` + `vite build` |

## Rust

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings   # run SEPARATELY — see note
```

- **Never run `cargo test` and `cargo clippy` concurrently** — they share the target dir and race,
  causing spurious failures. CI runs them sequentially in one job.
- **Windows temp:** set `TMP`/`TEMP` to a drive with space, e.g. `D:\Data\Temp`, and quote it
  (`$env:TMP='D:\Data\Temp'`) — an unquoted backslash mangles the path and breaks `link.exe`. Scratch git
  repos are created under the temp dir.
- **`BONSAI_REQUIRE_GIT_STRICT=1`** turns every git-CLI-gated test that would otherwise skip into a
  hard failure when `git` is missing. CI sets it so the suite can never silently under-run. Locally,
  leave it unset and tests needing `git`/`ssh-keygen` skip cleanly when the tool is absent.
- 3 large-fixture perf benchmarks are `#[ignore]`d by default; run with `cargo test -- --ignored`.
- Coverage: `cargo llvm-cov --workspace --summary-only` (one-time `cargo install cargo-llvm-cov
  --locked` + `rustup component add llvm-tools-preview`).

## Frontend

```bash
pnpm test              # both vitest projects (node + jsdom)
pnpm test:watch        # watch mode
pnpm test:coverage     # v8 coverage -> coverage/
pnpm build             # tsc + vite (the typecheck gate)
```

- Two vitest projects (configured in `vite.config.ts`): `*.test.ts` runs in the fast `node`
  environment (pure logic); `*.test.tsx` runs in `jsdom` with the mock IPC layer
  (`VITE_MOCK_IPC=1`). Shared setup (jest-dom matchers, canvas/ResizeObserver/matchMedia stubs) is in
  `src/test/setup.ts`.
- Component tests spy on the mock IPC object (`vi.spyOn(mockIpc, …)`); helpers live in
  `src/test/{actionHookKit,mockIpcKit}.ts`.

## End-to-end (Playwright)

```bash
pnpm test:e2e          # starts `pnpm dev:mock` automatically, runs e2e/*.spec.ts
```

- Drives the real React app against the deterministic mock IPC harness (no Tauri runtime, no network).
- **Locally on Windows** the config uses the installed Edge (`channel: 'msedge'`) so nothing is
  downloaded; **CI** uses bundled chromium. Point `PLAYWRIGHT_BROWSERS_PATH` off `C:` if anything
  would download.
- Every spec shares a fixture that **fails the test on any `console.error`/page error**.
- Fixtures + URL flags (`?fixture=20k`, `?forge=off`, `?ai=off`, `?op=merge`, `#fail` sentinels) live
  in `src/ipc/mock/`.

## What the harness cannot verify (manual USER CHECKPOINTs)

The mock harness proves all UI logic, but a few things need the real Tauri window / real services:
native OS dialogs, real forge PAT flow, real GPG/SSH signing keys, the auto-updater against a signed
release, and window/scroll feel on a real large repo. Run `pnpm tauri dev` for those.
