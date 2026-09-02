/**
 * REGRESSION (audit F4) — `logExportSession` takes NO destination path.
 *
 * The command used to accept `dest?: string | null`, justified by "it is the
 * result of the OS save dialog". No such dialog exists: the only production
 * caller invokes it with no argument, so the parameter was an unmediated IPC
 * surface — anything with script execution in the webview could make the backend
 * create directories and write a zip anywhere the process can write, outside the
 * `logsDeleteAll` scope (§6.2).
 *
 * The Rust side is pinned by `commands/tests_obs.rs`; this pins the frontend
 * halves — the typed surface, the Tauri wrapper's payload, and the mock — since
 * a reintroduced argument would type-check silently on a `dest?:` signature.
 *
 * The mock handler module reads `window.location` at import time (harness query
 * flags), so this `.test.ts` opts into jsdom rather than the node project.
 *
 * @vitest-environment jsdom
 */
import { describe, expect, it } from 'vitest';
import type { LogRecord } from '../types';
import { obsHandlers } from './handlers/obs';
import { ringAppend } from './obsRing';

// Every shipped module under src/ as raw text, keyed by path (same technique as
// `src/obs/wiring.guard.test.ts` — no @types/node in this tsconfig).
const sources = import.meta.glob('/src/**/*.{ts,tsx}', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

function source(suffix: string): string {
  const hit = Object.entries(sources).find(([path]) => path.endsWith(suffix));
  expect(hit, `source not found: ${suffix}`).toBeDefined();
  return hit?.[1] ?? '';
}

describe('logExportSession has no caller-supplied destination', () => {
  it('declares no parameter in the IpcApi surface', () => {
    expect(source('ipc/types/ipc-api-obs.ts')).toContain('logExportSession(): Promise<string>;');
  });

  it('sends no `dest` in the Tauri invoke payload', () => {
    const wrapper = source('ipc/tauri/obs.ts');
    expect(wrapper).toContain('logExportSession(): Promise<string>');
    expect(wrapper).toContain("invoke<string>('log_export_session')");
    // No payload object at all — a `{ dest }` would be the regression.
    expect(wrapper).not.toContain("'log_export_session',");
  });

  it('accepts no argument on the mock and always resolves inside exports/', async () => {
    // The mock mirrors the backend refusal ("there are no log files to export")
    // while nothing has been recorded, so seed one record first.
    ringAppend([
      { seq: 0, ts: 1, mono: 1, src: 'ui', lvl: 'debug', kind: 'note', msg: 'seed' },
    ] as unknown as LogRecord[]);
    expect(obsHandlers.logExportSession).toHaveLength(0);
    const out = await obsHandlers.logExportSession();
    expect(out.startsWith('/mock/config/com.bonsai.app/exports/')).toBe(true);
    expect(out.endsWith('.zip')).toBe(true);
  });
});
