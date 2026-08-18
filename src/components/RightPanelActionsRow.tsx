import { useEffect, useRef, useState } from 'react';
import type { StashScope } from '../ipc';

export interface RightPanelActionsRowProps {
  /** Amend state — OWNED BY WorkspaceRightPanel (P67 D4: CommitBox is keyed on
   *  it, so a checkbox inside that subtree would lose focus on every toggle). */
  amend: boolean;
  onToggleAmend(next: boolean): void;
  /** App-wide mutation in flight → every control disabled. */
  busy: boolean;
  /** Amend would rewrite already-pushed history (upstream set && ahead === 0). */
  showAmendPushWarning: boolean;
  /** `busy || nothing to stash at all` → the ⋯ button is disabled. */
  stashDisabled: boolean;
  /** Per-scope enablement, identical rules to the deleted StashSplitButton. */
  stagedCount: number;
  hasTrackedChanges: boolean;
  hasUntracked: boolean;
  onStash(scope: StashScope): void;
}

interface ScopeItem {
  scope: StashScope;
  label: string;
  enabled: boolean;
}

/** P67 §5.1: the single slim actions row above the commit box — Amend checkbox
 *  on the left, a `⋯` overflow menu on the right. Replaces the two stacked rows
 *  (`.stash-split` + `.amend-affordance`, ~62 px) that P34/P20 left behind, which
 *  is the space the changes tree gets back. Stash is demoted into the menu with
 *  ALL THREE scopes and the same per-scope gating StashSplitButton had (the
 *  sidebar's one-click stash stays the fast path). Purely presentational — the
 *  parent owns the IPC call, toast and refresh. */
export function RightPanelActionsRow({
  amend,
  onToggleAmend,
  busy,
  showAmendPushWarning,
  stashDisabled,
  stagedCount,
  hasTrackedChanges,
  hasUntracked,
  onStash,
}: RightPanelActionsRowProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  // Moved verbatim from StashSplitButton: outside-mousedown and Escape close.
  useEffect(() => {
    if (!open) return;
    function onDocMouseDown(e: MouseEvent) {
      if (rootRef.current !== null && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') setOpen(false);
    }
    document.addEventListener('mousedown', onDocMouseDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDocMouseDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  const items: ScopeItem[] = [
    { scope: 'all', label: 'Stash all', enabled: hasTrackedChanges },
    {
      scope: 'allWithUntracked',
      label: 'Stash all + untracked',
      enabled: hasTrackedChanges || hasUntracked,
    },
    { scope: 'staged', label: 'Stash staged only', enabled: stagedCount > 0 },
  ];

  function choose(scope: StashScope) {
    setOpen(false);
    onStash(scope);
  }

  return (
    <div className="rp-actions">
      <div className="rp-actions-row">
        <label className="amend-toggle">
          <input
            type="checkbox"
            checked={amend}
            disabled={busy}
            onChange={(e) => void onToggleAmend(e.target.checked)}
          />
          <span>Amend last commit</span>
        </label>
        <div className="rp-overflow" ref={rootRef}>
          <button
            type="button"
            className="rp-overflow-btn"
            aria-haspopup="menu"
            aria-expanded={open}
            aria-label="More actions"
            disabled={stashDisabled}
            onClick={() => setOpen((v) => !v)}
          >
            {'⋯'}
          </button>
          {open && (
            <div className="rp-overflow-menu" role="menu">
              {items.map((it) => (
                <button
                  key={it.scope}
                  type="button"
                  role="menuitem"
                  className="rp-overflow-item"
                  disabled={!it.enabled}
                  onClick={() => choose(it.scope)}
                >
                  {it.label}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
      {showAmendPushWarning && (
        <div className="amend-push-warning" role="note">
          This commit is already pushed — amending rewrites published history.
        </div>
      )}
    </div>
  );
}
