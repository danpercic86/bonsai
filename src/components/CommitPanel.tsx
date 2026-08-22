import { useState } from 'react';
import { relativeDate } from '../graph/draw';
import { verifyBadgeKind, verifyStatusLabel } from '../graph/verifyBadge';
import type { CommitDiff, CommitVerification, GraphNode, ListView } from '../ipc';
import { DiffFileTree } from './DiffFileTree';
import type { DiffScope } from './DiffFileTree';
import { SummarizeIcon } from './menuIcons';

// Mode B (M4 contract §4.3): shown INSTEAD of StatusPanel + CommitBox when a
// graph commit is selected. Presentational — App owns all fetching.
// P11g-rev §3.2: the file list is now the shared DiffFileTree, the SOLE scope
// navigator. Clicking root/folder/file drives the lifted `scope` AND opens the
// all-files DiffBrowser over the graph pane (commit mode is explicit-open).

const BODY_COLLAPSE_LINES = 8;

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

/** Compact a signing key / fingerprint for the inline signature line. */
function shortKey(key: string): string {
  return key.length > 26 ? `${key.slice(0, 25)}…` : key;
}

/** P58c: signature-status line under the author/date rows. Renders NOTHING for
 *  `unsigned` (no clutter) or when the selected commit is not yet verified (the
 *  verify map is the single source — no extra IPC for the selected commit). */
function SignatureLine({ signature }: { signature: CommitVerification }) {
  const kind = verifyBadgeKind(signature.status);
  if (kind === null) return null; // unsigned
  const glyph = kind === 'good' ? '✓' : kind === 'warn' ? '⚠' : '●';
  return (
    <div className={`commit-signature commit-signature-${kind}`}>
      <span className="commit-signature-icon" aria-hidden="true">
        {glyph}
      </span>
      <span className="commit-signature-text">{verifyStatusLabel(signature.status)}</span>
      {signature.signer !== undefined && (
        <span className="commit-signature-signer" title={signature.signer}>
          {signature.signer}
        </span>
      )}
      {signature.key !== undefined && (
        <span className="commit-signature-key mono" title={signature.key}>
          {shortKey(signature.key)}
        </span>
      )}
    </div>
  );
}

/** Body = message minus its first line and the following blank separator lines
 * (P1 §4.3 — the summary is always derived from line 1 by git2, so an
 * unconditional first-line strip can neither cut mid-line nor duplicate). */
function messageBody(message: string): string {
  const nl = message.indexOf('\n');
  if (nl === -1) return '';
  return message.slice(nl + 1).replace(/^(\r?\n)+/, '').replace(/^\r/, '');
}

function MessageBody({ body }: { body: string }) {
  const [showAll, setShowAll] = useState(false);
  const lines = body.split('\n');
  const collapsed = !showAll && lines.length > BODY_COLLAPSE_LINES;
  const visible = collapsed ? lines.slice(0, BODY_COLLAPSE_LINES).join('\n') : body;
  return (
    <div className="commit-msg-body">
      <pre className="commit-msg-text">{visible}</pre>
      {lines.length > BODY_COLLAPSE_LINES && (
        <button type="button" className="section-action" onClick={() => setShowAll(!showAll)}>
          {collapsed ? 'Show more' : 'Show less'}
        </button>
      )}
    </div>
  );
}

export function SkeletonRows() {
  return (
    <div className="skeleton-group" aria-hidden="true">
      {Array.from({ length: 4 }, (_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}

export interface CommitPanelProps {
  /** Selected node (immediate summary/oid while details load). */
  node: GraphNode;
  data: CommitDiff | null; // null while loading
  loading: boolean;
  error: string | null;
  /** P3b: flat vs directory-tree file list (display-only). */
  listView: ListView;
  /** P11g-rev §3.2: current diff scope (selection highlight) + its setter. */
  scope: DiffScope;
  onSelectScope(scope: DiffScope): void;
  /** Parent short-oid clicked; App maps to a row via node.parents indices. */
  onSelectParent(parentOrdinal: number): void;
  /** "×" button -> deselect. */
  onClose(): void;
  /** P15b: AI enabled+consented+CLI installed — gates the "✨ Explain" button. */
  aiEligible: boolean;
  /** P15b: request an AI explanation of this commit (App owns the call). */
  onExplain(): void;
  /** P58c: this commit's signature verdict from the shared verify map, or null
   *  when unverified / disabled. `unsigned` renders nothing. */
  signature: CommitVerification | null;
}

export function CommitPanel({
  node,
  data,
  loading,
  error,
  listView,
  scope,
  onSelectScope,
  onSelectParent,
  onClose,
  aiEligible,
  onExplain,
  signature,
}: CommitPanelProps) {
  const details = data?.details ?? null;
  const now = Math.floor(Date.now() / 1000);
  const body = details !== null ? messageBody(details.message) : '';

  return (
    <div className="commit-panel" data-testid="commit-details">
      <div className="commit-panel-header">
        <div className="commit-panel-title">
          <div className="commit-summary">{details?.summary ?? node.summary}</div>
          {aiEligible && (
            <button
              type="button"
              className="btn-secondary commit-explain-button"
              title="Explain this commit with AI"
              onClick={onExplain}
            >
              <SummarizeIcon />
              <span>Explain</span>
            </button>
          )}
          <button
            type="button"
            className="btn-icon commit-close"
            aria-label="Close commit details"
            title="Close"
            onClick={onClose}
          >
            {'×'}
          </button>
        </div>
        <div className="commit-oid mono" title={details?.oid ?? node.id}>
          {shortOid(details?.oid ?? node.id)}
        </div>
        {details !== null && (
          <>
            <div className="commit-author">
              {details.authorName}{' '}
              <span className="commit-author-email">{'<'}{details.authorEmail}{'>'}</span>
            </div>
            <div className="commit-date">
              {relativeDate(details.authorTs, now)}
              {' · '}
              {new Date(details.authorTs * 1000).toLocaleString()}
            </div>
            {signature !== null && <SignatureLine signature={signature} />}
            {details.parents.length > 0 && (
              <div className="commit-parents">
                <span className="commit-parents-label">Parents:</span>
                {details.parents.map((p, i) =>
                  node.parents[i] !== undefined ? (
                    <button
                      key={p}
                      type="button"
                      className="commit-parent-link mono"
                      title={p}
                      onClick={() => onSelectParent(i)}
                    >
                      {shortOid(p)}
                    </button>
                  ) : (
                    // Parent truncated out of the layout: plain text, no jump.
                    <span key={p} className="commit-parent-plain mono" title={p}>
                      {shortOid(p)}
                    </span>
                  ),
                )}
              </div>
            )}
            {details.parents.length > 1 && (
              <div className="commit-merge-note">Showing changes vs first parent</div>
            )}
          </>
        )}
      </div>

      {error !== null && (
        <div className="error-banner error-banner-dismissible commit-panel-error" role="alert">
          <span className="error-banner-text">{error}</span>
        </div>
      )}

      {details !== null && body !== '' && <MessageBody body={body} />}

      {loading && data === null ? (
        <div className="commit-panel-loading">
          <SkeletonRows />
        </div>
      ) : (
        data !== null && (
          <section className="status-section commit-files">
            <div className="section-header section-label">
              <span>Changes ({data.files.length})</span>
            </div>
            <DiffFileTree
              files={data.files}
              listView={listView}
              scope={scope}
              onSelect={onSelectScope}
            />
          </section>
        )
      )}
    </div>
  );
}
