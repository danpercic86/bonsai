import { useEffect, useRef, useState } from 'react';
import type { SigningStatus, StashScope } from '../ipc';

export interface CommitOptionsMenuProps {
  /** `blocked` — disables the whole `⋯` trigger (P80 D2). */
  disabled: boolean;
  /** App-wide mutation in flight → every action item is disabled. */
  busy: boolean;

  // ---- E1: one context-scoped AI review (top of the menu, when aiEligible).
  aiEligible: boolean;
  /** Number of staged entries — picks the Review scope (staged vs worktree). */
  stagedCount: number;
  /** Any working-tree change exists — gates worktree Review on a clean tree. */
  workingDirty: boolean;
  /** True while an AI explain/review call is in flight — disables Review. */
  aiAnalyzing: boolean;
  onReviewStaged(): void;
  onReviewWorktree(): void;

  // ---- Amend (owned upstream by RepoWorkspace; menu only forwards the toggle).
  canAmend: boolean;
  amend: boolean;
  onToggleAmend(next: boolean): void;

  // ---- Sign commit (P58c) — only when signingStatus is known + non-merge.
  showSign: boolean;
  signChecked: boolean;
  onChangeSign(next: boolean): void;
  /** 'SSH' | 'GPG' — precomputed by CommitBox; rendered in the Sign item label. */
  signFormatLabel: string;
  signingStatus: SigningStatus | null | undefined;

  // ---- Skip hooks (P59a) — offered in every commit-like mode.
  skipHooks: boolean;
  onChangeSkipHooks(next: boolean): void;

  // ---- Compose commits (P54c) — only when showCompose && aiEligible.
  showCompose: boolean;
  composeDisabled: boolean;
  composeTitle: string;
  onCompose(): void;

  // ---- Stash scopes (absorbed from RightPanelActionsRow — verbatim gating).
  /** Equivalent of the old `opState.kind === 'none' && head && !head.unborn`
   *  gate: stash is only reachable in the normal working state (never during a
   *  merge/rebase/etc, where `git stash` refuses on unmerged paths). */
  canStash: boolean;
  hasTrackedChanges: boolean;
  hasUntracked: boolean;
  onStash(scope: StashScope): void;
}

interface StashItem {
  scope: StashScope;
  label: string;
  enabled: boolean;
}

/** P80 §2b: the `⋯` overflow menu for the commit box — absorbs Amend, Sign,
 *  Skip hooks (menuitemcheckbox), Compose + the single context-scoped Review,
 *  and the three Stash scopes (menuitem). Opens UPWARD from the message toolbar.
 *  Purely presentational — the parent owns every IPC call, toast and refresh. */
export function CommitOptionsMenu({
  disabled,
  busy,
  aiEligible,
  stagedCount,
  workingDirty,
  aiAnalyzing,
  onReviewStaged,
  onReviewWorktree,
  canAmend,
  amend,
  onToggleAmend,
  showSign,
  signChecked,
  onChangeSign,
  signFormatLabel,
  skipHooks,
  onChangeSkipHooks,
  showCompose,
  composeDisabled,
  composeTitle,
  onCompose,
  canStash,
  hasTrackedChanges,
  hasUntracked,
  onStash,
}: CommitOptionsMenuProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  // Outside-mousedown + Escape close; focus returns to the `⋯` trigger (moved
  // verbatim from the deleted RightPanelActionsRow).
  useEffect(() => {
    if (!open) return;
    function onDocMouseDown(e: MouseEvent) {
      if (rootRef.current !== null && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        setOpen(false);
        triggerRef.current?.focus();
      }
    }
    document.addEventListener('mousedown', onDocMouseDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDocMouseDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  // Roving focus + arrow-key navigation (mirrors ContextMenu's keyboard model):
  // focus the first enabled item on open; ArrowUp/Down wrap; Home/End jump.
  const menuItemButtons = (): HTMLButtonElement[] => {
    const el = menuRef.current;
    if (el === null) return [];
    return Array.from(el.querySelectorAll<HTMLButtonElement>('button.rp-overflow-item'));
  };

  useEffect(() => {
    if (!open) return;
    // Defer to after paint so the items exist to be focused.
    const id = window.requestAnimationFrame(() => {
      const first = menuItemButtons().find((b) => !b.disabled);
      first?.focus();
    });
    return () => window.cancelAnimationFrame(id);
  }, [open]);

  const moveFocus = (from: number, step: number) => {
    const buttons = menuItemButtons();
    const n = buttons.length;
    if (n === 0) return;
    for (let k = 1; k <= n; k++) {
      const j = (((from + step * k) % n) + n) % n;
      if (!buttons[j].disabled) {
        buttons[j].focus();
        return;
      }
    }
  };

  const focusEdge = (fromEnd: boolean) => {
    const buttons = menuItemButtons();
    const ordered = fromEnd ? [...buttons].reverse() : buttons;
    ordered.find((b) => !b.disabled)?.focus();
  };

  const onItemKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>) => {
    const buttons = menuItemButtons();
    const index = buttons.indexOf(e.currentTarget);
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        moveFocus(index, 1);
        return;
      case 'ArrowUp':
        e.preventDefault();
        moveFocus(index, -1);
        return;
      case 'Home':
        e.preventDefault();
        focusEdge(false);
        return;
      case 'End':
        e.preventDefault();
        focusEdge(true);
        return;
      default:
        return;
    }
  };

  const reviewStaged = stagedCount > 0;
  const reviewLabel = aiAnalyzing
    ? '✨ Reviewing…'
    : reviewStaged
      ? '✨ Review staged'
      : '✨ Review changes';

  const stashItems: StashItem[] = [
    { scope: 'all', label: 'Stash all', enabled: hasTrackedChanges },
    {
      scope: 'allWithUntracked',
      label: 'Stash all + untracked',
      enabled: hasTrackedChanges || hasUntracked,
    },
    { scope: 'staged', label: 'Stash staged only', enabled: stagedCount > 0 },
  ];

  function choose(fn: () => void) {
    setOpen(false);
    fn();
  }

  // Only offer Review when there is something to review: staged Review needs
  // staged>0; worktree Review needs a dirty working tree. Clean tree ⇒ hidden.
  const showReview = aiEligible && (reviewStaged || workingDirty);
  const showComposeItem = showCompose && aiEligible;

  return (
    <div className="rp-overflow commit-options-overflow" ref={rootRef}>
      <button
        type="button"
        ref={triggerRef}
        className="rp-overflow-btn"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label="Commit options"
        disabled={disabled}
        onClick={() => setOpen((v) => !v)}
      >
        {'⋯'}
      </button>
      {open && (
        <div className="rp-overflow-menu rp-overflow-menu-up" role="menu" ref={menuRef}>
          {showReview && (
            <>
              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                className="rp-overflow-item rp-overflow-item-ai"
                disabled={aiAnalyzing}
                onClick={() => choose(reviewStaged ? onReviewStaged : onReviewWorktree)}
                onKeyDown={onItemKeyDown}
              >
                <span className="rp-overflow-check" aria-hidden="true" />
                <span className="rp-overflow-item-label">{reviewLabel}</span>
              </button>
              <div className="rp-overflow-sep" role="separator" />
            </>
          )}
          {canAmend && (
            <button
              type="button"
              role="menuitemcheckbox"
              aria-checked={amend}
              tabIndex={-1}
              className="rp-overflow-item"
              disabled={busy}
              onClick={() => onToggleAmend(!amend)}
              onKeyDown={onItemKeyDown}
            >
              <span className="rp-overflow-check" aria-hidden="true">
                {amend ? '✓' : ''}
              </span>
              <span className="rp-overflow-item-label">Amend last commit</span>
            </button>
          )}
          {showSign && (
            <button
              type="button"
              role="menuitemcheckbox"
              aria-checked={signChecked}
              tabIndex={-1}
              className="rp-overflow-item"
              disabled={busy}
              onClick={() => onChangeSign(!signChecked)}
              onKeyDown={onItemKeyDown}
            >
              <span className="rp-overflow-check" aria-hidden="true">
                {signChecked ? '✓' : ''}
              </span>
              <span className="rp-overflow-item-label">Sign commit ({signFormatLabel})</span>
            </button>
          )}
          <button
            type="button"
            role="menuitemcheckbox"
            aria-checked={skipHooks}
            tabIndex={-1}
            className="rp-overflow-item"
            disabled={busy}
            onClick={() => onChangeSkipHooks(!skipHooks)}
            onKeyDown={onItemKeyDown}
          >
            <span className="rp-overflow-check" aria-hidden="true">
              {skipHooks ? '✓' : ''}
            </span>
            <span className="rp-overflow-item-label">Skip hooks</span>
          </button>
          {showComposeItem && (
            <>
              <div className="rp-overflow-sep" role="separator" />
              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                className="rp-overflow-item rp-overflow-item-ai"
                disabled={composeDisabled}
                title={composeTitle}
                onClick={() => choose(onCompose)}
                onKeyDown={onItemKeyDown}
              >
                <span className="rp-overflow-check" aria-hidden="true" />
                <span className="rp-overflow-item-label">✨ Compose commits</span>
              </button>
            </>
          )}
          {canStash && <div className="rp-overflow-sep" role="separator" />}
          {canStash &&
            stashItems.map((it) => (
            <button
              key={it.scope}
              type="button"
              role="menuitem"
              tabIndex={-1}
              className="rp-overflow-item"
              disabled={busy || !it.enabled}
              onClick={() => choose(() => onStash(it.scope))}
              onKeyDown={onItemKeyDown}
            >
              <span className="rp-overflow-check" aria-hidden="true" />
              <span className="rp-overflow-item-label">{it.label}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
