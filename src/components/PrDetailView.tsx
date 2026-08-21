import type { MouseEvent, ReactNode } from 'react';
import type { ForgeKind, MergeMethod, PrDetail, PrState } from '../ipc';
import { PrActionsBar } from './PrActionsBar';

// P62c: presentational PR detail — header, meta, labels, mergeable, +/- stat,
// and the (markdown-ish) body, plus a slot for the comments component. No IPC.

const STATE_LABEL: Record<PrState, string> = {
  open: 'Open',
  merged: 'Merged',
  closed: 'Closed',
};

function formatDate(iso: string): string {
  const t = Date.parse(iso);
  return Number.isNaN(t) ? iso : new Date(t).toLocaleDateString();
}

function mergeableLabel(mergeable: boolean | null): { text: string; cls: string } {
  if (mergeable === null) return { text: 'Mergeability pending', cls: 'pending' };
  return mergeable
    ? { text: 'No conflicts', cls: 'clean' }
    : { text: 'Has conflicts', cls: 'conflict' };
}

/** A modified/auxiliary click (ctrl/cmd/shift/alt, middle button) must reach the
 *  platform untouched, so open-in-new-tab keeps working in the browser harness;
 *  in the Tauri webview it is a no-op. `true` ⇒ do NOT intercept. */
function isPlatformClick(e: MouseEvent): boolean {
  return e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0;
}

/** Tooltip text for the destination (P72 security audit LOW-4). Showing the URL
 *  is the right link affordance — it is how a user spots a link that does not go
 *  where the label implies — but `summary.url` comes from a forge API response,
 *  so it is neither length- nor content-bounded. Native tooltips render embedded
 *  newlines, so a crafted URL could push the real host out of view behind a
 *  plausible-looking first line. Collapse whitespace/control characters and
 *  truncate, so the tooltip always shows the BEGINNING of one single line. */
function destinationTitle(url: string): string {
  // Collapse every whitespace and C0/DEL control run to ONE space. The escapes
  // must stay as escapes: writing the class with literal control bytes puts a
  // raw NUL in the source, and writing it as `[\s -]` would read as
  // "whitespace, space or hyphen" and silently strip hyphens from real hosts.
  // eslint-disable-next-line no-control-regex -- flattening C0/DEL is the point
  const oneLine = url.replace(/[\s\u0000-\u001f\u007f]+/g, ' ').trim();
  return oneLine.length > 120 ? `${oneLine.slice(0, 119)}…` : oneLine;
}

export interface PrDetailViewProps {
  detail: PrDetail;
  onBack(): void;
  /** P72: route "Open in browser ↗" through the openUrl IPC — a bare
   *  `target="_blank"` is a silent no-op in the native webview. The URL comes
   *  from the forge API response, which is why the Rust side validates it.
   *  REQUIRED so a future call site cannot regress to a dead link. */
  onOpenUrl(url: string): void;
  /** Comments component (or its loading/error state) rendered under the body. */
  children?: ReactNode;
  /** P83: forge kind (drives the per-forge close label + method filter). */
  kind: ForgeKind;
  /** P83: merge methods this forge supports (already filtered). */
  supportedMethods: MergeMethod[];
  /** P83: an action is in flight (both buttons disabled; panel locked). */
  busy: boolean;
  /** P83: open the merge dialog. */
  onMerge(): void;
  /** P83: open the close/decline/abandon confirm. */
  onClose(): void;
}

export function PrDetailView({
  detail,
  onBack,
  onOpenUrl,
  children,
  kind,
  supportedMethods,
  busy,
  onMerge,
  onClose,
}: PrDetailViewProps) {
  const { summary } = detail;
  const merge = mergeableLabel(detail.mergeable);
  return (
    <div className="pr-detail" aria-busy={busy || undefined}>
      <div className="pr-detail-header">
        <div className="pr-detail-title-row">
          <button
            type="button"
            className="section-action pr-back-button"
            onClick={onBack}
          >
            {'← Pull requests'}
          </button>
          <a
            className="section-action pr-open-link"
            href={summary.url}
            target="_blank"
            rel="noreferrer noopener"
            title={destinationTitle(summary.url)}
            onClick={(e) => {
              if (isPlatformClick(e)) return; // let ctrl/middle-click through
              e.preventDefault();
              onOpenUrl(summary.url);
            }}
          >
            {/* aria-hidden keeps the accessible name exactly "Open in browser". */}
            {'Open in browser '}
            <span aria-hidden="true">↗</span>
          </a>
        </div>
        <div className="pr-detail-title">
          <span className={`pr-state-pill pr-state-${summary.state}`}>
            {STATE_LABEL[summary.state]}
          </span>
          {summary.isDraft && <span className="pr-draft-tag">Draft</span>}
          <span className="pr-detail-title-text">{summary.title}</span>
          <span className="pr-detail-num mono">{`#${summary.number}`}</span>
        </div>
        <div className="pr-detail-meta">
          <span className="pr-detail-author">{summary.author}</span>
          <span className="pr-detail-branches mono">
            {`${summary.sourceBranch} → ${summary.targetBranch}`}
          </span>
          <span className="pr-detail-date">{`opened ${formatDate(summary.createdAt)}`}</span>
        </div>
        <div className="pr-detail-stats">
          {summary.state === 'open' && (
            <span className={`pr-mergeable pr-mergeable-${merge.cls}`}>{merge.text}</span>
          )}
          <span className="pr-stat-add">{`+${detail.additions}`}</span>
          <span className="pr-stat-del">{`−${detail.deletions}`}</span>
          <span className="pr-stat-files">
            {`${detail.changedFiles} file${detail.changedFiles === 1 ? '' : 's'}`}
          </span>
        </div>
        {detail.labels.length > 0 && (
          <div className="pr-labels">
            {detail.labels.map((label) => (
              <span key={label} className="pr-label">
                {label}
              </span>
            ))}
          </div>
        )}
      </div>

      {detail.body !== '' ? (
        <div className="pr-body">{detail.body}</div>
      ) : (
        <p className="pane-empty pr-empty">No description provided.</p>
      )}

      {children}

      <PrActionsBar
        state={summary.state}
        kind={kind}
        mergeable={detail.mergeable}
        supportedMethods={supportedMethods}
        busy={busy}
        onMerge={onMerge}
        onClose={onClose}
      />
    </div>
  );
}
