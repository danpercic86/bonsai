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
// Flags: --bail (stop at first failure), --list (print steps and exit).
//
// Zero dependencies, plain Node ESM — same behaviour on Windows/macOS/Linux.

import { spawnSync } from 'node:child_process';

const argv = new Set(process.argv.slice(2));
const has = (f) => argv.has(f);
const isWin = process.platform === 'win32';

// --- resolve tier -----------------------------------------------------------
const quick = has('--quick');
const full = has('--full') || has('--ci');
const rustOnly = has('--rust');
const frontOnly = has('--frontend');
// e2e runs in the default and full tiers, never in --quick.
const wantE2e = !quick && !rustOnly && !frontOnly;
const wantAudit = full && !rustOnly && !frontOnly;

// --- is nextest available? --------------------------------------------------
function have(cmd, args) {
  const r = spawnSync(cmd, args, { stdio: 'ignore', shell: isWin });
  return r.status === 0;
}
const hasNextest = have('cargo', ['nextest', '--version']);

// --- step catalogue ---------------------------------------------------------
// group: 'rust' | 'frontend' | 'e2e' | 'audit'
const rustTest = hasNextest
  ? { name: 'cargo nextest', cmd: 'cargo', args: ['nextest', 'run', '--workspace'], group: 'rust' }
  : { name: 'cargo test', cmd: 'cargo', args: ['test', '--workspace'], group: 'rust' };

const steps = [
  rustTest,
  // nextest does not run doctests; cargo test already did. Only add when nextest ran.
  hasNextest && { name: 'cargo test --doc', cmd: 'cargo', args: ['test', '--workspace', '--doc'], group: 'rust' },
  {
    name: 'cargo clippy',
    cmd: 'cargo',
    args: ['clippy', '--workspace', '--all-targets', '--', '-D', 'warnings'],
    // Own target dir → never races/invalidates the test build above.
    env: { CARGO_TARGET_DIR: 'target/clippy' },
    group: 'rust',
  },
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

if (has('--list')) {
  console.log(`gate tier: ${tierLabel}  ·  nextest: ${hasNextest ? 'yes' : 'no (cargo test fallback)'}`);
  for (const s of selected) console.log(`  [${s.group}] ${s.name}: ${s.cmd} ${s.args.join(' ')}`);
  process.exit(0);
}

// --- run --------------------------------------------------------------------
const fmt = (ms) => (ms < 1000 ? `${ms | 0}ms` : `${(ms / 1000).toFixed(1)}s`);
const bail = has('--bail');
const results = [];
const t0 = Number(process.hrtime.bigint() / 1000000n);

console.log(`\n▶ Bonsai gate — ${tierLabel} tier — ${selected.length} steps${hasNextest ? '' : '  (nextest not installed: `cargo install cargo-nextest --locked` for parallel tests)'}\n`);

for (const step of selected) {
  const label = `[${step.group}] ${step.name}`;
  console.log(`\n─── ${label} ───────────────────────────────────`);
  const start = Number(process.hrtime.bigint() / 1000000n);
  const r = spawnSync(step.cmd, step.args, {
    stdio: 'inherit',
    shell: isWin,
    env: step.env ? { ...process.env, ...step.env } : process.env,
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
