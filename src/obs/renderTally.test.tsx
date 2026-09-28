/**
 * `render.tally` states whether StrictMode doubling applies (user ruling
 * 2026-09-28) so Rust's `render-storm` can judge REAL renders. `renders` itself
 * stays raw — the StrictMode budget tests sum it.
 */
import { StrictMode } from 'react';
import { cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { attachSink, flushNow, resetBatcherForTests } from './batcher';
import { configureObs, resetObsConfigForTests } from './enabled';
import { useRenderCount } from './react';
import { clearSessionSalt, setSessionSalt } from './redact';
import {
  __resetRenderTally,
  flushRenderTally,
  setStrictModeDoubling,
  tallyRender,
} from './renderTally';
import type { DevSettings } from '../ipc/types/settings';
import type { LogRecord } from './types';

const DEV_TRACE: DevSettings = {
  enabled: true,
  level: 'trace',
  captureIpc: true,
  captureReact: true,
  captureFrames: true,
  includeRawNames: false,
};
const SALT = '00112233445566778899aabbccddeeff';

let sunk: LogRecord[] = [];

async function tallies(): Promise<LogRecord[]> {
  flushRenderTally();
  await flushNow();
  return sunk.filter((r) => r.kind === 'render.tally');
}

function Probe(): null {
  useRenderCount('Probe', undefined, 'aggregate');
  return null;
}

beforeEach(() => {
  sunk = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  __resetRenderTally();
  attachSink({
    async logAppend(records) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: SALT };
    },
  });
  setSessionSalt(SALT);
  configureObs(DEV_TRACE);
});

afterEach(() => {
  cleanup();
  resetObsConfigForTests();
  resetBatcherForTests();
  __resetRenderTally();
  clearSessionSalt();
});

describe('render.tally strictModeDoubling', () => {
  it('omits the flag by default (non-StrictMode mount)', async () => {
    tallyRender('Row', 'Row#1', undefined, undefined);
    const [t] = await tallies();
    expect(t).toMatchObject({ kind: 'render.tally', renders: 1, instances: 1 });
    expect(t).not.toHaveProperty('strictModeDoubling');
  });

  it('stamps the flag when declared, leaving renders raw under StrictMode', async () => {
    setStrictModeDoubling(true);
    render(
      <StrictMode>
        <Probe />
      </StrictMode>,
    );
    const [t] = await tallies();
    expect(t).toMatchObject({
      kind: 'render.tally',
      component: 'Probe',
      renders: 2,
      instances: 1,
      strictModeDoubling: true,
    });
  });

  it('is cleared by the test reset', async () => {
    setStrictModeDoubling(true);
    __resetRenderTally();
    tallyRender('Row', 'Row#1', undefined, undefined);
    const [t] = await tallies();
    expect(t).not.toHaveProperty('strictModeDoubling');
  });
});
