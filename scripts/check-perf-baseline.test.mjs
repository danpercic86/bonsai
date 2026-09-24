/**
 * P120 — the lab-count ratchet's guarantees (spec 008 AC 5 / AC 6):
 *   1. a raised count fails and the message names interaction, metric,
 *      baseline and new value;
 *   2. a dropped count passes with a lower-it hint;
 *   3. `--update-baseline` lowers and seeds, NEVER raises, and never writes
 *      "nondeterministic";
 *   4. "nondeterministic" without a ceiling is skipped with a warning; a
 *      ceiling'd metric that is "nondeterministic" or missing (metric or whole
 *      interaction) FAILS; a missing counts file fails with instructions.
 * All I/O goes through `main`'s injected readers/writers — no real files.
 */
import { describe, expect, it } from 'vitest';
import { compare, lowerBaseline, main } from './check-perf-baseline.mjs';

const metrics = (o) => ({
  reactCommits: 3,
  ipcCalls: 1,
  recalcStyle: 4,
  layout: 3,
  layoutShiftsUnprompted: 0,
  focusLosses: 0,
  ...o,
});
const without = (o, key) => Object.fromEntries(Object.entries(o).filter(([k]) => k !== key));
const file = (byInteraction, channel = 'msedge') => ({ interactions: { [channel]: byInteraction } });

function run({ counts, baseline, argv = [] }) {
  const files = new Map([['counts', counts === undefined ? undefined : JSON.stringify(counts)]]);
  if (baseline !== undefined) files.set('baseline', JSON.stringify(baseline));
  const out = [];
  const err = [];
  const code = main({
    argv,
    countsPath: 'counts',
    baselinePath: 'baseline',
    readText: (p) => {
      const t = files.get(p);
      if (t === undefined) throw new Error(`ENOENT ${p}`);
      return t;
    },
    writeText: (p, t) => files.set(p, t),
    log: (l) => out.push(l),
    error: (l) => err.push(l),
  });
  const written = files.get('baseline');
  return { code, out: out.join('\n'), err: err.join('\n'), baseline: written === undefined ? undefined : JSON.parse(written) };
}

describe('check-perf-baseline', () => {
  it('fails on a raised count, naming interaction, metric, baseline and new value', () => {
    const r = run({
      counts: file({ 'select-commit': metrics({ reactCommits: 5 }) }),
      baseline: file({ 'select-commit': metrics() }),
    });
    expect(r.code).toBe(1);
    expect(r.err).toContain('select-commit: reactCommits baseline 3 -> new 5');
  });

  it('passes on a dropped count and hints at lowering the ceiling', () => {
    const r = run({
      counts: file({ 'select-commit': metrics({ layout: 1 }) }),
      baseline: file({ 'select-commit': metrics() }),
    });
    expect(r.code).toBe(0);
    expect(r.out).toContain('dropped: msedge select-commit.layout 3 -> 1');
    expect(r.out).toContain('--update-baseline');
  });

  it('--update-baseline lowers, never raises, and still fails the regression', () => {
    const r = run({
      argv: ['--update-baseline'],
      counts: file({ a: metrics({ reactCommits: 9, layout: 1 }) }),
      baseline: file({ a: metrics() }),
    });
    expect(r.baseline.interactions.msedge.a.layout).toBe(1); // lowered
    expect(r.baseline.interactions.msedge.a.reactCommits).toBe(3); // NOT raised to 9
    expect(r.code).toBe(1);
  });

  it('--update-baseline seeds a channel that has no entry', () => {
    const r = run({
      argv: ['--update-baseline'],
      counts: file({ a: metrics() }, 'chromium'),
      baseline: file({ a: metrics({ reactCommits: 7 }) }),
    });
    expect(r.code).toBe(0);
    expect(r.baseline.interactions.chromium.a).toEqual(metrics());
    expect(r.baseline.interactions.msedge.a.reactCommits).toBe(7); // other channel untouched
  });

  it('skips "nondeterministic" with no ceiling with a warning and never writes it', () => {
    const counts = file({ 'cold-open': metrics({ recalcStyle: 'nondeterministic' }) });
    const check = run({ counts, baseline: file({ 'cold-open': without(metrics(), 'recalcStyle') }) });
    expect(check.code).toBe(0);
    expect(check.out).toContain('skipped msedge cold-open.recalcStyle: nondeterministic');

    const seed = run({ argv: ['--update-baseline'], counts });
    expect(seed.baseline.interactions.msedge['cold-open']).not.toHaveProperty('recalcStyle');
    expect(JSON.stringify(seed.baseline)).not.toContain('nondeterministic');
  });

  it('fails when a metric with a ceiling comes back "nondeterministic"', () => {
    const r = run({
      counts: file({ 'stage-unstage': metrics({ reactCommits: 'nondeterministic' }) }),
      baseline: file({ 'stage-unstage': metrics() }),
    });
    expect(r.code).toBe(1);
    expect(r.err).toContain('stage-unstage: reactCommits baseline 3 -> nondeterministic across runs');
  });

  it('fails when a metric with a ceiling is missing from the counts', () => {
    const r = run({
      counts: file({ 'select-commit': without(metrics(), 'layout') }),
      baseline: file({ 'select-commit': metrics() }),
    });
    expect(r.code).toBe(1);
    expect(r.err).toContain('select-commit: layout baseline 3 -> missing: metric absent');
  });

  it('fails when an interaction with ceilings is missing from the counts', () => {
    const r = run({
      counts: file({ 'select-commit': metrics() }),
      baseline: file({ 'select-commit': metrics(), 'scroll-20k': metrics() }),
    });
    expect(r.code).toBe(1);
    expect(r.err).toContain('scroll-20k: reactCommits baseline 3 -> missing: interaction absent');
  });

  it('does not demand ceilings of a channel the counts file did not measure', () => {
    const r = compare(file({ a: metrics() }, 'msedge'), {
      interactions: { msedge: { a: metrics() }, chromium: { a: metrics(), b: metrics() } },
    });
    expect(r.failures).toEqual([]);
  });

  it('keeps an existing ceiling when the new count is nondeterministic', () => {
    const { baseline } = lowerBaseline(
      file({ a: metrics({ layout: 'nondeterministic' }) }),
      file({ a: metrics({ layout: 6 }) }),
    );
    expect(baseline.interactions.msedge.a.layout).toBe(6);
  });

  it('warns (does not fail) for a channel with no ceilings in check mode', () => {
    const r = compare(file({ a: metrics() }, 'chromium'), file({ a: metrics() }));
    expect(r.failures).toEqual([]);
    expect(r.warnings.join('\n')).toContain('no ceilings for channel "chromium"');
  });

  it('fails with instructions when the counts file is missing', () => {
    const r = run({ counts: undefined, baseline: file({}) });
    expect(r.code).toBe(1);
    expect(r.err).toContain('Run `pnpm test:perf` first');
  });
});
