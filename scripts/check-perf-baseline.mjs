#!/usr/bin/env node
// Bonsai — P120 lab-count ratchet (spec 008 AC 5 / AC 6).
//
// `pnpm test:perf` writes test-results/perf/perf-counts.json: per browser channel,
// per scripted interaction, deterministic counts (React commits, IPC calls,
// style recalcs, layouts, unprompted shifts, focus losses). This compares them
// to the committed ceilings in scripts/perf-baseline.json — the same ratchet
// shape as scripts/check-file-size.mjs:
//
//   * a count ABOVE its ceiling fails, naming interaction, metric, baseline, new;
//   * a count BELOW its ceiling passes, with a hint to lower the ceiling;
//   * a count with a ceiling that is "nondeterministic" or missing from the
//     counts file FAILS (a regression must not hide by flip-flopping/vanishing);
//   * "nondeterministic" counts with no ceiling are skipped with a warning;
//   * a channel / interaction / metric with no ceiling yet is warned about.
//
// Run:
//   node scripts/check-perf-baseline.mjs                    # check (exit 1 on regression)
//   node scripts/check-perf-baseline.mjs --update-baseline  # LOWER ceilings, seed missing ones
//
// `--update-baseline` never raises a ceiling: raising one is a hand-edit of the
// JSON, visible in the diff (AC 6). A regression still exits 1 in update mode.
//
// Plain Node ESM, zero dependencies. The pure functions are exported for
// scripts/check-perf-baseline.test.mjs; the CLI runs only when invoked directly.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export const COUNTS_PATH = join(REPO_ROOT, 'test-results', 'perf', 'perf-counts.json');
export const BASELINE_PATH = join(REPO_ROOT, 'scripts', 'perf-baseline.json');
export const NONDETERMINISTIC = 'nondeterministic';

const BASELINE_COMMENT =
  'Ceilings for scripts/check-perf-baseline.mjs (P120 lab counts, keyed by browser channel). They may only drop: `pnpm lint:perf -- --update-baseline` lowers them and seeds missing ones; raising one is a deliberate hand-edit.';

/** `{ [channel]: { [interaction]: { [metric]: value } } }`, or {} when absent. */
function interactionsOf(file) {
  return file && typeof file === 'object' && file.interactions && typeof file.interactions === 'object'
    ? file.interactions
    : {};
}

/** Walk every (channel, interaction, metric, value) of a counts file. */
function* entries(counts) {
  for (const [channel, byInteraction] of Object.entries(interactionsOf(counts))) {
    for (const [interaction, metrics] of Object.entries(byInteraction ?? {})) {
      for (const [metric, value] of Object.entries(metrics ?? {})) {
        yield { channel, interaction, metric, value };
      }
    }
  }
}

function ceilingOf(baseline, channel, interaction, metric) {
  const v = interactionsOf(baseline)[channel]?.[interaction]?.[metric];
  return typeof v === 'number' ? v : undefined;
}

/** Compare counts to ceilings. Returns `{ failures, drops, warnings }`, each
 *  failure/drop `{ channel, interaction, metric, baseline, value }`; a failure
 *  that is not a numeric increase also carries a `reason`.
 *
 *  Both sides are walked for every channel the counts file reports: a ceiling
 *  whose interaction/metric is absent from the counts, or whose count came back
 *  "nondeterministic", FAILS — otherwise a regression could hide by vanishing
 *  or by flip-flopping (run-to-run disagreement is itself a render-stability
 *  regression). "nondeterministic" is only a warning where there is no ceiling. */
export function compare(counts, baseline) {
  const failures = [];
  const drops = [];
  const warnings = [];
  const baselineChannels = interactionsOf(baseline);
  const warnedChannels = new Set();
  for (const { channel, interaction, metric, value } of entries(counts)) {
    const ceiling = ceilingOf(baseline, channel, interaction, metric);
    if (value === NONDETERMINISTIC) {
      if (ceiling !== undefined) {
        failures.push({ channel, interaction, metric, baseline: ceiling, value, reason: 'nondeterministic' });
      } else {
        warnings.push(`skipped ${channel} ${interaction}.${metric}: nondeterministic across runs (not ratcheted)`);
      }
      continue;
    }
    if (typeof value !== 'number') {
      if (ceiling !== undefined) {
        failures.push({ channel, interaction, metric, baseline: ceiling, value, reason: 'not a number' });
      } else {
        warnings.push(`skipped ${channel} ${interaction}.${metric}: not a number (${JSON.stringify(value)})`);
      }
      continue;
    }
    if (!(channel in baselineChannels)) {
      if (!warnedChannels.has(channel)) {
        warnedChannels.add(channel);
        warnings.push(`no ceilings for channel "${channel}" yet; seed them with \`pnpm lint:perf -- --update-baseline\``);
      }
      continue;
    }
    if (ceiling === undefined) {
      warnings.push(`no ceiling for ${channel} ${interaction}.${metric} (${value}); seed it with \`pnpm lint:perf -- --update-baseline\``);
    } else if (value > ceiling) {
      failures.push({ channel, interaction, metric, baseline: ceiling, value });
    } else if (value < ceiling) {
      drops.push({ channel, interaction, metric, baseline: ceiling, value });
    }
  }
  // The baseline side, for the channels this counts file measured.
  const countsChannels = interactionsOf(counts);
  for (const channel of Object.keys(countsChannels)) {
    for (const [interaction, ceilings] of Object.entries(baselineChannels[channel] ?? {})) {
      const measured = countsChannels[channel]?.[interaction];
      for (const [metric, ceiling] of Object.entries(ceilings ?? {})) {
        if (typeof ceiling !== 'number') continue;
        if (measured === undefined || measured === null || !(metric in measured)) {
          const reason = measured === undefined || measured === null ? 'interaction missing' : 'metric missing';
          failures.push({ channel, interaction, metric, baseline: ceiling, value: undefined, reason });
        }
      }
    }
  }
  return { failures, drops, warnings };
}

/** The lowered baseline: every numeric count below its ceiling lowers it, a
 *  count with no ceiling seeds one, and nothing is ever raised (a count above
 *  its ceiling keeps the ceiling). "nondeterministic" never enters the file. */
export function lowerBaseline(counts, baseline) {
  const next = structuredClone(interactionsOf(baseline));
  const lowered = [];
  const seeded = [];
  for (const { channel, interaction, metric, value } of entries(counts)) {
    if (typeof value !== 'number') continue;
    const ceiling = ceilingOf(baseline, channel, interaction, metric);
    if (ceiling !== undefined && value >= ceiling) continue;
    next[channel] ??= {};
    next[channel][interaction] ??= {};
    next[channel][interaction][metric] = value;
    (ceiling === undefined ? seeded : lowered).push({ channel, interaction, metric, baseline: ceiling, value });
  }
  return { baseline: { _comment: BASELINE_COMMENT, interactions: next }, lowered, seeded };
}

function failureDetail(e) {
  switch (e.reason) {
    case 'nondeterministic':
      return 'nondeterministic across runs (a count with a ceiling must be deterministic)';
    case 'interaction missing':
      return 'missing: interaction absent from the counts file';
    case 'metric missing':
      return 'missing: metric absent from the counts file';
    default:
      return `${e.reason} (${JSON.stringify(e.value)})`;
  }
}

function describe(e) {
  return `${e.channel} ${e.interaction}.${e.metric}`;
}

/** CLI. `io` is injectable so the test never touches the real files. */
export function main({
  argv = process.argv.slice(2),
  countsPath = COUNTS_PATH,
  baselinePath = BASELINE_PATH,
  readText = (p) => readFileSync(p, 'utf8'),
  writeText = (p, text) => writeFileSync(p, text, 'utf8'),
  log = console.log,
  error = console.error,
} = {}) {
  const update = argv.includes('--update-baseline');

  let counts;
  try {
    counts = JSON.parse(readText(countsPath));
  } catch {
    error(`FAIL — no readable lab counts at ${countsPath}.`);
    error('  Run `pnpm test:perf` first; it writes that file.');
    return 1;
  }
  let baseline = {};
  try {
    baseline = JSON.parse(readText(baselinePath));
  } catch {
    // No baseline yet: every count is unseeded (warned; --update-baseline seeds).
  }

  const { failures, drops, warnings } = compare(counts, baseline);
  log(`perf-count ratchet — ${[...entries(counts)].length} count(s) checked`);
  for (const w of warnings) log(`  warning: ${w}`);

  if (update) {
    const { baseline: next, lowered, seeded } = lowerBaseline(counts, baseline);
    writeText(baselinePath, `${JSON.stringify(next, null, 2)}\n`);
    for (const e of lowered) log(`  lowered: ${describe(e)} ${e.baseline} -> ${e.value}`);
    for (const e of seeded) log(`  seeded:  ${describe(e)} = ${e.value}`);
    log(`Baseline written (${lowered.length} lowered, ${seeded.length} seeded; nothing raised).`);
  } else {
    for (const e of drops) log(`  dropped: ${describe(e)} ${e.baseline} -> ${e.value}`);
    if (drops.length > 0) {
      log(`  ${drops.length} count(s) dropped — run \`pnpm lint:perf -- --update-baseline\` to lower the ceiling(s).`);
    }
  }

  if (failures.length === 0) {
    log('OK — no count exceeded its ceiling.');
    return 0;
  }
  error('');
  error('FAIL — perf-count ratchet regression:');
  for (const e of failures) {
    if (e.reason === undefined) {
      error(`  ${e.interaction}: ${e.metric} baseline ${e.baseline} -> new ${e.value} (+${e.value - e.baseline}) [${e.channel}]`);
    } else {
      error(`  ${e.interaction}: ${e.metric} baseline ${e.baseline} -> ${failureDetail(e)} [${e.channel}]`);
    }
  }
  error('');
  error('Fix the regression. If the increase is intended, raise the ceiling by hand in scripts/perf-baseline.json');
  error('(--update-baseline never raises one) and justify it in the commit.');
  return 1;
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = main();
}
