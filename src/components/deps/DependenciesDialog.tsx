// P125-ui §2 / §4 / §5 / §6 — the Dependencies dialog CONTAINER. It owns the
// filter, the expanded-group / expanded-row / show-all sets, the link note,
// the context-menu target, the live-region text and the focus handling; it
// picks the one state body from `deriveState`. Everything else is a child.
//
// The tracker (`useDepTracker`) is mounted ONCE in App and passed in, so the
// toolbar button and this dialog share one fetch source.
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';

import type { UseDepTracker } from '../../hooks/useDepTracker';
import { useDialogFocus } from '../../hooks/useDialogFocus';
import { ipc } from '../../ipc';
import type { UiSettingsPatch } from '../../ipc';
import type { DepRow, DepTrackerSettings } from '../../ipc/types/depTracker';
import { ContextMenu } from '../ContextMenu';
import type { SettingsCategoryId } from '../settings/types';
import { DepsBodyList } from './DepsBodyList';
import { DepsFooter } from './DepsFooter';
import { DepsHeader } from './DepsHeader';
import { DepsNotice } from './DepsNotice';
import { DepsEmptyBody, DepsFailedBody, DepsLoadingBody, DepsOffBody } from './DepsStateBodies';
import { COPY_FAILED, LIVE_START, failureBody, failureDetails, linkFailure, liveFinished } from './depsCopy';
import { depsMenuItems } from './depsMenu';
import type { DepsFilter } from './depsView';
import { NO_FILTER, deriveState, firstSentence, noticeText, rowKey, updateCount } from './depsView';

export interface DependenciesDialogProps {
  open: boolean;
  onClose(): void;
  tracker: UseDepTracker;
  settings: DepTrackerSettings;
  onSettingsChange(patch: UiSettingsPatch): void;
  onOpenSettingsAt(category: SettingsCategoryId): void;
}

interface MenuTarget {
  row: DepRow;
  x: number;
  y: number;
}

function toggled<T>(set: ReadonlySet<T>, v: T): Set<T> {
  const next = new Set(set);
  if (next.has(v)) next.delete(v);
  else next.add(v);
  return next;
}

export function DependenciesDialog({
  open,
  onClose,
  tracker,
  settings,
  onSettingsChange,
  onOpenSettingsAt,
}: DependenciesDialogProps) {
  const { view, progress, refresh } = tracker;
  const state = deriveState(view);
  const titleId = useId();
  const idStem = useId();
  const cardRef = useRef<HTMLDivElement>(null);
  useDialogFocus(open, cardRef, true);

  const [filter, setFilter] = useState<DepsFilter>(NO_FILTER);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [shownAll, setShownAll] = useState<ReadonlySet<string>>(new Set());
  const [expandedRows, setExpandedRows] = useState<ReadonlySet<string>>(new Set());
  const [linkNote, setLinkNote] = useState<string | null>(null);
  const [menu, setMenu] = useState<MenuTarget | null>(null);
  const [live, setLive] = useState('');

  // The dialog-lifetime state resets on close (§4.3 "for the dialog's lifetime";
  // §6 the link note clears when the dialog closes).
  useEffect(() => {
    if (open) return;
    setFilter(NO_FILTER);
    setCollapsed(new Set());
    setShownAll(new Set());
    setExpandedRows(new Set());
    setLinkNote(null);
    setMenu(null);
    setLive('');
  }, [open]);

  // §4.1 live region: start / finish / failure transitions only — progress
  // counts are never announced.
  const wasRunning = useRef(state.running);
  useEffect(() => {
    const was = wasRunning.current;
    wasRunning.current = state.running;
    if (!open || view === null || !view.enabled || was === state.running) return;
    if (state.running) {
      setLive(LIVE_START);
      return;
    }
    const nowMs = Date.now();
    if (view.lastError !== null) {
      setLive(
        view.report !== null && state.notice !== null
          ? firstSentence(noticeText(state.notice, nowMs))
          : failureBody(view.lastError.kind).title,
      );
    } else if (view.report !== null) {
      setLive(liveFinished(view.report.summary.vulnerable, updateCount(view.report.summary)));
    }
  }, [open, view, state.running, state.notice]);

  const openUrl = useCallback((url: string, host: string) => {
    setLinkNote(null);
    ipc.openUrl(url).catch(() => setLinkNote(linkFailure(host)));
  }, []);

  // §8.3: a failed copy lands in the link-note slot (the overlay covers toasts).
  const copy = useCallback((text: string) => {
    setLinkNote(null);
    const p = navigator.clipboard?.writeText(text) ?? Promise.reject(new Error('Clipboard unavailable'));
    p.catch(() => setLinkNote(COPY_FAILED));
  }, []);

  // An open context menu consumes Esc: the menu's own capture listener closes
  // it, and this one stops the key reaching App's dialog-closing Esc layer.
  useEffect(() => {
    if (menu === null) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      setMenu(null);
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [menu]);

  const onCardKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    // §5.1: Ctrl/Cmd+F focuses the name filter, never the graph search.
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      e.stopPropagation();
      cardRef.current?.querySelector<HTMLInputElement>('.deps-filter input')?.focus();
    }
  };

  const openSettings = useCallback(() => {
    onClose();
    onOpenSettingsAt('general');
  }, [onClose, onOpenSettingsAt]);

  if (!open) return null;

  const report = view?.report ?? null;
  const running = state.running;
  const nowMs = Date.now();
  const checkNow = () => void refresh();

  let body: ReactNode;
  switch (state.body) {
    case 'off':
      body = (
        <DepsOffBody
          intervalHours={settings.intervalHours}
          onTurnOn={() => onSettingsChange({ depTracker: { ...settings, enabled: true } })}
          onOpenSettings={openSettings}
        />
      );
      break;
    case 'loading':
      body = <DepsLoadingBody />;
      break;
    case 'failed':
      body =
        view?.lastError != null ? (
          <DepsFailedBody
            kind={view.lastError.kind}
            details={failureDetails(view.lastError.message)}
            running={running}
            onRetry={checkNow}
          />
        ) : null;
      break;
    case 'empty':
      body = <DepsEmptyBody />;
      break;
    case 'list':
      body =
        report !== null ? (
          <DepsBodyList
            report={report}
            upToDate={state.upToDate}
            filter={filter}
            idStem={idStem}
            collapsed={collapsed}
            shownAll={shownAll}
            expandedRows={expandedRows}
            onFilter={setFilter}
            onToggleGroup={(p) => setCollapsed((s) => toggled(s, p))}
            onShowAll={(p) => setShownAll((s) => toggled(s, p))}
            onToggleRow={(p, k) => setExpandedRows((s) => toggled(s, `${p}\u0001${k}`))}
            onOpenUrl={openUrl}
            onRowMenu={(row, x, y) => setMenu({ row, x, y })}
          />
        ) : null;
      break;
  }

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={cardRef}
        className="dialog-card deps-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        onKeyDown={onCardKeyDown}
      >
        <DepsHeader
          titleId={titleId}
          checkedAtMs={report?.checkedAtMs ?? null}
          nowMs={nowMs}
          running={running}
          progress={progress}
          stalePill={state.stalePill}
          showCheckNow={state.body !== 'off' && view !== null}
          live={live}
          onCheckNow={checkNow}
          onClose={onClose}
        />
        <DepsNotice notice={state.notice} linkNote={linkNote} nowMs={nowMs} running={running} onCheckNow={checkNow} />
        {body}
        <DepsFooter
          nextCheckMs={running ? null : (view?.nextCheckMs ?? null)}
          nowMs={nowMs}
          onOpenSettings={openSettings}
        />
      </div>
      {menu !== null && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={depsMenuItems(menu.row, {
            openUrl,
            copy,
            expand: () => {
              const target = menu.row;
              const group = report?.groups.find((g) => g.rows.includes(target));
              if (group !== undefined) {
                setExpandedRows((s) => new Set(s).add(`${group.manifestPath}\u0001${rowKey(target)}`));
              }
            },
          })}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}
