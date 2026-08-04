import { useEffect, useRef, useState } from 'react';
import type { StashScope } from '../ipc';

export interface StashSplitButtonProps {
  /** mutating || no changes at all → whole control disabled. */
  disabled: boolean;
  /** enables the 'staged' option. */
  stagedCount: number;
  /** staged||unstaged present → enables 'all'. */
  hasTrackedChanges: boolean;
  /** untracked present → enables 'allWithUntracked' (also enabled by tracked changes). */
  hasUntracked: boolean;
  onStash(scope: StashScope): void;
}

interface ScopeItem {
  scope: StashScope;
  label: string;
  enabled: boolean;
}

/** P34: the staging-panel stash control. Primary action stashes `all`; the caret
 *  opens a small menu offering the three scopes. Each item is disabled when its
 *  scope has nothing to capture (mirrors the backend created:false rule). Purely
 *  presentational — the parent owns the IPC call, toast and refresh. */
export function StashSplitButton({
  disabled,
  stagedCount,
  hasTrackedChanges,
  hasUntracked,
  onStash,
}: StashSplitButtonProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

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
    <div className="stash-split" ref={rootRef}>
      <div className="stash-split-row">
        <button
          type="button"
          className="btn-secondary stash-split-primary"
          disabled={disabled || !hasTrackedChanges}
          onClick={() => choose('all')}
          title="Stash tracked changes (staged + unstaged)"
        >
          Stash all
        </button>
        <button
          type="button"
          className="btn-secondary stash-split-caret"
          disabled={disabled}
          aria-haspopup="menu"
          aria-expanded={open}
          aria-label="Stash options"
          onClick={() => setOpen((v) => !v)}
        >
          {'▾'}
        </button>
      </div>
      {open && (
        <div className="stash-split-menu" role="menu">
          {items.map((it) => (
            <button
              key={it.scope}
              type="button"
              role="menuitem"
              className="stash-split-menu-item"
              disabled={!it.enabled}
              onClick={() => choose(it.scope)}
            >
              {it.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
