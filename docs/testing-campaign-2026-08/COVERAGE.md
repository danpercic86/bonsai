# Coverage baseline — testing campaign 2026-08

T1 records baselines only; NO thresholds are enforced yet (contract §1.3/§3).

## Frontend (vitest v8 coverage)

Produced by `pnpm test:coverage` → `coverage/` (text + html + json-summary).
Aggregates the `node` and `dom` projects; excludes tests, `src/test/**`,
`src/ipc/tauri.ts` (Tauri-runtime-only), and `.d.ts`.

Baseline (2026-08-09, T1 increment):

| metric | % | covered/total |
|---|---|---|
| statements | 3.98 | 482/12101 |
| branches | 4.00 | 316/7897 |
| functions | 2.98 | 84/2812 |
| lines | 3.87 | 412/10643 |

(Expected low: pre-T1 the suite was pure-logic `node` tests only; component
coverage starts in Phase T3.)

After T3 (2026-08-09, 109 files / 1317 tests):

| metric | % (all files) |
|---|---|
| statements | 61.18 |
| branches | 54.51 |
| functions | 56.44 |
| lines | 62.71 |

The uncovered remainder is dominated by the `RepoWorkspace.tsx` container (state
wiring — exercised end-to-end by Playwright, not unit tests), the `GraphCanvas.tsx`
imperative paint loop (pure geometry/viewport/hitTest extracted + unit-tested in
T3.6; the `ctx.*` draw calls are covered by e2e specs 02/03), and `ipc/tauri.ts`
(Tauri-runtime-only, coverage-excluded). Component/hook/util/mock logic is broadly
covered; the e2e suite (88 journeys) covers the integrated container paths.

## Rust (cargo llvm-cov)

One-time setup: `rustup component add llvm-tools-preview` then
`cargo install cargo-llvm-cov --locked` (compiled locally — never cargo-binstall).

Baseline run (PowerShell, sequential — NEVER concurrent with other cargo commands;
llvm-cov uses its own profile dir, so expect a full rebuild — do not interleave
with `cargo test`/`clippy`):

```powershell
$env:TMP = 'D:\Temp'; $env:TEMP = 'D:\Temp'
cargo llvm-cov --workspace --summary-only
cargo llvm-cov --workspace --html --output-dir D:\Temp\bonsai-llvm-cov
```

Per-crate line/region % baseline: PENDING (run deferred to the orchestrator —
full-workspace instrumented rebuild + test run).
