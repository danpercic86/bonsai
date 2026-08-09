/** T3.4 — status.ts + compose.ts + config.ts state invariants: the stage /
 *  unstage / commit cycle updates status + graph + ahead coherently; partial
 *  (line-level) ops only for the model file; composer atomicity; config CRUD
 *  feeding the commit identity gate. */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { statusHandlers } from './status';
import { diffHandlers } from './diff';
import { composeHandlers } from './compose';
import { configHandlers } from './config';
import { requireRepo } from '../repoState';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());

async function openDefault(): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('sc')));
  return repoId;
}

describe('getStatus', () => {
  it('returns a fresh copy (mutations do not poison the state)', async () => {
    const repoId = await openDefault();
    const a = await run(statusHandlers.getStatus(repoId));
    a.staged.length = 0;
    const b = await run(statusHandlers.getStatus(repoId));
    expect(b.staged.length).toBeGreaterThan(0);
  });

  it('appends the model-derived src/main.rs row to BOTH sections (seeded three-way)', async () => {
    const repoId = await openDefault();
    const s = await run(statusHandlers.getStatus(repoId));
    // The seeded model has index ≠ head (staged insert) AND workdir ≠ index.
    expect(s.unstaged.some((e) => e.path === 'src/main.rs')).toBe(true);
    expect(s.staged.some((e) => e.path === 'src/main.rs')).toBe(true);
  });
});

describe('stage / unstage cycle', () => {
  it('staging an unstaged file moves it; unstaging returns it with status intact', async () => {
    const repoId = await openDefault();
    await run(statusHandlers.stage(repoId, ['README.md']));
    let s = await run(statusHandlers.getStatus(repoId));
    expect(s.staged.some((e) => e.path === 'README.md' && e.status === 'modified')).toBe(true);
    expect(s.unstaged.some((e) => e.path === 'README.md')).toBe(false);
    await run(statusHandlers.unstage(repoId, ['README.md']));
    s = await run(statusHandlers.getStatus(repoId));
    expect(s.unstaged.some((e) => e.path === 'README.md' && e.status === 'modified')).toBe(true);
  });

  it('staging an untracked file becomes "added"; unstaging returns it untracked', async () => {
    const repoId = await openDefault();
    await run(statusHandlers.stage(repoId, ['scratch.rs']));
    let s = await run(statusHandlers.getStatus(repoId));
    expect(s.staged.find((e) => e.path === 'scratch.rs')?.status).toBe('added');
    await run(statusHandlers.unstage(repoId, ['scratch.rs']));
    s = await run(statusHandlers.getStatus(repoId));
    expect(s.untracked.find((e) => e.path === 'scratch.rs')?.status).toBe('untracked');
  });

  it('matches renamed entries by origPath too', async () => {
    const repoId = await openDefault();
    await run(statusHandlers.unstage(repoId, ['docs/intro.md'])); // origPath of the rename
    const s = await run(statusHandlers.getStatus(repoId));
    expect(s.unstaged.find((e) => e.path === 'docs/getting-started.md')?.status).toBe('renamed');
  });

  it('staged lists stay path-sorted after mutations', async () => {
    const repoId = await openDefault();
    await run(statusHandlers.stage(repoId, ['README.md', 'scratch.rs']));
    const s = await run(statusHandlers.getStatus(repoId));
    const paths = s.staged.map((e) => e.path);
    expect(paths).toEqual([...paths].sort());
  });
});

describe('commit', () => {
  it('clears staged, prepends a graph node, moves HEAD, bumps ahead', async () => {
    const repoId = await openDefault();
    const before = await run(diffHandlers.getGraph(repoId));
    const result = await run(statusHandlers.commit(repoId, 'feat: t34 commit\n\nbody'));
    expect(result.summary).toBe('feat: t34 commit');
    expect(result.branch).toBe('main');
    const s = await run(statusHandlers.getStatus(repoId));
    expect(s.staged.filter((e) => e.path !== 'src/main.rs')).toEqual([]);
    const after = await run(diffHandlers.getGraph(repoId));
    expect(after.nodes.length).toBe(before.nodes.length + 1);
    expect(after.nodes.some((n) => n.id === result.oid)).toBe(true);
    const state = requireRepo(repoId);
    expect(state.headOid).toBe(result.oid);
    expect(state.branches.local.find((b) => b.name === 'main')?.ahead).toBe(1);
  });

  it('empty message → emptyMessage; empty index → nothingToCommit', async () => {
    const repoId = await openDefault();
    expect((await runErr(statusHandlers.commit(repoId, '  \n '))).kind).toBe('emptyMessage');
    const state = requireRepo(repoId);
    state.status.staged = [];
    expect((await runErr(statusHandlers.commit(repoId, 'msg'))).kind).toBe('nothingToCommit');
  });

  it('missing identity (config store) → configMissing; restoring it clears the gate', async () => {
    const repoId = await openDefault();
    await run(configHandlers.unsetConfig(repoId, 'global', 'user.name'));
    await run(configHandlers.unsetConfig(repoId, 'global', 'user.email'));
    expect((await runErr(statusHandlers.commit(repoId, 'msg'))).kind).toBe('configMissing');
    await run(configHandlers.setConfig(repoId, 'global', 'user.name', 'T'));
    await run(configHandlers.setConfig(repoId, 'global', 'user.email', 't@x.dev'));
    const result = await run(statusHandlers.commit(repoId, 'msg'));
    expect(result.oid).toHaveLength(40);
  });
});

describe('partial (line-level) staging model', () => {
  it('rejects any path other than src/main.rs', async () => {
    const repoId = await openDefault();
    expect(
      (await runErr(statusHandlers.stagePartial(repoId, 'README.md', null, []))).kind,
    ).toBe('other');
    expect(
      (await runErr(statusHandlers.discardPartial(repoId, 'README.md', null, []))).kind,
    ).toBe('other');
  });

  it('a full discard converges the workdir onto the index (row disappears)', async () => {
    const repoId = await openDefault();
    // Select EVERY changed line of the unstaged diff, then discard them all.
    const fd = await run(
      diffHandlers.getWorkdirFileDiff(repoId, 'src/main.rs', null, false, false, false),
    );
    const selection = fd.hunks.flatMap((h) =>
      h.lines
        .filter((l) => l.kind !== 'context')
        .map((l) => ({ kind: l.kind, oldNo: l.oldNo, newNo: l.newNo })),
    );
    expect(selection.length).toBeGreaterThan(0);
    await run(statusHandlers.discardPartial(repoId, 'src/main.rs', null, selection));
    const s = await run(statusHandlers.getStatus(repoId));
    expect(s.unstaged.some((e) => e.path === 'src/main.rs')).toBe(false);
  });
});

describe('applyComposedCommits (atomic composer apply)', () => {
  it('validates the whole plan first: empty plan / empty message / empty group', async () => {
    const repoId = await openDefault();
    expect(
      (await runErr(composeHandlers.applyComposedCommits(repoId, { groups: [] }))).kind,
    ).toBe('nothingToCommit');
    expect(
      (
        await runErr(
          composeHandlers.applyComposedCommits(repoId, {
            groups: [{ files: ['README.md'], message: '  ' }],
          }),
        )
      ).kind,
    ).toBe('emptyMessage');
    expect(
      (
        await runErr(
          composeHandlers.applyComposedCommits(repoId, { groups: [{ files: [], message: 'm' }] }),
        )
      ).kind,
    ).toBe('other');
  });

  it('#fail in ANY message → atomic rollback: throws and mutates NOTHING', async () => {
    const repoId = await openDefault();
    const before = structuredClone(requireRepo(repoId).status);
    const err = await runErr(
      composeHandlers.applyComposedCommits(repoId, {
        groups: [
          { files: ['README.md'], message: 'ok' },
          { files: ['scratch.rs'], message: 'boom #fail' },
        ],
      }),
    );
    expect(err.kind).toBe('git');
    expect(requireRepo(repoId).status).toEqual(before);
  });

  it('applies oldest→newest: files leave status, commits stack newest-on-top', async () => {
    const repoId = await openDefault();
    const result = await run(
      composeHandlers.applyComposedCommits(repoId, {
        groups: [
          { files: ['README.md'], message: 'first: readme' },
          { files: ['scratch.rs', 'src/main.rs'], message: 'second: code' },
        ],
      }),
    );
    expect(result.commits.map((c) => c.summary)).toEqual(['first: readme', 'second: code']);
    const state = requireRepo(repoId);
    expect(state.commits[0].summary).toBe('second: code');
    expect(state.commits[1].summary).toBe('first: readme');
    expect(state.headOid).toBe(result.commits[1].oid);
    const s = await run(statusHandlers.getStatus(repoId));
    expect(s.unstaged.some((e) => e.path === 'README.md')).toBe(false);
    expect(s.untracked.some((e) => e.path === 'scratch.rs')).toBe(false);
    // Committing the model file cleans its three-way sections too.
    expect(s.unstaged.some((e) => e.path === 'src/main.rs')).toBe(false);
    // Files in no group stay uncommitted.
    expect(s.staged.some((e) => e.path === 'src/app.rs')).toBe(true);
  });
});

describe('config handlers', () => {
  it('setConfig/unsetConfig round-trip through getConfig (trimmed; local wins)', async () => {
    const repoId = await openDefault();
    await run(configHandlers.setConfig(repoId, 'local', 'user.name', ' Ada '));
    const view = await run(configHandlers.getConfig(repoId, 'local'));
    const name = view.curated.find((c) => c.key === 'user.name');
    expect(name?.targetValue).toBe('Ada');
    expect(name?.effectiveLevel).toBe('local');
    await run(configHandlers.unsetConfig(repoId, 'local', 'user.name'));
    const after = await run(configHandlers.getConfig(repoId, 'local'));
    const name2 = after.curated.find((c) => c.key === 'user.name');
    expect(name2?.targetValue).toBeNull();
    expect(name2?.effectiveLevel).toBe('global'); // falls back to the seeded identity
  });

  it('invalid keys / enum values reject invalidName', async () => {
    const repoId = await openDefault();
    expect(
      (await runErr(configHandlers.setConfig(repoId, 'local', 'no-dot', 'v'))).kind,
    ).toBe('invalidName');
    expect(
      (await runErr(configHandlers.setConfig(repoId, 'local', 'pull.rebase', 'sideways'))).kind,
    ).toBe('invalidName');
  });

  it('applyIdentityProfile writes local identity; empty signing key leaves it untouched', async () => {
    const repoId = await openDefault();
    await run(configHandlers.setConfig(repoId, 'local', 'user.signingkey', 'OLDKEY'));
    const view = await run(
      configHandlers.applyIdentityProfile(repoId, ' Ada ', ' ada@x.dev ', '  '),
    );
    expect(view.curated.find((c) => c.key === 'user.name')?.targetValue).toBe('Ada');
    expect(view.curated.find((c) => c.key === 'user.email')?.targetValue).toBe('ada@x.dev');
    // signingkey is non-curated → shows in `advanced`; empty key left it untouched.
    expect(view.advanced.find((e) => e.name === 'user.signingkey')?.value).toBe('OLDKEY');
    const view2 = await run(
      configHandlers.applyIdentityProfile(repoId, 'A', 'a@x.dev', 'NEWKEY'),
    );
    expect(view2.advanced.find((e) => e.name === 'user.signingkey')?.value).toBe('NEWKEY');
  });
});
