import { lazy, Suspense, useState } from 'react';
import type { ConflictFile, FileStatus, ImageDiff, LineSelection } from '../ipc';
import { DiffSlotView } from './DiffView';
import type { DiffSlot } from './DiffView';
import { DiffImageView } from './DiffImageView';
import type { ImageMode } from './DiffImageView';
import { ErrorBoundary } from './ErrorBoundary';
import { SummarizeIcon } from './menuIcons';
import { detectLanguage } from '../utils/language';
import { isImagePath } from '../utils/imagePaths';

// Lazy so CodeMirror is code-split out of the main bundle — it must not load
// until a text-mergeable conflict is actually opened (P12b SHOULD-FIX).
const ConflictEditor = lazy(() => import('./ConflictEditor'));

// P3a §2.2: full-pane diff overlay over the center graph pane. Purely
// presentational — App owns the slot state, meta derivation, and Esc handling.

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'A',
  conflicted: 'C',
};

// P93: `pr` is deliberately absent — its chip is computed from the PR number
// (the one kind whose label is not a static string), see `kindChip` below.
const KIND_LABEL: Record<Exclude<DiffOverlayMeta['kind'], 'pr'>, string> = {
  staged: 'Staged',
  unstaged: 'Unstaged',
  untracked: 'Untracked',
  commit: 'Commit',
  conflict: 'Conflict',
  compare: 'Compare',
  aiProposal: 'AI proposal',
};

/** Display metadata for the overlay header, derived by App (P3a §2.3) from the
 * slot key + the current snapshot/commitDiff. Never stored — recomputed each
 * render so it can't go stale relative to the data that produced the slot. */
export interface DiffOverlayMeta {
  path: string;
  /** Rename: header shows "orig → path". */
  origPath: string | null;
  /** null = lookup failed (P3a §2.3 fallback): no badge. */
  status: FileStatus | null;
  /** Drives the header context label. `aiProposal` (P13 §8.3) reuses the
   *  conflict editor, seeded with the AI-proposed markerless body. */
  kind:
    | 'staged'
    | 'unstaged'
    | 'untracked'
    | 'commit'
    | 'conflict'
    | 'compare'
    | 'aiProposal'
    /** P93: one changed file of a pull request, base…head (slot key
     *  `pr:<baseOid>:<headOid>:<path>`). Read-only. */
    | 'pr';
}

// P3c §8.3 (locked): the marker view is a plain highlighted <pre>, NOT
// DiffView — the marker text is one file body, not hunks.
const MARKER_RE = /^(<{7}|={7}|>{7})/;

function ConflictMarkerView({ file }: { file: ConflictFile }) {
  if (file.binary) return <div className="diff-placeholder">Binary file</div>;
  if (file.tooLarge) return <div className="diff-placeholder">File too large to display</div>;
  if (file.missing) return <div className="diff-placeholder">File was deleted</div>;
  return (
    <pre className="conflict-view">
      {file.text.split('\n').map((line, i) => (
        <div
          key={i}
          className={MARKER_RE.test(line) ? 'conflict-line conflict-marker-line' : 'conflict-line'}
        >
          {line === '' ? ' ' : line}
        </div>
      ))}
    </pre>
  );
}

/** True when a conflict file is a text-merge kind the rich editor handles
 * (P12 §5). Every other kind / suppressed payload keeps ConflictMarkerView. */
function isTextMergeable(file: ConflictFile): boolean {
  return (
    (file.kind === 'bothModified' || file.kind === 'bothAdded') &&
    !file.binary &&
    !file.tooLarge &&
    !file.missing
  );
}

/** Loading / error / ready body for a `conflict:<path>` slot — same state
 * recipe as DiffSlotView but rendering either the rich ConflictEditor (text
 * kinds, P12) or the read-only ConflictMarkerView (every other kind). */
function ConflictSlotView({
  slot,
  onDismissError,
  onClose,
  onResolveConflictText,
  mutating,
}: {
  slot: DiffSlot;
  onDismissError(): void;
  onClose(): void;
  onResolveConflictText(path: string, content: string): Promise<void>;
  mutating: boolean;
}) {
  const file = slot.conflict ?? null;
  if (slot.state === 'loading' && file === null) {
    return (
      <div className="diff-slot-loading skeleton-group" aria-hidden="true">
        {Array.from({ length: 3 }, (_, i) => (
          <div key={i} className="skeleton-row" />
        ))}
      </div>
    );
  }
  if (slot.state === 'error') {
    return (
      <div className="error-banner error-banner-dismissible diff-slot-error" role="alert">
        <span className="error-banner-text">{slot.error}</span>
        <button
          type="button"
          className="error-dismiss"
          aria-label="Dismiss error"
          onClick={onDismissError}
        >
          {'×'}
        </button>
      </div>
    );
  }
  if (file === null) return null;
  if (isTextMergeable(file)) {
    return (
      <Suspense
        fallback={
          <div className="diff-slot-loading skeleton-group" aria-hidden="true">
            {Array.from({ length: 3 }, (_, i) => (
              <div key={i} className="skeleton-row" />
            ))}
          </div>
        }
      >
        <ConflictEditor
          file={file}
          onResolve={onResolveConflictText}
          onCancel={onClose}
          mutating={mutating}
        />
      </Suspense>
    );
  }
  return (
    <div className={slot.state === 'loading' ? 'diff-scroll diff-stale' : 'diff-scroll'}>
      <ConflictMarkerView file={file} />
    </div>
  );
}

/** P61b: loading / error / ready body for an image slot — the same state recipe
 *  as DiffSlotView, rendering DiffImageView once the ImageDiff has loaded. */
function ImageDiffBody({
  loading,
  error,
  diff,
  mode,
  onDismissError,
}: {
  loading: boolean;
  error: string | null;
  diff: ImageDiff | null;
  mode: ImageMode;
  onDismissError(): void;
}) {
  if (error !== null) {
    return (
      <div className="error-banner error-banner-dismissible diff-slot-error" role="alert">
        <span className="error-banner-text">{error}</span>
        <button
          type="button"
          className="error-dismiss"
          aria-label="Dismiss error"
          onClick={onDismissError}
        >
          {'×'}
        </button>
      </div>
    );
  }
  if (diff === null) {
    // Loading (or a transient null before the first fetch resolves).
    return (
      <div className="diff-slot-loading skeleton-group" aria-hidden="true">
        {Array.from({ length: 3 }, (_, i) => (
          <div key={i} className="skeleton-row" />
        ))}
      </div>
    );
  }
  return (
    <div className={loading ? 'diff-stale' : undefined}>
      <DiffImageView diff={diff} mode={mode} />
    </div>
  );
}

export interface DiffOverlayProps {
  /** Non-null by construction — App only mounts the overlay when a slot is open. */
  slot: DiffSlot;
  meta: DiffOverlayMeta;
  /** × button AND error-banner dismiss both call this. */
  onClose(): void;
  /** P12 §2.3: stage user-authored resolved text (text-kind conflict slots). */
  onResolveConflictText(path: string, content: string): Promise<void>;
  /** RepoWorkspace busy flag — disables the editor's Stage-resolved button. */
  mutating: boolean;
  /** P15b: request an AI explanation of THIS file diff (workdir kinds only).
   *  `undefined` hides the "✨ Explain" action (AI ineligible or a non-workdir
   *  slot kind). App owns the aiAnalyzeDiff call + AiOutputPanel state. */
  onExplain?(): void;
  /** P17c: File/Diff/Split toggle state (available for ALL kinds). */
  viewMode: 'diff' | 'file' | 'split';
  onSetViewMode(m: 'diff' | 'file' | 'split'): void;
  /** P61a: "Highlight changes" (word-level intraline emphasis) toggle. Flipping
   *  it refetches the open slot with the new `intraline` flag (App owns that). */
  intraline: boolean;
  onSetIntraline(v: boolean): void;
  /** P61b: image-diff data for the open slot when its path is an image (D4).
   *  App fetches getImageDiff in parallel with the slot; when the path is an
   *  image the overlay swaps the Diff/File/Split group for an image-mode
   *  switcher and renders DiffImageView instead of DiffSlotView. */
  imageDiff?: ImageDiff | null;
  imageLoading?: boolean;
  imageError?: string | null;
  /** P17c: partial-staging direction (null = read-only). App derives this from
   *  the slot kind + loaded diff; forwarded to the DiffSlotView branch ONLY. */
  stageable: null | 'stage' | 'unstage';
  onStageLines(selection: LineSelection[]): void;
  onStageHunk(hunkIndex: number): void;
  /** P28: set ONLY for unstaged tracked diffs (never untracked/staged/commit);
   *  forwarded to the DiffSlotView branch only. */
  onDiscardHunk?(hunkIndex: number): void;
  /** P45: set ONLY for unstaged tracked diffs (same gate as onDiscardHunk);
   *  forwarded to the DiffSlotView branch only. */
  onDiscardLines?(selection: LineSelection[]): void;
  /** P93: PR number for the `pr` kind's computed header chip. null/undefined =>
   *  the generic `Pull request` fallback. Context, not file identity — hence a
   *  prop and not part of DiffOverlayMeta. */
  prNumber?: number | null;
}

export function DiffOverlay({
  slot,
  meta,
  onClose,
  onResolveConflictText,
  mutating,
  onExplain,
  viewMode,
  onSetViewMode,
  stageable,
  onStageLines,
  onStageHunk,
  onDiscardHunk,
  onDiscardLines,
  intraline,
  onSetIntraline,
  imageDiff = null,
  imageLoading = false,
  imageError = null,
  prNumber = null,
}: DiffOverlayProps) {
  const lang = detectLanguage(meta.path);
  // P61b: image slots (D4) replace the text diff with DiffImageView. Never for
  // conflict/ai-proposal slots (those are the CodeMirror editor). SVG is a text
  // diff (excluded by isImagePath). `imageMode` is the switcher's local state.
  // P93 §4.3: `pr` is excluded too — the image-diff effect only serves workdir
  // kinds, so a PR image slot would render a permanently empty image pane.
  const isImage =
    meta.kind !== 'conflict' &&
    meta.kind !== 'aiProposal' &&
    meta.kind !== 'pr' &&
    isImagePath(meta.path);
  const [imageMode, setImageMode] = useState<ImageMode>('sideBySide');
  return (
    <div className="diff-overlay" role="region" aria-label={`Diff: ${meta.path}`}>
      <div className="diff-overlay-header">
        {meta.status !== null && <span className="file-badge mono">{BADGES[meta.status]}</span>}
        {meta.origPath !== null ? (
          <span
            className="diff-overlay-path mono file-rename"
            title={`${meta.origPath} → ${meta.path}`}
          >
            {meta.origPath} {'→'} {meta.path}
          </span>
        ) : (
          <span className="diff-overlay-path mono" title={meta.path}>
            {meta.path}
          </span>
        )}
        {lang !== null && (
          <span className="lang-chip" data-lang={lang.id}>{lang.label}</span>
        )}
        {meta.kind === 'pr' ? (
          <span
            className="diff-overlay-kind"
            title={
              prNumber !== null && prNumber !== undefined
                ? `Diff against the merge base of pull request #${prNumber}`
                : "Diff against the pull request's merge base"
            }
          >
            {prNumber !== null && prNumber !== undefined ? `PR #${prNumber}` : 'Pull request'}
          </span>
        ) : (
          <span className="diff-overlay-kind">{KIND_LABEL[meta.kind]}</span>
        )}
        {onExplain !== undefined && (
          <button
            type="button"
            className="btn-secondary diff-explain-button"
            title="Explain this change with AI"
            onClick={onExplain}
          >
            <SummarizeIcon />
            <span>Explain</span>
          </button>
        )}
        {/* File/Diff/Split does nothing for the conflict/proposal CodeMirror
            editor (it has its own Unified/Side-by-side toggle) — hide it there.
            P61b: an image slot shows the image-mode switcher instead. */}
        {meta.kind !== 'conflict' && meta.kind !== 'aiProposal' && (
          isImage ? (
            <div className="diff-view-toggle" role="group" aria-label="Image compare mode">
              <button
                type="button"
                className={imageMode === 'sideBySide' ? 'active' : ''}
                aria-pressed={imageMode === 'sideBySide'}
                onClick={() => setImageMode('sideBySide')}
              >
                Side-by-side
              </button>
              <button
                type="button"
                className={imageMode === 'onion' ? 'active' : ''}
                aria-pressed={imageMode === 'onion'}
                onClick={() => setImageMode('onion')}
              >
                Onion
              </button>
              <button
                type="button"
                className={imageMode === 'swipe' ? 'active' : ''}
                aria-pressed={imageMode === 'swipe'}
                onClick={() => setImageMode('swipe')}
              >
                Swipe
              </button>
            </div>
          ) : (
            <>
              <div className="diff-view-toggle" role="group" aria-label="View mode">
                <button
                  type="button"
                  className={viewMode === 'file' ? 'active' : ''}
                  aria-pressed={viewMode === 'file'}
                  onClick={() => onSetViewMode('file')}
                >
                  File
                </button>
                <button
                  type="button"
                  className={viewMode === 'diff' ? 'active' : ''}
                  aria-pressed={viewMode === 'diff'}
                  onClick={() => onSetViewMode('diff')}
                >
                  Diff
                </button>
                <button
                  type="button"
                  className={viewMode === 'split' ? 'active' : ''}
                  aria-pressed={viewMode === 'split'}
                  onClick={() => onSetViewMode('split')}
                >
                  Split
                </button>
              </div>
              <button
                type="button"
                className={`diff-intra-toggle${intraline ? ' active' : ''}`}
                aria-pressed={intraline}
                title="Highlight the changed words within each modified line"
                onClick={() => onSetIntraline(!intraline)}
              >
                {'Highlight changes'}
              </button>
            </>
          )
        )}
        <button
          type="button"
          className="btn-icon diff-overlay-close"
          aria-label="Close diff"
          title="Close (Esc)"
          onClick={onClose}
        >
          {'×'}
        </button>
      </div>
      <div className="diff-overlay-body">
        {/* T0.4: contain a render throw to the overlay body so the header (and
            its Close button) stays usable and the rest of the app survives. */}
        {meta.kind === 'conflict' || meta.kind === 'aiProposal' ? (
          <ErrorBoundary label="Conflict editor">
            {/* Keyed by kind+path so switching a `conflict:` slot to an
                `ai-proposal:` slot for the SAME path remounts the editor and
                reseeds from the proposed body (its reseed guard keys on path only). */}
            <ConflictSlotView
              key={`${meta.kind}:${meta.path}`}
              slot={slot}
              onDismissError={onClose}
              onClose={onClose}
              onResolveConflictText={onResolveConflictText}
              mutating={mutating}
            />
          </ErrorBoundary>
        ) : isImage ? (
          <ErrorBoundary label="Image diff">
            <ImageDiffBody
              loading={imageLoading}
              error={imageError}
              diff={imageDiff}
              mode={imageMode}
              onDismissError={onClose}
            />
          </ErrorBoundary>
        ) : (
          <ErrorBoundary label="Diff view">
            <DiffSlotView
              slot={slot}
              onDismissError={onClose}
              viewMode={viewMode}
              intraline={intraline}
              stageable={stageable}
              onStageLines={onStageLines}
              onStageHunk={onStageHunk}
              onDiscardHunk={onDiscardHunk}
              onDiscardLines={onDiscardLines}
            />
          </ErrorBoundary>
        )}
      </div>
    </div>
  );
}
