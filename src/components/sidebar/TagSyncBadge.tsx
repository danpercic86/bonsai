// P77 §2: per-tag sync status pill for the sidebar Tags list. Presentational —
// reuses the existing `.submodule-badge-*` recipes (no new token/colour). Verdict
// pills carry the aria-hidden `⚠` glyph; muted pills carry the word. Returns null
// for in-sync / unknown so a clean tag stays badge-less (§2.1).
import type { TagSyncStatus } from '../../ipc';

function shortOid(oid: string | null): string {
  return oid === null ? '' : oid.slice(0, 7);
}

/** The four rendered verdicts (§2.1). `in-sync` and any unknown status render
 *  nothing. `deleted-on-remote` is reserved (never emitted in v1) but mapped so a
 *  future backend upgrade lights it without a code change. */
export function TagSyncBadge({
  status,
  tag,
  remote,
  localOid,
  remoteOid,
}: {
  status: TagSyncStatus;
  tag: string;
  remote: string;
  localOid: string | null;
  remoteOid: string | null;
}) {
  const localShort = shortOid(localOid);
  const remoteShort = shortOid(remoteOid);

  switch (status) {
    case 'local-only':
      return (
        <span
          className="branch-badge submodule-badge-muted"
          title={`Not on ${remote} yet. Right-click to push it.`}
        >
          <span>unpushed</span>
        </span>
      );
    case 'stale':
      return (
        <span
          className="branch-badge submodule-badge-warn"
          title={`Your ${tag} points to ${localShort}; ${remote} has ${remoteShort}. Right-click to resolve.`}
        >
          <span className="submodule-badge-glyph" aria-hidden="true">
            {'⚠'}
          </span>
          <span>out of sync</span>
        </span>
      );
    case 'deleted-on-remote':
      return (
        <span
          className="branch-badge submodule-badge-warn"
          title={`${tag} was removed from ${remote}. Your local copy remains.`}
        >
          <span className="submodule-badge-glyph" aria-hidden="true">
            {'⚠'}
          </span>
          <span>deleted on remote</span>
        </span>
      );
    case 'remote-only':
      return (
        <span
          className="branch-badge submodule-badge-muted"
          title={`On ${remote}, not in your repo. Right-click to create it locally.`}
        >
          <span>remote only</span>
        </span>
      );
    default:
      // in-sync (restraint — the rollup carries the all-clear by absence) and any
      // forward-compat status we don't render.
      return null;
  }
}
