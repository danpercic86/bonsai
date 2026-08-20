// P77 tag-sync mock handlers. Behaviour-consistent with the real IPC in
// `src/ipc/tauri.ts` / `src-tauri/src/commands/tags.rs`:
//  - `listTagSync` = live ls-remote reconciliation (best-effort; may reject).
//  - `forceRefreshTag` corrects a stale local tag → the row flips to `in-sync`.
//  - `deleteRemoteTag` removes the remote side → the row becomes `local-only`
//    (or disappears entirely for a remote-only ghost, which has no local side).
// The report is held per-repo in memory so resolve ops persist across a
// subsequent `listTagSync`, mirroring the on-disk backend truth.
import type { AppError, IpcApi, TagSyncReport } from '../../types';
import { buildTagSyncReport } from '../../fixtures/tagSync';
import { delay, requireRepo, throwAuthFailed, throwNetworkError } from '../repoState';
import type { MockRepoState } from '../repoState';

// Live per-repo reports, seeded lazily from the fixture and mutated by resolve ops.
const reports = new Map<string /* repoId */, TagSyncReport>();

/** Resolve the remote to query: the caller's choice, else `origin`, else the
 *  first configured remote; NoRemote when none are configured (mirrors Rust). */
function resolveRemote(state: MockRepoState, remote: string | null): string {
  if (remote !== null && remote !== '') return remote;
  const origin = state.remotes.find((r) => r.name === 'origin');
  if (origin !== undefined) return origin.name;
  const first = state.remotes[0];
  if (first === undefined) {
    const err: AppError = { kind: 'noRemote', message: 'mock: no remote to compare against' };
    throw err;
  }
  return first.name;
}

/** The live report for `repoId`, seeded from the fixture on first access. */
function reportFor(repoId: string, remote: string): TagSyncReport {
  let report = reports.get(repoId);
  if (report === undefined) {
    report = buildTagSyncReport(remote);
    reports.set(repoId, report);
  } else {
    report.remote = remote;
  }
  return report;
}

export const tagSyncHandlers = {
  async listTagSync(repoId: string, remote: string | null): Promise<TagSyncReport> {
    // A live ls-remote round-trip — the slowest of the three.
    await delay(400);
    const state = requireRepo(repoId);
    // `?remote=` failure triggers drive the graceful-degrade path in the sidebar.
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    const name = resolveRemote(state, remote);
    return structuredClone(reportFor(repoId, name));
  },

  async forceRefreshTag(repoId: string, remote: string, tagName: string): Promise<void> {
    await delay(300);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    const report = reportFor(repoId, remote);
    const entry = report.entries.find((e) => e.name === tagName);
    if (entry !== undefined && entry.remoteOid !== null) {
      // Fast-forward the local ref onto the remote committish.
      entry.localOid = entry.remoteOid;
      entry.status = 'in-sync';
    }
  },

  async deleteRemoteTag(repoId: string, remote: string, tagName: string): Promise<void> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (state.remoteTrigger === 'rejected') {
      const err: AppError = {
        kind: 'pushRejected',
        message: `push rejected: the remote refused to delete tag '${tagName}' (protected ref?).`,
      };
      throw err;
    }
    const report = reportFor(repoId, remote);
    const entry = report.entries.find((e) => e.name === tagName);
    if (entry !== undefined) {
      entry.remoteOid = null;
      if (entry.localOid !== null) {
        // Still present locally → drops to local-only.
        entry.status = 'local-only';
      } else {
        // Remote-only ghost with no local side → the row vanishes.
        report.entries = report.entries.filter((e) => e.name !== tagName);
      }
    }
  },
} satisfies Partial<IpcApi>;

/** Mock-only: reflect a successful tag push into the live sync report so the
 *  harness's next `listTagSync` flips the row to `in-sync` — mirroring real IPC.
 *  Both push-unpushed (local-only) and force-move (stale) make the remote match
 *  local, so both resolve to `in-sync`. Seeds the report (and the entry, when
 *  absent — e.g. a freshly-created local tag) so the flip is always visible. */
export function applyTagPushToSync(repoId: string, remote: string, tagName: string): void {
  const report = reportFor(repoId, remote);
  const entry = report.entries.find((e) => e.name === tagName);
  if (entry !== undefined) {
    if (entry.localOid !== null) {
      entry.remoteOid = entry.localOid;
      entry.status = 'in-sync';
    }
    return;
  }
  // No prior verdict (a brand-new local tag): synthesize a matched, in-sync row.
  const oid = tagName.padEnd(40, '0').slice(0, 40);
  report.entries.push({
    name: tagName,
    status: 'in-sync',
    localOid: oid,
    remoteOid: oid,
    annotated: false,
  });
  report.entries.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
}

/** Mock-only: reflect a LOCAL tag delete into an existing live sync report so it
 *  stays consistent — the local side drops (→ `remote-only` if still on the
 *  remote, else the row vanishes). No-op when no live check has run yet (a local
 *  delete must not fabricate a remote verdict). */
export function applyTagDeleteLocalToSync(repoId: string, tagName: string): void {
  const report = reports.get(repoId);
  if (report === undefined) return;
  const entry = report.entries.find((e) => e.name === tagName);
  if (entry === undefined) return;
  entry.localOid = null;
  if (entry.remoteOid !== null) {
    entry.status = 'remote-only';
  } else {
    report.entries = report.entries.filter((e) => e.name !== tagName);
  }
}

// Test/harness aid: forget the cached reports so a fresh open re-seeds from the
// fixture. Not part of IpcApi; safe to leave unused in production builds.
export function __resetTagSyncMock(): void {
  reports.clear();
}
