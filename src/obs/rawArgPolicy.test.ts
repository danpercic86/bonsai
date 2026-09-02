// @vitest-environment jsdom
/**
 * P91 Amendment A26 (`docs/contracts/P91-raw-args-privacy.md` §H, AC3–AC5) —
 * the producer half of the raw-mode `args` allow-list.
 *
 * `vocabulary drift` is the forcing function on future additions: it must fail
 * the moment someone lists `message`, `token`, `prompt`, … or a command that is
 * not on `IpcApi`.
 */
import { describe, expect, it } from 'vitest';
// The mock api is the one RUNTIME enumeration of every `IpcApi` method (it
// implements the interface), which is what makes the key check executable.
// It reads `window.location` at import time, hence the jsdom environment.
import { mockIpc } from '../ipc/mock';
import {
  buildRawArgs,
  isFreeTextParam,
  isSensitiveParam,
  RAW_ARG_MAX_STR,
  RAW_ARG_POLICY,
} from './rawArgPolicy';

const POLICY: Readonly<Record<string, readonly (string | null)[]>> = RAW_ARG_POLICY;

describe('A26 vocabulary drift guard (AC5)', () => {
  it('lists no free-text or credential parameter name', () => {
    const offenders: string[] = [];
    for (const [cmd, row] of Object.entries(POLICY)) {
      for (const name of row) {
        if (name === null) continue;
        if (isFreeTextParam(name) || isSensitiveParam(name)) offenders.push(`${cmd}.${name}`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it('keys every row by a real IpcApi method name', () => {
    const methods = new Set(Object.keys(mockIpc));
    expect(Object.keys(POLICY).filter((cmd) => !methods.has(cmd))).toEqual([]);
  });

  it('names every position by a lower-camel key the writer will accept', () => {
    const bad: string[] = [];
    for (const [cmd, row] of Object.entries(POLICY)) {
      for (const name of row) {
        if (name !== null && !/^[a-z][A-Za-z0-9]*$/.test(name)) bad.push(`${cmd}.${name}`);
      }
    }
    expect(bad).toEqual([]);
  });

  it('rejects the names the vocabularies exist to catch', () => {
    for (const n of ['message', 'msg', 'query', 'searchText', 'prompt', 'noteBody', 'content']) {
      expect(isFreeTextParam(n)).toBe(true);
    }
    for (const n of ['token', 'accessToken', 'password', 'authHeader', 'apiKey', 'pat']) {
      expect(isSensitiveParam(n)).toBe(true);
    }
    for (const n of ['repoId', 'path', 'origPath', 'oid', 'refName', 'remote', 'url']) {
      expect(isFreeTextParam(n) || isSensitiveParam(n)).toBe(false);
    }
  });
});

describe('buildRawArgs (AC3, AC4)', () => {
  it('denies by default: an unlisted command yields no args at all', () => {
    expect(buildRawArgs('someBrandNewCommand', ['r1', 'anything'])).toEqual({ omitted: 2 });
  });

  it('keeps only the allow-listed scalars of `commit`, never the message', () => {
    const out = buildRawArgs('commit', ['r1', 'FIXTURE_MESSAGE_ZQX', false, true]);
    expect(out.args).toEqual({ repoId: 'r1', sign: false, skipHooks: true });
    expect(out.omitted).toBe(1);
    expect(JSON.stringify(out)).not.toContain('FIXTURE_MESSAGE_ZQX');
  });

  it('keeps only `repoId` for forgeSetToken and only host/kind for the account setters', () => {
    const pat = 'ghp_FIXTURE_TOKEN_ZQX0000000000000000';
    const set = buildRawArgs('forgeSetToken', ['r1', pat]);
    expect(set.args).toEqual({ repoId: 'r1' });
    expect(JSON.stringify(set)).not.toContain(pat);

    for (const cmd of ['forgeAddAccount', 'forgeSetTokenForHost']) {
      const out = buildRawArgs(cmd, ['github.com', 'gitHub', pat]);
      expect(out.args).toEqual({ host: 'github.com', kind: 'gitHub' });
      expect(out.omitted).toBe(1);
      expect(JSON.stringify(out)).not.toContain(pat);
    }
  });

  it('elides an object argument by shape alone (searchCommits)', () => {
    const out = buildRawArgs('searchCommits', ['r1', { text: 'FIXTURE_QUERY_ZQX' }]);
    expect(out.args).toEqual({ repoId: 'r1' });
    expect(JSON.stringify(out)).not.toContain('FIXTURE_QUERY_ZQX');
  });

  it('widens IDENTIFIER fidelity: real paths, refs and remotes survive', () => {
    expect(buildRawArgs('blameFile', ['r1', 'src/App.tsx', 'abc123']).args).toEqual({
      repoId: 'r1',
      path: 'src/App.tsx',
      atOid: 'abc123',
    });
    expect(buildRawArgs('addRemote', ['r1', 'origin', 'https://example.test/r.git']).args).toEqual({
      repoId: 'r1',
      name: 'origin',
      url: 'https://example.test/r.git',
    });
  });

  it('elides arrays, functions, oversized and multi-line strings', () => {
    expect(buildRawArgs('stage', ['r1', ['a.txt', 'b.txt']]).args).toEqual({ repoId: 'r1' });
    expect(buildRawArgs('cloneRepo', ['u', 'd', () => undefined]).omitted).toBe(1);
    const long = 'x'.repeat(RAW_ARG_MAX_STR + 1);
    expect(buildRawArgs('openRepo', [long])).toEqual({ omitted: 1 });
    expect(buildRawArgs('openRepo', ['a\nb'])).toEqual({ omitted: 1 });
    expect(buildRawArgs('openRepo', ['x'.repeat(RAW_ARG_MAX_STR)]).args).toBeDefined();
  });

  it('elides trailing actual arguments beyond the policy row', () => {
    const out = buildRawArgs('openRepo', ['D:/repos/x', 'surprise']);
    expect(out.args).toEqual({ path: 'D:/repos/x' });
    expect(out.omitted).toBe(1);
  });
});
