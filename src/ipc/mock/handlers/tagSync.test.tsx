/** P77 — tag-sync mock: the three handlers (listTagSync / forceRefreshTag /
 *  deleteRemoteTag) and the two harness mutators (applyTagPushToSync /
 *  applyTagDeleteLocalToSync) that keep the in-memory report consistent so the
 *  next listTagSync reflects a push/local-delete — mirroring real-IPC truth. */
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import {
  __resetTagSyncMock,
  applyTagDeleteLocalToSync,
  applyTagPushToSync,
  tagSyncHandlers,
} from './tagSync';

beforeAll(() => vi.useFakeTimers());
afterAll(() => {
  vi.useRealTimers();
  window.history.replaceState({}, '', '/');
});
beforeEach(() => __resetTagSyncMock());
afterEach(() => window.history.replaceState({}, '', '/'));

async function openDefault(): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('ts')));
  return repoId;
}
async function openWithRemote(mode: string): Promise<string> {
  window.history.replaceState({}, '', `/?remote=${mode}`);
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('ts')));
  return repoId;
}

function statusOf(rep: { entries: { name: string; status: string }[] }, name: string): string | undefined {
  return rep.entries.find((e) => e.name === name)?.status;
}

describe('listTagSync handler', () => {
  it('resolves origin by default and serves every shipping status from the fixture', async () => {
    const repoId = await openDefault();
    const rep = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(rep.remote).toBe('origin');
    expect(statusOf(rep, 'v0.1.0')).toBe('in-sync');
    expect(statusOf(rep, 'v0.3.0')).toBe('local-only');
    expect(statusOf(rep, 'v1.1.0')).toBe('stale'); // the flagship moved-tag row
    expect(statusOf(rep, 'v1.2.0')).toBe('remote-only');
    // Never emits the reserved deleted-on-remote variant in v1.
    expect(rep.entries.some((e) => e.status === 'deleted-on-remote')).toBe(false);
  });

  it('returns a deep clone so a caller cannot mutate the live report', async () => {
    const repoId = await openDefault();
    const first = await run(tagSyncHandlers.listTagSync(repoId, null));
    first.entries[0].status = 'stale';
    const second = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(second.entries[0].status).toBe('in-sync');
  });

  it('rejects networkError / authFailed via the ?remote= seam (graceful-degrade path)', async () => {
    const net = await openWithRemote('network');
    expect((await runErr(tagSyncHandlers.listTagSync(net, null))).kind).toBe('networkError');
    const auth = await openWithRemote('authfail');
    expect((await runErr(tagSyncHandlers.listTagSync(auth, null))).kind).toBe('authFailed');
  });
});

describe('autoSyncTags handler (P84)', () => {
  it('adopts remote-only, moves FF-able stale, skips diverged; leaves the rest', async () => {
    const repoId = await openDefault();
    const rep = await run(tagSyncHandlers.autoSyncTags(repoId, null));
    expect(rep.remote).toBe('origin');
    // remote-only fixtures → adopted.
    expect(rep.adopted).toEqual(['v1.2.0', 'v2.0.0']);
    // FF-able stale (fixture flag) → moved.
    expect(rep.moved).toEqual(['v1.5.0']);
    // diverged stale (v1.1.0) → skipped.
    expect(rep.skippedDiverged).toEqual(['v1.1.0']);

    // The live report reflects the mutations on the next listTagSync.
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(statusOf(after, 'v1.2.0')).toBe('in-sync');
    expect(statusOf(after, 'v1.5.0')).toBe('in-sync');
    expect(statusOf(after, 'v1.1.0')).toBe('stale'); // untouched
  });

  it('returns an empty report (no throw) on auth/network — best-effort like Rust', async () => {
    const net = await run(tagSyncHandlers.autoSyncTags(await openWithRemote('network'), null));
    expect(net).toEqual({ remote: 'origin', adopted: [], moved: [], skippedDiverged: [] });
    const auth = await run(tagSyncHandlers.autoSyncTags(await openWithRemote('authfail'), null));
    expect(auth.adopted).toEqual([]);
  });
});

describe('forceRefreshTag handler', () => {
  it('flips a stale row to in-sync by fast-forwarding local onto the remote committish', async () => {
    const repoId = await openDefault();
    const before = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(statusOf(before, 'v1.1.0')).toBe('stale');

    await run(tagSyncHandlers.forceRefreshTag(repoId, 'origin', 'v1.1.0'));
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(statusOf(after, 'v1.1.0')).toBe('in-sync');
    const row = after.entries.find((e) => e.name === 'v1.1.0');
    expect(row?.localOid).toBe(row?.remoteOid);
  });
});

describe('deleteRemoteTag handler', () => {
  it('drops an in-sync tag to local-only (local side survives)', async () => {
    const repoId = await openDefault();
    await run(tagSyncHandlers.listTagSync(repoId, null)); // seed
    await run(tagSyncHandlers.deleteRemoteTag(repoId, 'origin', 'v0.1.0'));
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(statusOf(after, 'v0.1.0')).toBe('local-only');
    expect(after.entries.find((e) => e.name === 'v0.1.0')?.remoteOid).toBeNull();
  });

  it('removes a remote-only ghost row entirely (no local side to keep)', async () => {
    const repoId = await openDefault();
    await run(tagSyncHandlers.listTagSync(repoId, null)); // seed
    await run(tagSyncHandlers.deleteRemoteTag(repoId, 'origin', 'v1.2.0'));
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(after.entries.some((e) => e.name === 'v1.2.0')).toBe(false);
  });

  it('surfaces pushRejected via the ?remote=rejected seam', async () => {
    const repoId = await openWithRemote('rejected');
    const err = await runErr(tagSyncHandlers.deleteRemoteTag(repoId, 'origin', 'v0.1.0'));
    expect(err.kind).toBe('pushRejected');
  });
});

describe('applyTagPushToSync mutator', () => {
  it('flips a local-only row to in-sync (remote now matches local)', async () => {
    const repoId = await openDefault();
    await run(tagSyncHandlers.listTagSync(repoId, null)); // seed
    applyTagPushToSync(repoId, 'origin', 'v0.3.0');
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(statusOf(after, 'v0.3.0')).toBe('in-sync');
    const row = after.entries.find((e) => e.name === 'v0.3.0');
    expect(row?.remoteOid).toBe(row?.localOid);
  });

  it('flips a stale row to in-sync (force-move made remote match local)', async () => {
    const repoId = await openDefault();
    await run(tagSyncHandlers.listTagSync(repoId, null));
    applyTagPushToSync(repoId, 'origin', 'v1.1.0');
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(statusOf(after, 'v1.1.0')).toBe('in-sync');
  });

  it('synthesizes a matched in-sync row for a brand-new local tag, kept sorted', async () => {
    const repoId = await openDefault();
    await run(tagSyncHandlers.listTagSync(repoId, null));
    applyTagPushToSync(repoId, 'origin', 'v0.4.0');
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    const row = after.entries.find((e) => e.name === 'v0.4.0');
    expect(row?.status).toBe('in-sync');
    expect(row?.localOid).toBe(row?.remoteOid);
    // Case-insensitive sort preserved.
    const names = after.entries.map((e) => e.name.toLowerCase());
    expect([...names]).toEqual([...names].sort());
  });
});

describe('applyTagDeleteLocalToSync mutator', () => {
  it('drops a stale (remote-present) row to remote-only when the local side is deleted', async () => {
    const repoId = await openDefault();
    await run(tagSyncHandlers.listTagSync(repoId, null)); // seed
    applyTagDeleteLocalToSync(repoId, 'v1.1.0');
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(statusOf(after, 'v1.1.0')).toBe('remote-only');
    expect(after.entries.find((e) => e.name === 'v1.1.0')?.localOid).toBeNull();
  });

  it('removes a local-only row entirely (nothing on the remote to keep)', async () => {
    const repoId = await openDefault();
    await run(tagSyncHandlers.listTagSync(repoId, null));
    applyTagDeleteLocalToSync(repoId, 'v0.3.0');
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    expect(after.entries.some((e) => e.name === 'v0.3.0')).toBe(false);
  });

  it('is a no-op when no live check has run yet (never fabricates a verdict)', async () => {
    const repoId = await openDefault();
    // No prior listTagSync → report unseeded; the mutator must not create one.
    applyTagDeleteLocalToSync(repoId, 'v0.3.0');
    const after = await run(tagSyncHandlers.listTagSync(repoId, null));
    // The pristine fixture verdict is intact (proves nothing was fabricated).
    expect(statusOf(after, 'v0.3.0')).toBe('local-only');
  });
});
