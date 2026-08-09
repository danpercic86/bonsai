/** T3.4 — search.ts (filtering/caps/#fail), history.ts (build → status →
 *  retrieve → AI answer; module-level built flag, so ordering matters here),
 *  signing.ts (?sign= live seam + deterministic verify verdicts). */
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { searchHandlers } from './search';
import { historyHandlers } from './history';
import { signingHandlers } from './signing';
import { diffHandlers } from './diff';
import type { IndexProgress, SearchQuery } from '../../types';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());
afterEach(() => window.history.replaceState({}, '', '/'));

async function openDefault(): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('sh')));
  return repoId;
}

function query(text: string, over: Partial<SearchQuery> = {}): SearchQuery {
  return {
    text,
    field: 'all',
    regex: false,
    caseSensitive: false,
    maxResults: 0,
    scopeRef: null,
    ...over,
  };
}

describe('searchCommits', () => {
  it('empty/whitespace text → no matches; #fail → git rejection', async () => {
    const repoId = await openDefault();
    expect(await run(searchHandlers.searchCommits(repoId, query('   ')))).toEqual({
      matches: [],
      truncated: false,
    });
    expect(
      (await runErr(searchHandlers.searchCommits(repoId, query('boom #fail')))).kind,
    ).toBe('git');
  });

  it('matches summaries case-insensitively by default; caseSensitive narrows', async () => {
    const repoId = await openDefault();
    const layout = await run(diffHandlers.getGraph(repoId));
    const summary = layout.nodes[0].summary;
    const upper = await run(searchHandlers.searchCommits(repoId, query(summary.toUpperCase())));
    expect(upper.matches.some((m) => m.oid === layout.nodes[0].id)).toBe(true);
    const strict = await run(
      searchHandlers.searchCommits(repoId, query(summary.toUpperCase(), { caseSensitive: true })),
    );
    expect(strict.matches.some((m) => m.oid === layout.nodes[0].id)).toBe(false);
  });

  it('field=author matches authors and tags the matched field', async () => {
    const repoId = await openDefault();
    const layout = await run(diffHandlers.getGraph(repoId));
    // Stash offshoot rows carry author '' — pick a real commit row's author.
    const author = layout.nodes.find((n) => n.author !== '')?.author ?? '';
    expect(author).not.toBe('');
    const results = await run(
      searchHandlers.searchCommits(repoId, query(author, { field: 'author' })),
    );
    expect(results.matches.length).toBeGreaterThan(0);
    expect(results.matches.every((m) => m.matched === 'author')).toBe(true);
  });

  it('maxResults caps the list and reports truncated', async () => {
    const repoId = await openDefault();
    const layout = await run(diffHandlers.getGraph(repoId));
    const author = layout.nodes.find((n) => n.author !== '')?.author ?? ''; // matches many rows
    const capped = await run(
      searchHandlers.searchCommits(repoId, query(author, { maxResults: 1 })),
    );
    expect(capped.matches).toHaveLength(1);
    expect(capped.truncated).toBe(true);
  });
});

// NB: `mockBuilt` is module-level in history.ts — these run in declaration
// order: pre-build assertions FIRST, then the build, then post-build reads.
describe('history index lifecycle (order-dependent)', () => {
  it('before a build: status not built; search returns the stale hint', async () => {
    const repoId = await openDefault();
    expect(await run(historyHandlers.historyIndexStatus(repoId))).toMatchObject({
      built: false,
      indexedCommits: 0,
      headOid: null,
      builtAt: null,
    });
    expect(
      await run(historyHandlers.historySearch(repoId, { text: 'core', topK: 0 })),
    ).toEqual({ hits: [], indexStale: true, indexedCommits: 0 });
    const err = await runErr(historyHandlers.aiSearchHistory(repoId, 'core work', 5));
    expect(err.kind).toBe('aiFailed');
    expect(err.message).toContain('not built');
  });

  it('build streams monotonic progress phases and flips the status', async () => {
    const repoId = await openDefault();
    const phases: IndexProgress['phase'][] = [];
    let lastProcessed = -1;
    let monotonic = true;
    const status = await run(
      historyHandlers.historyIndexBuild(repoId, (p) => {
        phases.push(p.phase);
        if (p.processed < lastProcessed) monotonic = false;
        lastProcessed = p.processed;
      }),
    );
    expect(phases[0]).toBe('counting');
    expect(phases[phases.length - 1]).toBe('done');
    expect(phases).toContain('extracting');
    expect(phases).toContain('writing');
    expect(monotonic).toBe(true);
    expect(status.built).toBe(true);
    expect(status.indexedCommits).toBeGreaterThan(0);
    expect(await run(historyHandlers.historyIndexStatus(repoId))).toMatchObject({
      built: true,
      stale: false,
    });
  });

  it('after the build: retrieval ranks hits with strictly-descending scores', async () => {
    const repoId = await openDefault();
    const layout = await run(diffHandlers.getGraph(repoId));
    const results = await run(
      historyHandlers.historySearch(repoId, { text: layout.nodes[0].summary, topK: 0 }),
    );
    expect(results.indexStale).toBe(false);
    expect(results.hits.length).toBeGreaterThan(0);
    const scores = results.hits.map((h) => h.score);
    expect(scores).toEqual([...scores].sort((a, b) => b - a));
    // topK clamps.
    const one = await run(
      historyHandlers.historySearch(repoId, { text: layout.nodes[0].summary, topK: 1 }),
    );
    expect(one.hits).toHaveLength(1);
  });

  it('aiSearchHistory: no relevant commits → aiFailed; hits → grounded answer', async () => {
    const repoId = await openDefault();
    const miss = await runErr(historyHandlers.aiSearchHistory(repoId, 'zzzqqqxxx', 5));
    expect(miss.kind).toBe('aiFailed');
    const layout = await run(diffHandlers.getGraph(repoId));
    const answer = await run(
      historyHandlers.aiSearchHistory(repoId, layout.nodes[0].summary, 5),
    );
    expect(answer.retrieved.length).toBeGreaterThan(0);
    expect(answer.cited.length).toBeGreaterThan(0);
    const retrievedShorts = new Set(answer.retrieved.map((h) => h.oid.slice(0, 7)));
    for (const c of answer.cited) expect(retrievedShorts.has(c)).toBe(true);
  });
});

describe('signing', () => {
  it('default: signing disabled; ?sign=ssh / gpg flip it live (query read per call)', async () => {
    const repoId = await openDefault();
    expect(await run(signingHandlers.signingStatus(repoId))).toEqual({
      enabled: false,
      format: null,
      hasKey: false,
    });
    window.history.replaceState({}, '', '/?sign=ssh');
    expect(await run(signingHandlers.signingStatus(repoId))).toMatchObject({
      enabled: true,
      format: 'ssh',
      hasKey: true,
    });
    window.history.replaceState({}, '', '/?sign=openpgp');
    expect(await run(signingHandlers.signingStatus(repoId))).toMatchObject({
      format: 'openpgp',
    });
  });

  it('verifyCommits: deterministic per-oid verdicts, unknown oids omitted', async () => {
    const repoId = await openDefault();
    const layout = await run(diffHandlers.getGraph(repoId));
    const known = layout.nodes.slice(0, 5).map((n) => n.id);
    const results = await run(
      signingHandlers.verifyCommits(repoId, [...known, 'f0'.repeat(20)]),
    );
    expect(results.verifications.map((v) => v.oid)).toEqual(known); // unknown dropped
    for (const v of results.verifications) {
      if (v.status === 'unsigned') {
        expect('signer' in v).toBe(false);
      } else {
        expect(v.signer).toContain('Ada');
      }
    }
    // Determinism: a second call yields identical verdicts.
    const again = await run(signingHandlers.verifyCommits(repoId, known));
    expect(again).toEqual({
      verifications: results.verifications.filter((v) => known.includes(v.oid)),
    });
  });
});
