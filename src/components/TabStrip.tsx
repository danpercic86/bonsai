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
  /** Reorder tabs by drag-and-drop (issue 4): move tab at `fromIndex` to
   *  `toIndex` in display order. */
  onReorder(fromIndex: number, toIndex: number): void;
  /** Folder picker. */
  onBrowse(): void;
  /** Open the Clone-repository dialog. */
  onClone(): void;
  /** New repository (folder picker → init). */
  onInit(): void;
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
  onReorder,
  onBrowse,
  onClone,
  onInit,
  onMenuOpenChange,
}: TabStripProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  // Drag-and-drop reorder (issue 4): source index recorded on drag start, and
  // the index the pointer is currently hovering (drop target) for CSS feedback.
  const dragFrom = useRef<number | null>(null);
  const [dropTarget, setDropTarget] = useState<number | null>(null);
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
      <div className="tab-scroll">
        {tabs.map((t, index) => (
          <div
            key={t.repoId}
            className={
              `tab${t.repoId === activeRepo ? ' tab-active' : ''}` +
              `${dropTarget === index ? ' tab-drop-target' : ''}`
            }
            draggable
            onDragStart={(e) => {
              dragFrom.current = index;
              e.dataTransfer.effectAllowed = 'move';
              // Some browsers require data to be set for the drag to begin.
              e.dataTransfer.setData('text/plain', String(index));
            }}
            onDragOver={(e) => {
              if (dragFrom.current === null) return;
              e.preventDefault();
              e.dataTransfer.dropEffect = 'move';
              if (dropTarget !== index) setDropTarget(index);
            }}
            onDrop={(e) => {
              e.preventDefault();
              const from = dragFrom.current;
              if (from !== null && from !== index) onReorder(from, index);
              dragFrom.current = null;
              setDropTarget(null);
            }}
            onDragEnd={() => {
              dragFrom.current = null;
              setDropTarget(null);
            }}
          >
            {/* Label is a role=button span, NOT a native <button>: a
                <button> child swallows the mousedown and prevents the
                draggable `.tab` ancestor from ever starting a reorder drag
                (Chromium/WebView2 form-control drag suppression). */}
            <span
              className="tab-label"
              role="button"
              tabIndex={disabled ? -1 : 0}
              aria-disabled={disabled}
              onClick={() => {
                if (!disabled) onSelect(t.repoId);
              }}
              onKeyDown={(e) => {
                if (disabled) return;
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  onSelect(t.repoId);
                }
              }}
              title={t.path}
            >
              {folderName(t.path)}
            </span>
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
      </div>
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
            <button
              type="button"
              className="repo-switcher-item"
              onClick={() => {
                close();
                onClone();
              }}
            >
              {'Clone repository…'}
            </button>
            <button
              type="button"
              className="repo-switcher-item"
              onClick={() => {
                close();
                onInit();
              }}
            >
              {'New repository…'}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
