/**
 * P91 increment 7d — the anti-regression guard.
 *
 * The entire frontend observability pipeline was built and unit-tested but,
 * before 7d, activated by NOTHING in production: `configureObs` (the master
 * switch) and `attachSink` (the log-flush sink) had only test callers, so the
 * proxy, trace minting and batcher all silently no-opped. This test fails if
 * either wire loses its single production caller again.
 *
 * Mirrors the invoke-import guard (src/ipc/tauri/invoke.test.ts): it reads the
 * shipped source via Vite's `?raw` glob — no @types/node in this tsconfig —
 * excluding *.test.* and the file that DEFINES each symbol.
 */
import { describe, expect, it } from 'vitest';

// Every shipped module under src/ as raw text, keyed by path.
const sources = import.meta.glob('../**/*.{ts,tsx}', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

function callers(call: RegExp, definingSuffix: string): string[] {
  return Object.entries(sources)
    .filter(([path]) => !/\.test\.(ts|tsx)$/.test(path) && !path.endsWith(definingSuffix))
    .filter(([, src]) => call.test(src))
    .map(([path]) => path);
}

describe('obs pipeline stays wired to production (increment 7d)', () => {
  it('sees the shipped source tree', () => {
    expect(Object.keys(sources).length).toBeGreaterThan(50);
  });

  it('configureObs has at least one production caller', () => {
    expect(callers(/\bconfigureObs\s*\(/, 'obs/enabled.ts').length).toBeGreaterThan(0);
  });

  it('attachSink has at least one production caller', () => {
    expect(callers(/\battachSink\s*\(/, 'obs/batcher.ts').length).toBeGreaterThan(0);
  });
});
