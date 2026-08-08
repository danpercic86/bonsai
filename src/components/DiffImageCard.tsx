import { useEffect, useRef, useState } from 'react';
import { ipc } from '../ipc';
import type { FileDiffHeader, ImageDiff, ImageDiffRequest } from '../ipc';
import { errorMessage } from '../utils/errors';
import { SkeletonRows } from './CommitPanel';
import { DiffImageView } from './DiffImageView';
import type { ImageMode } from './DiffImageView';
import type { DiffBrowserSource } from './DiffBrowser';

// P61b (SHOULD-FIX follow-up): the commit/compare counterpart to the workdir
// image overlay (RepoWorkspace → DiffOverlay). DiffBrowser streams text files
// through its bounded FileDiff queue; image headers are `binary:true` and so are
// excluded from that queue — this card instead does its OWN local getImageDiff
// fetch, the same documented local-fetch exception DiffBrowser already relies on
// (§8.4). There is intentionally NO shared image cache: collapsing a card
// unmounts it (like the text card), so re-expanding refetches. The fetch effect
// mirrors the workdir image effect in RepoWorkspace (reqId race guard,
// loading/error state, refetch on context change).

export interface DiffImageCardProps {
  repoId: string;
  /** The browser's active source; `mode` selects the commit-vs-compare request. */
  source: DiffBrowserSource;
  /** Header for THIS image file (path + origPath for renames). */
  header: FileDiffHeader;
}

export function DiffImageCard({ repoId, source, header }: DiffImageCardProps) {
  const [diff, setDiff] = useState<ImageDiff | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<ImageMode>('sideBySide');
  // Retry bumps this so the fetch effect re-runs (which bumps reqId + refetches).
  const [retryTick, setRetryTick] = useState(0);
  // Race guard: only the newest in-flight request may write state (drop stale).
  const reqIdRef = useRef(0);

  useEffect(() => {
    // Build the request from the browser's context. Workdir never reaches here
    // (that overlay lives in RepoWorkspace). For `compare`, source.oid is the
    // "to" commit — HEAD is the implicit "from" (matches compareWithHeadFileDiff).
    const request: ImageDiffRequest =
      source.mode === 'commit'
        ? { kind: 'commit', oid: source.oid, path: header.path, origPath: header.origPath }
        : { kind: 'compare', toOid: source.oid, path: header.path, origPath: header.origPath };
    const id = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    void ipc.getImageDiff(repoId, request).then(
      (d) => {
        if (id !== reqIdRef.current) return; // superseded by a newer request
        setDiff(d);
        setLoading(false);
      },
      (e: unknown) => {
        if (id !== reqIdRef.current) return;
        setDiff(null);
        setError(errorMessage(e));
        setLoading(false);
      },
    );
  }, [repoId, source.oid, source.mode, header.path, header.origPath, retryTick]);

  return (
    <div className="diff-image-card">
      <div className="diff-image-card-toolbar">
        <div className="diff-view-toggle" role="group" aria-label="Image compare mode">
          <button
            type="button"
            className={mode === 'sideBySide' ? 'active' : ''}
            aria-pressed={mode === 'sideBySide'}
            onClick={() => setMode('sideBySide')}
          >
            Side by side
          </button>
          <button
            type="button"
            className={mode === 'onion' ? 'active' : ''}
            aria-pressed={mode === 'onion'}
            onClick={() => setMode('onion')}
          >
            Onion
          </button>
          <button
            type="button"
            className={mode === 'swipe' ? 'active' : ''}
            aria-pressed={mode === 'swipe'}
            onClick={() => setMode('swipe')}
          >
            Swipe
          </button>
        </div>
      </div>
      <DiffImageCardBody
        loading={loading}
        error={error}
        diff={diff}
        mode={mode}
        onRetry={() => setRetryTick((t) => t + 1)}
      />
    </div>
  );
}

/** loading / error / ready body — same state recipe as DiffCardBody (:414-433). */
function DiffImageCardBody({
  loading,
  error,
  diff,
  mode,
  onRetry,
}: {
  loading: boolean;
  error: string | null;
  diff: ImageDiff | null;
  mode: ImageMode;
  onRetry(): void;
}) {
  if (loading) {
    return (
      <div className="diff-card-loading skeleton-group" aria-hidden="true">
        <SkeletonRows />
      </div>
    );
  }
  if (error !== null) {
    return (
      <div className="error-banner error-banner-dismissible diff-slot-error" role="alert">
        <span className="error-banner-text">{error}</span>
        <button type="button" className="section-action" onClick={onRetry}>
          Retry
        </button>
      </div>
    );
  }
  if (diff === null) return null; // settled with no payload (never in practice)
  return <DiffImageView diff={diff} mode={mode} />;
}
