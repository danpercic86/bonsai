import { useEffect, useRef, useState } from 'react';
import type { HeadInfo, RecentRepo, RepoInfo } from '../ipc';

function folderName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

function HeadSummary({ head }: { head: HeadInfo }) {
  if (head.unborn) {
    return (
      <span className="head-summary">
        <span className="head-branch">{head.branchName ?? '?'}</span>
        <span className="pill pill-unborn">no commits yet</span>
      </span>
    );
  }
  if (head.detached) {
    return (
      <span className="head-summary">
        <span className="head-branch">
          HEAD detached @ <span className="mono">{shortOid(head.oid)}</span>
        </span>
        <span className="pill pill-detached">detached</span>
      </span>
    );
  }
  return (
    <span className="head-summary">
      <span className="head-branch">
        {'⎇ '}
        {head.branchName ?? '?'} @ <span className="mono">{shortOid(head.oid)}</span>
      </span>
    </span>
  );
}

export interface RepoSwitcherProps {
  /** Current (open) repo — name/path/HeadSummary render as today. */
  repo: RepoInfo;
  /** App state; the current repo is filtered out INSIDE the component. */
  recents: RecentRepo[];
  disabled: boolean;
  onOpenPath(path: string): void;
  onBrowse(): void;
  /** P1 §6.2: lifted so App's global shortcut handler can suppress bindings
   *  while the dropdown is open, and its Esc effect can skip a keypress this
   *  component's own Esc listener already consumed (Sidebar/ConfirmDialog
   *  pattern — onDialogOpenChange). */
  onOpenChange?(open: boolean): void;
}

/** Header repo block, now a button with a `▾` affix opening a recents
 * dropdown (P1 §10.3). Replaces the static `.header-repo` block. */
export function RepoSwitcher({
  repo,
  recents,
  disabled,
  onOpenPath,
  onBrowse,
  onOpenChange,
}: RepoSwitcherProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const close = () => {
    setOpen(false);
    onOpenChange?.(false);
  };

  useEffect(() => {
    if (!open) return;
    const onMouseDown = (e: MouseEvent) => {
      if (rootRef.current !== null && !rootRef.current.contains(e.target as Node)) {
        close();
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
    };
    document.addEventListener('mousedown', onMouseDown);
    window.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onMouseDown);
      window.removeEventListener('keydown', onKeyDown);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const others = recents.filter((r) => r.path.toLowerCase() !== repo.path.toLowerCase());

  return (
    <div className="repo-switcher" ref={rootRef}>
      <button
        type="button"
        className="header-repo header-repo-btn"
        disabled={disabled}
        onClick={() => {
          const next = !open;
          setOpen(next);
          onOpenChange?.(next);
        }}
        title={repo.path}
      >
        <span className="repo-name">{folderName(repo.path)}</span>
        <span className="repo-path">{repo.path}</span>
        {repo.head && <HeadSummary head={repo.head} />}
        <span className="repo-switcher-caret">{'▾'}</span>
      </button>
      {open && (
        <div className="repo-switcher-menu">
          {others.length > 0 && (
            <>
              {others.map((r) => (
                <button
                  key={r.path}
                  type="button"
                  className="repo-switcher-item"
                  onClick={() => {
                    close();
                    onOpenPath(r.path);
                  }}
                >
                  <span className="repo-switcher-item-name">{folderName(r.path)}</span>
                  <span className="repo-switcher-item-path" title={r.path}>
                    {r.path}
                  </span>
                </button>
              ))}
              <div className="repo-switcher-sep" />
            </>
          )}
          <button
            type="button"
            className="repo-switcher-item repo-switcher-browse"
            onClick={() => {
              close();
              onBrowse();
            }}
          >
            {'Browse…'}
          </button>
        </div>
      )}
    </div>
  );
}
