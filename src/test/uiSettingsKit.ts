/** Shared fixtures + mount helper for the `useUiSettings` suites
 *  (`useUiSettings.test.tsx` = the state machine, `useUiSettings.writePath.test.tsx`
 *  = P69b's persist path). Extracted when the second suite pushed the first
 *  past the 500-line limit: one `HYDRATED` literal for both, so a new
 *  `UiSettings` field breaks exactly one place.
 *
 *  House pattern (see useUpdateController.test.tsx): the `dom` vitest project
 *  runs with VITE_MOCK_IPC=1, so `ipc` IS `mockIpc` — spy on it directly. */
import { vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { useUiSettings } from '../hooks/useUiSettings';
import type { ToastTone } from '../components/Toasts';
import type { GraphPrefs, UiSettings } from '../ipc';

/** An externally-settled promise, for holding a write in flight. */
export function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** A complete non-default `UiSettings`, used both as the resolved value of the
 *  mocked write and as the hydration payload. Every field differs from the
 *  hook's own defaults so hydration assertions cannot pass vacuously. A new
 *  UiSettings field breaks this literal at compile time — on purpose. */
export const HYDRATED: UiSettings = {
  theme: 'light',
  paneWidths: { sidebar: 300, rightPanel: 420 },
  listView: 'flat',
  panelDensity: 'compact',
  primaryCommitAction: 'commitPush',
  autoFetch: { enabled: true, intervalMinutes: 11 },
  healthRefresh: { enabled: true, intervalMinutes: 45 },
  graph: {
    avatarRadius: 14,
    rowHeight: 40,
    laneWidth: 22,
    showSha: false,
    showAuthor: true,
    showDate: false,
    dateBasis: 'committer',
    showAheadBehind: false,
    compact: true,
    showSignatureBadge: false,
    showPrBadge: true,
    showCiStatus: true,
  },
  aiEnabled: false,
  aiConflictAutonomy: 'autoResolve',
  aiConsented: true,
  mcpConsented: true,
  mcpWriteConsented: true,
  onboardingSeen: true,
  autoCheckUpdates: true,
  profiles: [
    { id: 'p1', label: 'Work', userName: 'A Dev', userEmail: 'dev@example.com', signingKey: null },
  ],
  terminalCommand: 'wt.exe -d {path}',
  editorCommand: 'code {path}',
  // P68g: every one of these is now UI-reachable, so hydration must seed them all.
  aiIdleTimeoutSecs: 120,
  aiHardCapSecs: 900,
  aiMaxTurns: 9,
  aiStreamLog: false,
  aiIncludePartialMessages: true,
  aiConflictTools: 'none',
  aiBulkMaxBytes: 200_000,
  aiMaxBudgetUsd: 3,
  aiDockHeight: 320,
  aiDockCollapsed: true,
};

/** A graph patch that differs from the hook's defaults in a few knobs. */
export const GRAPH_PATCH: GraphPrefs = { ...HYDRATED.graph, rowHeight: 36 };

/** Stable toast pusher — identity must not churn, or the referential-stability
 *  behaviour is untestable. */
export function mountUiSettings(push: (tone: ToastTone, text: string) => void = vi.fn()) {
  return { push, ...renderHook(() => useUiSettings(push)) };
}
