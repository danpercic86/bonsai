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
