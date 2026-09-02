import { useEffect, useRef } from 'react';
import type { FileDiffHeader, PrDiffStats } from '../../ipc';
import { SkeletonRows } from '../CommitPanel';
import type { PrRestoreFocus } from '../repoWorkspace/usePrFileOverlay';
import { PrFileRow } from './PrFileRow';
import type { PrDiffErrorCause, PrDiffStatus } from './usePrDiff';

// P89/P93: the PR detail's changed-files section. A small state machine over the
// forgePrDiff result: loading skeleton, empty, error+retry, or the <ul> of file
// rows. Since P93 a row click opens the file's diff in the CENTER OVERLAY (never
// inline), so this component owns no per-file fetch state — only the "which row
// is open" marker, which is passed in as `activePath`.

const ERROR_COPY: Record<PrDiffErrorCause, string> = {
  network:
    "Couldn't reach the remote to fetch this pull request. Check your connection and retry.",
  auth: 'Sign-in required to fetch this pull request.',
  unresolved: "Couldn't resolve this pull request's base or head commit.",
  rateLimited: 'Rate limited by the forge. Try again in a moment.',
  generic: "Couldn't compute this pull request's diff.",
};

/** P93 §3: the click payload that travels up to RepoWorkspace, which owns the
 *  overlay slot. The overlay key alone cannot carry the header status, the
 *  rename origin or the PR number, so they ride along here. */
export interface PrFileDiffOpen {
  prNumber: number;
  /** `prDiff.stats.mergeBaseOid` — the three-dot base. */
  baseOid: string;
  /** `prDiff.stats.headOid`. */
  headOid: string;
  header: FileDiffHeader;
}

export interface PrChangesSectionProps {
  status: PrDiffStatus;
  stats: PrDiffStats | null;
  stale: boolean;
  errorCause: PrDiffErrorCause;
  /** Re-run forgePrDiff for this PR. */
  onRetry(): void;
  /** Path of the file whose diff is open in the center overlay (null = none). */
  activePath: string | null;
  /** P93 §6.1: set ONLY when the user dismissed this PR file's overlay. `token`
   *  changes on every dismissal so a repeat close of the same row still fires.
   *  Never derived from an `activePath` transition — that cannot tell a user
   *  dismissal apart from a slot replacement / PR switch / head advance. */
  restoreFocusTo: PrRestoreFocus | null;
  onOpenFile(header: FileDiffHeader): void;
}

export function PrChangesSection({
  status,
  stats,
  stale,
  errorCause,
  onRetry,
  activePath,
  restoreFocusTo,
  onOpenFile,
}: PrChangesSectionProps) {
  const files = stats?.files ?? [];
  const count = stats?.changedFiles ?? files.length;

  // P93 §6.1 focus restore, driven by the DISMISSAL EVENT (`restoreFocusTo`), not
  // by an `activePath` transition. Fires once per token, and only when all three
  // guards hold: the row still exists, it has a <button> (binary rows do not),
  // and focus has fallen back to <body> — if the user has since focused the graph
  // or the sidebar we must not take it back.
  //
  // P96: the row is resolved by its `data-path` attribute, not by a positional
  // index into `children`, so a re-order or an added wrapper element can't send
  // focus to the wrong row. Paths may contain quotes/backslashes, so nothing is
  // interpolated into a selector — the attribute values are compared directly.
  // `:scope >` keeps "a row, not a descendant" structural rather than relying on
  // the convention that nothing nested carries `data-path`.
  const listRef = useRef<HTMLUListElement>(null);
  const lastTokenRef = useRef<number | null>(restoreFocusTo?.token ?? null);
  useEffect(() => {
    if (restoreFocusTo === null) return;
    if (lastTokenRef.current === restoreFocusTo.token) return;
    lastTokenRef.current = restoreFocusTo.token;
    const active = document.activeElement;
    if (active !== null && active !== document.body) return;
    const rows = listRef.current?.querySelectorAll<HTMLElement>(':scope > [data-path]') ?? [];
    for (const row of rows) {
      if (row.dataset.path !== restoreFocusTo.path) continue;
      row.querySelector('button')?.focus();
      return;
    }
  }, [restoreFocusTo]);

  // SF2: on a head-advance refetch the hook keeps the prior stats and sets
  // `stale`. Keep rendering the existing rows underneath, dimmed via
  // `.diff-stale`, instead of collapsing to a skeleton — only the FIRST load
  // (no prior rows) shows the bare skeleton.
  const showStaleRows = status === 'loading' && stale && files.length > 0;
  const fileList = (
    <ul ref={listRef} className={`pr-changes-list${stale ? ' diff-stale' : ''}`}>
      {files.map((f) => (
        <PrFileRow
          key={f.path}
          header={f}
          active={activePath === f.path}
          onOpen={onOpenFile}
        />
      ))}
    </ul>
  );

  return (
    <section className="pr-changes" role="region" aria-label="Changed files">
      <div className="pr-changes-head">
        <span className="pr-changes-label">Changed files</span>
        {(status === 'ready' || status === 'empty') && (
          <span className="pr-changes-count">
            {`${count} file${count === 1 ? '' : 's'}`}
          </span>
        )}
        {status === 'error' && (
          <button type="button" className="section-action pr-changes-retry" onClick={onRetry}>
            Retry
          </button>
        )}
      </div>

      {status === 'loading' &&
        (showStaleRows ? (
          <>
            <p className="pane-empty pr-changes-loading">Computing diff…</p>
            {fileList}
          </>
        ) : (
          <>
            <p className="pane-empty pr-changes-loading">Computing diff…</p>
            <div className="skeleton-group" aria-hidden="true">
              <SkeletonRows />
            </div>
          </>
        ))}

      {status === 'error' && (
        <div className="error-banner" role="alert">
          <span className="error-banner-text">{ERROR_COPY[errorCause]}</span>
        </div>
      )}

      {status === 'empty' && (
        <p className="pane-empty">No changes between base and head.</p>
      )}

      {status === 'ready' && fileList}
    </section>
  );
}
