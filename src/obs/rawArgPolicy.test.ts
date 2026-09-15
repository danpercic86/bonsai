// @vitest-environment jsdom
/**
 * P91 Amendment A26 (`docs/contracts/P91-observability.md §7.4.3` §H, AC3–AC5) —
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

/**
 * The vocabulary guard above pins NAMES; this one pins POSITIONS.
 *
 * A signature reorder is the one failure the writer-side backstop cannot catch:
 * if `createTag(repoId, name, targetOid, message, force)` became
 * `createTag(repoId, name, message, targetOid, force)`, the policy would relabel
 * the MESSAGE as `targetOid` — an identifier-looking key holding a one-line
 * string, i.e. legal under the writer's W3/W4/W5. This test is the only defence,
 * so it re-derives the parameter names from the `IpcApi` sources at test time.
 */
// The `IpcApi` sources as raw text (it extends the other two interfaces). A glob
// rather than a hardcoded list, so a fourth `ipc-api-*.ts` is covered the day it
// lands; the key assertion below fails if the glob ever matches nothing.
const IPC_API_SOURCES = import.meta.glob('../ipc/types/ipc-api*.ts', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

/** Members expected per file — a FLOOR, so the parser going blind to a
 *  declaration form shows up here rather than cancelling out silently. Counts at
 *  the time of writing: 172 / 20 / 7 / 2. */
const MEMBER_FLOORS: ReadonlyArray<readonly [string, number]> = [
  ['ipc-api.ts', 150],
  ['ipc-api-forge.ts', 15],
  ['ipc-api-obs.ts', 5],
  ['ipc-api-tools.ts', 2],
];

/** `name:` / `name?:` at the head of one parameter segment. */
const NAMED_PARAM = /^\s*([A-Za-z_$][\w$]*)\s*\??\s*:/;

/** Strips block and line comments so doc text can never look like a parameter. */
function stripComments(src: string): string {
  return src.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
}

/**
 * Parameter names, in order, of every 2-space-indented member of an interface
 * body — both the method shorthand (`foo(a: T): R;`) and the property form
 * (`foo: (a: T) => R;`). Depth-tracks `([{<` so a nested callback type or a
 * generic argument never splits a parameter list (`=>` is not a closing angle).
 */
function signaturesIn(source: string): Map<string, string[]> {
  const src = stripComments(source);
  const out = new Map<string, string[]>();
  const member = /^ {2}([a-z][A-Za-z0-9]*)\s*(?:\(|:\s*\()/gm;
  for (let m = member.exec(src); m !== null; m = member.exec(src)) {
    const open = src.indexOf('(', m.index + m[1].length);
    if (open < 0) continue;
    const params: string[] = [];
    let depth = 1;
    let segment = '';
    for (let i = open + 1; i < src.length && depth > 0; i += 1) {
      const c = src[i];
      if (c === '(' || c === '[' || c === '{' || c === '<') depth += 1;
      else if (c === ')' || c === ']' || c === '}' || (c === '>' && src[i - 1] !== '=')) depth -= 1;
      if (depth === 0) break;
      if (c === ',' && depth === 1) {
        params.push(segment);
        segment = '';
      } else segment += c;
    }
    params.push(segment);
    // A zero-parameter member, and the trailing comma of a multi-line signature,
    // both leave a blank final segment — dropped so indices stay positional.
    while (params.length > 0 && params[params.length - 1].trim() === '') params.pop();
    out.set(
      m[1],
      params.map((p) => NAMED_PARAM.exec(p)?.[1] ?? ''),
    );
  }
  return out;
}

function declaredSignatures(): Map<string, string[]> {
  const all = new Map<string, string[]>();
  for (const [path, src] of Object.entries(IPC_API_SOURCES)) {
    const floor = MEMBER_FLOORS.find(([suffix]) => path.endsWith(suffix))?.[1] ?? 1;
    const found = signaturesIn(src);
    expect(found.size, `too few IpcApi members parsed from ${path}`).toBeGreaterThanOrEqual(floor);
    for (const [name, params] of found) all.set(name, params);
  }
  return all;
}

describe('A26 positional drift guard', () => {
  it('names position i by the parameter actually declared at position i', () => {
    const declared = declaredSignatures();
    const mismatches: string[] = [];
    for (const [cmd, row] of Object.entries(POLICY)) {
      const params = declared.get(cmd);
      if (params === undefined) {
        mismatches.push(`${cmd}: not declared on IpcApi`);
        continue;
      }
      row.forEach((name, i) => {
        if (name === null) return;
        if (params[i] !== name) {
          mismatches.push(`${cmd}[${i}]: policy "${name}" vs declared "${params[i] ?? '<none>'}"`);
        }
      });
    }
    expect(mismatches).toEqual([]);
  });

  it('parses the parameter names it claims to (parser self-check)', () => {
    expect(Object.keys(IPC_API_SOURCES).length).toBe(MEMBER_FLOORS.length);
    const declared = declaredSignatures();
    expect(declared.get('commit')).toEqual(['repoId', 'message', 'sign', 'skipHooks']);
    expect(declared.get('cloneRepo')).toEqual(['url', 'dest', 'onProgress']);
    expect(declared.get('getCommitDiff')).toEqual(['repoId', 'oid']);
    // Both declaration forms, and neither a doc comment nor a non-callable
    // property: a property-style member must not be invisible to this guard.
    const synthetic = signaturesIn(
      [
        'export interface IpcApi {',
        '  /** doc(with, parens) */',
        '  shorthand(repoId: string, onProgress: (p: P) => void): Promise<void>;',
        '  propStyle: (repoId: string, oid: string) => Promise<void>;',
        '  notCallable: string;',
        '  Uppercase(): void;',
        '}',
      ].join('\n'),
    );
    expect([...synthetic.keys()]).toEqual(['shorthand', 'propStyle']);
    expect(synthetic.get('shorthand')).toEqual(['repoId', 'onProgress']);
    expect(synthetic.get('propStyle')).toEqual(['repoId', 'oid']);
  });
});
