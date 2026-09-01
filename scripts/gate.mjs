#!/usr/bin/env node
// Bonsai — one-command gate runner (dev-loop speedup, 2026-08-20).
//
// Replaces the ad-hoc "run these 9 commands in the right order" ritual with a
// single entry point that runs the AI gate, times every step, and prints a
// summary. Two things it does that a raw command list can't:
//
//   * runs the Rust tests with `cargo nextest` (parallel across the 77+ test
//     binaries) when it is installed, falling back to `cargo test` otherwise;
//   * runs `cargo clippy` in its OWN target dir (target/clippy) so it never
//     races or invalidates the test build — the documented `cargo test` vs
//     `clippy` shared-target conflict simply cannot happen here.
//
// Tiers:
//   pnpm gate            # pre-commit gate: rust + frontend + e2e
//   pnpm gate --quick    # fast inner loop: everything except e2e
//   pnpm gate --full     # gate + supply-chain audit + coverage  (~ CI)
//   pnpm gate --rust     # rust steps only
//   pnpm gate --frontend # frontend steps only
// Flags: --bail (stop at first failure), --list (print steps and exit),
//        --ci-parity (add the cross-target compile checks to any tier).
//
// CI-parity notes (both classes bit the 1.1.0 release — see TODO.md / the
// release memory):
//   * Doctests run under RUSTDOCFLAGS=-D warnings, so a rustdoc lint (e.g. a
//     prose ``` that rustdoc reads as a code fence) fails here, not only in CI.
//   * --full / --ci-parity cross-compile the pure crates for the other OS
//     targets when their rustup targets are installed, catching cfg-gated dead
//     code that -D dead_code rejects off-Windows. If a target is not installed
//     the gate says so and defers to CI's rust matrix (.github/workflows/ci.yml).
//
// Zero dependencies, plain Node ESM — same behaviour on Windows/macOS/Linux.

import { spawnSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';

const argv = new Set(process.argv.slice(2));
const has = (f) => argv.has(f);
const isWin = process.platform === 'win32';

// --- keep compile intermediates out of Defender-scanned space ---------------
// Measured 2026-09-01: agent/dev shells here run with TMP=TEMP=C:\Temp, but
// D:\Data is the Defender-excluded volume on this machine (see the global
// rule + CLAUDE.md). rustc and the linker write their intermediates to TMP, so
// every gate run was handing MsMpEng a few thousand files to scan. Point the
// child processes at the excluded volume instead.
//
// Deliberately NOT in .cargo/config.toml: cargo's `[env]` table has no
// per-target conditional, so a hardcoded D:\ path would also be exported on
// the ubuntu/macos CI legs. Windows-only, and only when the dir is available.
const SCRATCH_WIN = String.raw`D:\Data\Temp\bonsai-build`;
function scratchEnv() {
  if (!isWin) return {};
  try {
    mkdirSync(SCRATCH_WIN, { recursive: true });
    return { TMP: SCRATCH_WIN, TEMP: SCRATCH_WIN };
  } catch {
    // No D: volume (another machine, or a contributor's checkout) — stock TMP.
    return {};
  }
}
const scratch = scratchEnv();

// --- resolve tier -----------------------------------------------------------
const quick = has('--quick');
const full = has('--full') || has('--ci');
const rustOnly = has('--rust');
const frontOnly = has('--frontend');
const ciParity = has('--ci-parity');
// e2e runs in the default and full tiers, never in --quick.
const wantE2e = !quick && !rustOnly && !frontOnly;
const wantAudit = full && !rustOnly && !frontOnly;

// --- is nextest available? --------------------------------------------------
function have(cmd, args) {
  const r = spawnSync(cmd, args, { stdio: 'ignore', shell: isWin });
  return r.status === 0;
}
const hasNextest = have('cargo', ['nextest', '--version']);

// --- cross-platform compile parity ------------------------------------------
// Both CI failures during the 1.1.0 release slipped past this gate because it
// only exercised the host (Windows) target: (1) cfg(windows)-only dead code
// that -D dead_code rejects on macOS/Linux, and (2) a rustdoc lint CI escalates
// to an error. `.github/workflows/ci.yml` builds the `rust` job on
// ubuntu-22.04 + windows-latest + macos-latest under clippy/rustdoc `-D
// warnings` — that matrix stays authoritative; this is a best-effort local
// mirror, enabled by --full or --ci-parity.
const CROSS_TARGETS = ['aarch64-apple-darwin', 'x86_64-unknown-linux-gnu'];
const crossWanted = (full || ciParity) && !frontOnly;
function installedTargets() {
  const r = spawnSync('rustup', ['target', 'list', '--installed'], { encoding: 'utf8', shell: isWin });
  return r.status === 0 && r.stdout ? r.stdout.split(/\r?\n/).map((s) => s.trim()).filter(Boolean) : [];
}
const installed = crossWanted ? installedTargets() : [];
const crossPresent = CROSS_TARGETS.filter((t) => installed.includes(t));
const crossMissing = crossWanted ? CROSS_TARGETS.filter((t) => !installed.includes(t)) : [];
// Only the pure crates cross-compile cleanly from any host (the tauri app pulls
// the platform webview) — and bonsai-core is where the release dead code hid.
// clippy (not `cargo check` + RUSTFLAGS) so `-D warnings` scopes to OUR crates,
// exactly like CI's clippy step; RUSTFLAGS would also deny warnings in deps and
// fail on unrelated third-party lints. dead_code is a rustc lint clippy still
// reports, so the cfg-gated-dead-code class is caught. Own target dir (per
// triple) keeps it off the host test build.
const crossSteps = crossPresent.map((t) => ({
  name: `cargo clippy --target ${t}`,
  cmd: 'cargo',
  args: ['clippy', '-p', 'bonsai-core', '-p', 'bonsai-forge', '-p', 'bonsai-mcp', '--all-targets', '--target', t, '--', '-D', 'warnings'],
  env: { CARGO_TARGET_DIR: 'target/clippy' },
  group: 'rust',
}));

// --- step catalogue ---------------------------------------------------------
// group: 'rust' | 'frontend' | 'e2e' | 'audit'
// RUSTDOCFLAGS=-D warnings mirrors CI so a doctest/rustdoc lint fails locally.
const RUSTDOC_DENY = { RUSTDOCFLAGS: '-D warnings' };
// --quick shrinks the property-test case counts for the fast inner loop. The
// prop_* suites (crates/bonsai-core/tests/prop_*.rs) hardcode a per-suite
// `cases: N` literal, but proptest's PROPTEST_CASES env var OVERRIDES that
// literal at runtime — verified empirically: prop_graph_layout runs its baked
// 64 cases in ~89s when unset vs ~7s at PROPTEST_CASES=4. The default / --full /
// CI tiers leave it UNSET, so the suites run their full baked-in counts and CI
// thoroughness is unchanged. Only the test step needs it (doctests/clippy run
// no proptests). Determinism is preserved — a smaller run is still a subset.
//
// Why 4 and not 16 (changed 2026-09-01): PROPTEST_CASES is a FLAT per-test-fn
// override, and prop_graph_layout is now 8 banded test fns partitioning the
// commit-count axis (so nextest can parallelize what used to be one 57s test).
// A flat N therefore costs that suite 8×N cases, not N. Measured per-suite wall
// at full baked counts: prop_status 23.5s (one fn — now the workspace's slowest
// test), prop_stash_roundtrip 14.3s, prop_graph_layout 13.9s, the other two
// <0.2s. At N=16 the banded suite balloons to 128 cases / ~41s, which would
// make --quick SLOWER than before the split; at N=4 it runs 32 cases in ~10s —
// still twice the total cases the old N=16 flat override gave this suite, and
// faster. The quick-tier floor then falls back to submodule_cli (~14s), which
// is not a proptest and needs a separate fix.
const proptestEnv = quick ? { PROPTEST_CASES: '4' } : {};
const rustTest = hasNextest
  ? { name: 'cargo nextest', cmd: 'cargo', args: ['nextest', 'run', '--workspace'], group: 'rust', env: proptestEnv }
  // nextest never runs doctests; the fallback cargo test does, so deny there too.
  : { name: 'cargo test', cmd: 'cargo', args: ['test', '--workspace'], group: 'rust', env: { ...RUSTDOC_DENY, ...proptestEnv } };

const steps = [
  rustTest,
  // nextest does not run doctests; cargo test already did. Only add when nextest ran.
  hasNextest && { name: 'cargo test --doc', cmd: 'cargo', args: ['test', '--workspace', '--doc'], group: 'rust', env: RUSTDOC_DENY },
  {
    name: 'cargo clippy',
    cmd: 'cargo',
    args: ['clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'],
    // Own target dir → never races/invalidates the test build above.
    env: { CARGO_TARGET_DIR: 'target/clippy' },
    group: 'rust',
  },
  ...crossSteps,
  { name: 'eslint', cmd: 'pnpm', args: ['lint:ci'], group: 'frontend' },
  { name: 'file-size ratchet', cmd: 'pnpm', args: ['lint:size'], group: 'frontend' },
  {
    name: full ? 'vitest (coverage)' : 'vitest',
    cmd: 'pnpm',
    args: [full ? 'test:coverage' : 'test'],
    group: 'frontend',
  },
  { name: 'tsc + vite build', cmd: 'pnpm', args: ['build'], group: 'frontend' },
  { name: 'playwright e2e', cmd: 'pnpm', args: ['test:e2e'], group: 'e2e' },
  { name: 'cargo-deny', cmd: 'cargo', args: ['deny', '--all-features', 'check'], group: 'audit' },
  { name: 'pnpm audit', cmd: 'pnpm', args: ['audit', '--audit-level', 'high'], group: 'audit' },
].filter(Boolean);

// --- select by tier ---------------------------------------------------------
const wantGroup = (g) => {
  if (rustOnly) return g === 'rust';
  if (frontOnly) return g === 'frontend';
  if (g === 'e2e') return wantE2e;
  if (g === 'audit') return wantAudit;
  return g === 'rust' || g === 'frontend';
};
const selected = steps.filter((s) => wantGroup(s.group));

const tierLabel = full ? 'full (≈ CI)' : quick ? 'quick' : rustOnly ? 'rust-only' : frontOnly ? 'frontend-only' : 'pre-commit';

// Best-effort warning when a cross-target build cannot be proven locally.
function printCrossCaveat() {
  if (!crossWanted || !crossMissing.length) return;
  if (crossPresent.length) console.log(`  cross-target parity: checking ${crossPresent.join(', ')}.`);
  console.log(`  ⚠ cross-target NOT installed: ${crossMissing.join(', ')} — this gate cannot prove those builds.`);
  console.log(`    CI's rust matrix (.github/workflows/ci.yml) is authoritative. To check locally: rustup target add ${crossMissing[0]}`);
}

if (has('--list')) {
  console.log(`gate tier: ${tierLabel}  ·  nextest: ${hasNextest ? 'yes' : 'no (cargo test fallback)'}`);
  for (const s of selected) console.log(`  [${s.group}] ${s.name}: ${s.cmd} ${s.args.join(' ')}`);
  printCrossCaveat();
  process.exit(0);
}

// --- run --------------------------------------------------------------------
const fmt = (ms) => (ms < 1000 ? `${ms | 0}ms` : `${(ms / 1000).toFixed(1)}s`);
const bail = has('--bail');
const results = [];
const t0 = Number(process.hrtime.bigint() / 1000000n);

console.log(`\n▶ Bonsai gate — ${tierLabel} tier — ${selected.length} steps${hasNextest ? '' : '  (nextest not installed: `cargo install cargo-nextest --locked` for parallel tests)'}\n`);
printCrossCaveat();

for (const step of selected) {
  const label = `[${step.group}] ${step.name}`;
  console.log(`\n─── ${label} ───────────────────────────────────`);
  const start = Number(process.hrtime.bigint() / 1000000n);
  const r = spawnSync(step.cmd, step.args, {
    stdio: 'inherit',
    shell: isWin,
    env: { ...process.env, ...scratch, ...(step.env ?? {}) },
  });
  const dur = Number(process.hrtime.bigint() / 1000000n) - start;
  const ok = r.status === 0 && !r.error;
  results.push({ label, ok, dur });
  if (r.error) console.error(`  ! failed to launch: ${r.error.message}`);
  if (!ok && bail) break;
}

// --- summary ----------------------------------------------------------------
const total = Number(process.hrtime.bigint() / 1000000n) - t0;
const failed = results.filter((r) => !r.ok);
console.log(`\n════ gate summary — ${tierLabel} — ${fmt(total)} total ════`);
for (const r of results) console.log(`  ${r.ok ? '✓' : '✗'}  ${fmt(r.dur).padStart(7)}  ${r.label}`);
if (failed.length) {
  console.error(`\n✗ ${failed.length} step(s) failed: ${failed.map((r) => r.label).join(', ')}`);
  process.exit(1);
}
console.log(`\n✓ all ${results.length} steps passed`);
