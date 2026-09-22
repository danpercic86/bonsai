/**
 * P117 §2.2 / AC2-12 (second clause) — the `ipc.call` repo lift.
 *
 * Two halves: the pure name-table lookup, and what the instrumented proxy
 * actually puts on the wire. The "omits otherwise" half matters as much as the
 * positive one — a WRONG attribution turns a mutation's suppression into a false
 * positive, while an absent one only over-suppresses (§2.4).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { configureObs, resetObsConfigForTests } from './enabled';
import { flushNow, refreshSalt, resetBatcherForTests } from './batcher';
import { instrumentIpc, resetIpcProxyForTests } from './ipcProxy';
import { clearSessionSalt } from './redact';
import { repoIdArg } from './repoArg';
import { resetTraceForTests } from './trace';
import type { DevSettings } from '../ipc/types/settings';
import type { LogRecord } from './types';

const SALT = '00112233445566778899aabbccddeeff';
const REPO = 'D:\\Repos\\my project';

const DEV_ON: DevSettings = {
  enabled: true,
  level: 'debug',
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

interface FakeApi {
  /** Named `repoId` at position 0 in the policy table. */
  stage(repoId: string, paths: string[]): Promise<void>;
  /** Not repo-scoped: the table names `path`. */
  openRepo(path: string): Promise<{ repoId: string }>;
  logAppend(records: LogRecord[]): Promise<void>;
  logSessionInfo(): Promise<{ salt: string }>;
}

let sunk: LogRecord[] = [];

function makeApi(): FakeApi {
  return {
    async stage() {
      return undefined;
    },
    async openRepo() {
      return { repoId: 'r1' };
    },
    async logAppend(records: LogRecord[]) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: SALT };
    },
  };
}

beforeEach(() => {
  sunk = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  resetIpcProxyForTests();
  resetTraceForTests();
  clearSessionSalt();
});

afterEach(() => {
  vi.useRealTimers();
  resetObsConfigForTests();
  resetBatcherForTests();
  clearSessionSalt();
});

describe('repoIdArg — the positional name lookup', () => {
  it('lifts the repoId the policy table names', () => {
    expect(repoIdArg('stage', [REPO, ['a.txt']])).toBe(REPO);
    expect(repoIdArg('checkoutBranch', [REPO, 'main'])).toBe(REPO);
  });

  it('omits for a command whose table row has no repoId', () => {
    expect(repoIdArg('openRepo', [REPO])).toBeUndefined();
    expect(repoIdArg('openUrl', ['https://example.test'])).toBeUndefined();
  });

  it('omits for a command absent from the table — unknown never guesses', () => {
    expect(repoIdArg('getStatus', [REPO])).toBeUndefined();
    expect(repoIdArg('notACommand', [REPO])).toBeUndefined();
  });

  /**
   * Review fix 1 — the 10 recognised mutations with no `rawArgPolicy.json` row.
   * Unattributed, each suppressed `cache-collapse` in every OTHER open repo for
   * 10 s; the fallback position map fixes that without giving raw mode a new
   * `args: {repoId}` to write.
   */
  it('lifts repoId for a recognised mutation the policy table omits', () => {
    for (const cmd of [
      'fetch',
      'pull',
      'push',
      'rebaseContinue',
      'rebaseSkip',
      'rebaseAbort',
      'cherrypickContinue',
      'cherrypickAbort',
      'revertContinue',
      'revertAbort',
    ]) {
      expect(repoIdArg(cmd, [REPO]), cmd).toBe(REPO);
    }
  });

  /**
   * Negative controls. `forcePush` takes `repoId` at 0 like its neighbours but
   * is deliberately NOT in the fallback map: `is_mutation_cmd`'s table is
   * snake_case (`force_push`), so the camelCase wire name never matches it and
   * no `mutations` entry is ever created — attributing it would buy nothing
   * (tracked as a follow-up, `is_mutation_cmd` being frozen by §2.4).
   * `cloneRepo` has no repo yet when it runs.
   */
  it('omits for commands deliberately left out of the fallback map', () => {
    expect(repoIdArg('forcePush', [REPO])).toBeUndefined();
    expect(repoIdArg('cloneRepo', ['https://example.test/r.git', REPO])).toBeUndefined();
  });

  it('never lets a prototype key masquerade as a position', () => {
    expect(repoIdArg('constructor', [REPO])).toBeUndefined();
    expect(repoIdArg('toString', [REPO])).toBeUndefined();
  });

  it('omits when the named position is not a non-empty string', () => {
    expect(repoIdArg('stage', [undefined, ['a']])).toBeUndefined();
    expect(repoIdArg('stage', [42, ['a']])).toBeUndefined();
    expect(repoIdArg('stage', ['', ['a']])).toBeUndefined();
    expect(repoIdArg('stage', [])).toBeUndefined();
  });
});

describe('ipcProxy — the repo dimension on the wire', () => {
  it('sets repo on ipc.call, RAW, in the default strict mode', async () => {
    // `instrumentIpc` is what attaches the sink, so it must run BEFORE the salt
    // fetch — the proxy stays inert until `redactionReady()`.
    const ipc = instrumentIpc(makeApi());
    configureObs(DEV_ON); // includeRawNames: false ⇒ strict
    await refreshSalt();
    await ipc.stage(REPO, ['a.txt']);
    await flushNow();
    const call = sunk.find((r) => r.kind === 'ipc.call');
    expect(call?.repo).toBe(REPO);
    // Strict mode elides argument VALUES, so this is the only place the repo
    // appears — which is exactly why §2.2 put it on the record base.
    expect(call?.args).toBeUndefined();
  });

  it('omits repo for a command the name table does not repo-scope', async () => {
    const ipc = instrumentIpc(makeApi());
    configureObs(DEV_ON);
    await refreshSalt();
    await ipc.openRepo(REPO);
    await flushNow();
    const call = sunk.find((r) => r.kind === 'ipc.call');
    expect(call).toBeTruthy();
    expect('repo' in (call as object)).toBe(false);
  });
});
