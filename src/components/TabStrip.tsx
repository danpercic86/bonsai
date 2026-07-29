import { useEffect, useRef, useState } from 'react';
import type { RecentRepo } from '../ipc';

/** One open tab. `repoId` == canonical workdir path (P3e §2); `path` is the
 *  same string, kept explicitly for the display label. */
export interface TabMeta {
  repoId: string;
  path: string;
}

function folderName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

export interface TabStripProps {
  tabs: TabMeta[];
  activeRepo: string | null;
  recents: RecentRepo[];
  /** Disabled during the initial pick (folder dialog open). */
  disabled: boolean;
  onSelect(repoId: string): void;
  onClose(repoId: string): void;
  /** Open (or focus) a recents path — adds/focuses a tab. */
  onOpenPath(path: string): void;
  /** Folder picker. */
  onBrowse(): void;
  /** P3e §5.6: lifts menu-open like RepoSwitcher.onOpenChange — App suppresses
   *  global shortcuts while open and its Esc effect skips the consumed key. */
  onMenuOpenChange?(open: boolean): void;
}

/** P3e §5.6: the multi-tab header strip (replaces RepoSwitcher). One pill per
 *  open repo (folder name + close), a trailing `+` opening the recents dropdown
 *  and Browse… affordance (recents already open in a tab are filtered out). */
export function TabStrip({
  tabs,
  activeRepo,
  recents,
  disabled,
  onSelect,
  onClose,
  onOpenPath,
  onBrowse,
  onMenuOpenChange,
}: TabStripProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const close = () => {
    setMenuOpen(false);
    onMenuOpenChange?.(false);
  };

  useEffect(() => {
    if (!menuOpen) return;
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
  }, [menuOpen]);

  // Recents already open in a tab are hidden from the dropdown (case-insensitive
  // path match — same identity notion as the backend dedupe).
  const openPaths = new Set(tabs.map((t) => t.path.toLowerCase()));
  const others = recents.filter((r) => !openPaths.has(r.path.toLowerCase()));

  return (
    <div className="tab-strip" ref={rootRef}>
      {tabs.map((t) => (
        <div key={t.repoId} className={`tab${t.repoId === activeRepo ? ' tab-active' : ''}`}>
          <button
            type="button"
            className="tab-label"
            disabled={disabled}
            onClick={() => onSelect(t.repoId)}
            title={t.path}
          >
            {folderName(t.path)}
          </button>
          <button
            type="button"
            className="tab-close"
            aria-label={`Close ${folderName(t.path)}`}
            title={`Close ${folderName(t.path)}`}
            onClick={() => onClose(t.repoId)}
          >
            {'×'}
          </button>
        </div>
      ))}
      <div className="tab-add-wrap">
        <button
          type="button"
          className="tab-add"
          disabled={disabled}
          onClick={() => {
            const next = !menuOpen;
            setMenuOpen(next);
            onMenuOpenChange?.(next);
          }}
          title="Open a repository"
          aria-label="Open a repository"
        >
          {'+'}
        </button>
        {menuOpen && (
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
    </div>
  );
}
