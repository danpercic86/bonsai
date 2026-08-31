import { createElement, useState } from 'react';
import type { ReflogEntry, ResetMode } from '../ipc';
import { relativeDate } from '../graph/draw';
import { ContextMenu } from './ContextMenu';
import type { ContextMenuItem } from './ContextMenu';
import { MoreIcon } from './appIcons';
import { BranchIcon, RebaseIcon } from './menuIcons';

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

const ZERO_OID = '0'.repeat(40);

export interface ReflogViewProps {
  /** "HEAD" or a branch name — drives the header label ("HEAD" / "branch: main"). */
  refName: string;
  entries: ReflogEntry[];
  loading: boolean;
  error: string | null;
  /** True while a restore action is in flight → disable per-row action buttons. */
  busy: boolean;
  /** The current HEAD branch name (or "HEAD") — labels the reset menu items. */
  resetBranchLabel: string;
  onClose(): void;
  /** Reveal (select + scroll) the entry's newOid in the graph, if present. */
  onRevealCommit(oid: string): void;
  /** Arm "Create branch here" for this entry's newOid (opens the shared PromptDialog). */
  onCreateBranch(newOid: string): void;
  /** Arm "Reset current branch to this" for newOid + mode (opens the shared reset
   *  ConfirmDialog). Undefined when reset is not allowed (detached/unborn HEAD) →
   *  the view hides the reset actions. */
  onReset?: (newOid: string, mode: ResetMode) => void;
}

/** Read-mostly reflog overlay (P38 §7.1). A sibling to BlameView/FileHistoryView:
 *  same `diff-overlay` chrome, skeleton loading, error/empty placeholders, and
 *  Esc/close via the container's `reflogReqId` stale-guard. Each row offers
 *  "Create branch here" + (soft/mixed/hard) "Reset …" via a compact kebab menu
 *  that dispatches the SHARED create-branch / reset dialogs — no new mutation. */
export function ReflogView({
  refName,
  entries,
  loading,
  error,
  busy,
  resetBranchLabel,
  onClose,
  onRevealCommit,
  onCreateBranch,
  onReset,
}: ReflogViewProps) {
  const now = Math.floor(Date.now() / 1000);
  // The kebab-menu anchor: which entry index + viewport position, or null.
  const [menu, setMenu] = useState<{ index: number; oid: string; x: number; y: number } | null>(
    null,
  );

  const label = refName === 'HEAD' ? 'HEAD' : `branch: ${refName}`;

  const openMenu = (e: React.MouseEvent, index: number, oid: string) => {
    e.stopPropagation();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setMenu({ index, oid, x: rect.left, y: rect.bottom + 2 });
  };

  const menuItems = (oid: string): ContextMenuItem[] => {
    const items: ContextMenuItem[] = [
      {
        label: 'Create branch here',
        icon: createElement(BranchIcon),
        disabled: busy,
        onSelect: () => onCreateBranch(oid),
      },
    ];
    if (onReset !== undefined) {
      const make = (mode: ResetMode, text: string): ContextMenuItem => ({
        label: text,
        icon: createElement(RebaseIcon),
        disabled: busy,
        onSelect: () => onReset(oid, mode),
      });
      items.push(make('soft', `Reset ${resetBranchLabel} to this (soft)`));
      items.push(make('mixed', `Reset ${resetBranchLabel} to this (mixed)`));
      items.push(make('hard', `Reset ${resetBranchLabel} to this (hard)…`));
    }
    return items;
  };

  return (
    <div className="diff-overlay reflog-view" role="region" aria-label={`Reflog: ${label}`}>
      <div className="diff-overlay-header">
        <span className="diff-overlay-path mono" title={label}>
          {label}
        </span>
        <span className="diff-overlay-kind">Reflog</span>
        <button
          type="button"
          className="btn-icon diff-overlay-close"
          aria-label="Close reflog"
          title="Close (Esc)"
          onClick={onClose}
        >
          {'×'}
        </button>
      </div>
      <div className="diff-overlay-body">
        {error !== null ? (
          <div className="diff-placeholder">{error}</div>
        ) : loading && entries.length === 0 ? (
          <div className="diff-slot-loading skeleton-group" aria-hidden="true">
            {Array.from({ length: 6 }, (_, i) => (
              <div key={i} className="skeleton-row" />
            ))}
          </div>
        ) : entries.length === 0 ? (
          <div className="diff-placeholder">No reflog entries for {label}</div>
        ) : (
          <div className={loading ? 'diff-scroll diff-stale' : 'diff-scroll'}>
            <ul className="reflog-list">
              {entries.map((entry) => {
                const isRoot = entry.oldOid === ZERO_OID;
                return (
                  <li key={`${entry.index}:${entry.newOid}`} className="reflog-row">
                    <button
                      type="button"
                      className="reflog-main"
                      title={`${refName}@{${entry.index}} — ${entry.message}\nClick to reveal in graph`}
                      onClick={() => onRevealCommit(entry.newOid)}
                    >
                      <span className="reflog-index mono">{`${refName}@{${entry.index}}`}</span>
                      <span className="reflog-oids mono">
                        {isRoot ? (
                          <span className="reflog-oid-root" title="Ref creation (no prior tip)">
                            (root)
                          </span>
                        ) : (
                          <span className="reflog-oid-old">{shortOid(entry.oldOid)}</span>
                        )}
                        <span className="reflog-oid-arrow" aria-hidden="true">
                          {' → '}
                        </span>
                        <span className="reflog-oid-new">{shortOid(entry.newOid)}</span>
                      </span>
                      <span className="reflog-message">{entry.message}</span>
                      <span className="reflog-author">{entry.committerName}</span>
                      <span className="reflog-date">{relativeDate(entry.committerTs, now)}</span>
                    </button>
                    <button
                      type="button"
                      className="btn-icon reflog-actions"
                      aria-label={`Restore actions for ${refName}@{${entry.index}}`}
                      aria-haspopup="menu"
                      title="Restore actions"
                      disabled={busy}
                      onClick={(e) => openMenu(e, entry.index, entry.newOid)}
                    >
                      <MoreIcon />
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        )}
      </div>
      {menu !== null && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems(menu.oid)}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}
