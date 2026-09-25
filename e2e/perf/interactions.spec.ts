/**
 * P120 part a — lab counts for the scripted interactions (spec 008 AC 4; eight since
 * P124c added `background-refresh-diff-open`).
 * Each interaction runs RUNS times, each in a fresh browser context; a metric
 * the runs disagree on is written as "nondeterministic" and never ratcheted.
 * Results land in test-results/perf/perf-counts.json (afterAll, and only when
 * every interaction passed and reported — a partial file would fail the ratchet
 * as missing interactions instead of naming the real failure), which
 * `pnpm lint:perf` compares against scripts/perf-baseline.json.
 */
import { test } from './perfFixtures';
import { INTERACTIONS } from './interactions';
import { agree, writeCounts } from './perfHelpers';
import type { InteractionCounts, RunCounts } from './perfHelpers';

// PERF_RUNS=6 is how determinism is re-validated after a harness change;
// PERF_RUNS=1 effectively disables the agreement check — never use it to seed.
const RUNS = Number(process.env.PERF_RUNS) || 3;

const results: Record<string, InteractionCounts> = {};
let channelKey = '';
let anyFailed = false;

test.describe('perf interactions', { tag: '@perf' }, () => {
  test.describe.configure({ mode: 'serial' });

  for (const interaction of INTERACTIONS) {
    test(interaction.name, async ({ freshPage, channel }) => {
      test.setTimeout(120_000);
      channelKey = channel;
      const runs: RunCounts[] = [];
      for (let i = 0; i < RUNS; i++) {
        const page = await freshPage();
        runs.push(await interaction.run(page));
        await page.context().close();
      }
      // The raw per-run counts: the evidence behind every "nondeterministic".
      console.log(`[perf] ${interaction.name} ${JSON.stringify(runs)}`);
      results[interaction.name] = agree(runs, interaction.alwaysNondeterministic);
      test.info().annotations.push({ type: 'runs', description: JSON.stringify(runs) });
    });
  }

  test.afterEach(() => {
    const info = test.info();
    if (info.status !== info.expectedStatus) anyFailed = true;
  });

  test.afterAll(() => {
    // afterEach sees the test body's status, not the fixture teardown's console
    // check — so also require every interaction to have reported.
    if (anyFailed || channelKey === '' || !INTERACTIONS.every((i) => i.name in results)) return;
    writeCounts(channelKey, results);
  });
});
