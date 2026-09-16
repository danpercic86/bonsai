/**
 * P113 shared fixtures + render helper for the `SettingsPanel` container tests
 * (split per the ~500-line rule, the `aiDockKit.tsx` / `identityKit.tsx` idiom:
 * `SettingsPanel.test.tsx` was 526 lines, and the §7 note-lifetime suite moved
 * to its own file, which needs the same harness).
 *
 * Lives in `src/test/` so it stays out of coverage. Data + one render helper
 * only: every assertion stays in the test file that owns its concern.
 */
import { vi } from 'vitest';
import { render } from '@testing-library/react';

import { SettingsPanel } from '../components/SettingsPanel';
import type { ReactNode } from 'react';
import type { SettingsPanelProps } from '../components/SettingsPanel';
import type { GraphPrefs } from '../ipc';
import type { AiRunPrefs } from '../settings/aiRunPrefs';

const GRAPH: GraphPrefs = {
  avatarRadius: 9,
  rowHeight: 28,
  laneWidth: 14,
  showSha: true,
  showAuthor: false,
  showDate: true,
  dateBasis: 'author',
  showAheadBehind: true,
  compact: false,
  showSignatureBadge: true,
  showPrBadge: false,
  showCiStatus: false,
};

/** P68g: the shipped AI-run defaults, including the two LOCKED zeros. */
const AI_RUN: AiRunPrefs = {
  aiConflictTools: 'readOnly',
  aiStreamLog: true,
  aiIncludePartialMessages: false,
  aiIdleTimeoutSecs: 300,
  aiHardCapSecs: 0,
  aiMaxTurns: 6,
  aiMaxBudgetUsd: 0,
  aiBulkMaxBytes: 400_000,
};

// P69d: this suite needs no `resetEffectiveIdentityForTests` ONLY because `repoPath`
// is null below — the Git-config and profiles sections then issue no IPC and write
// nothing into the module-level identity cache. Set a repo path here and this suite
// starts leaking store state between tests; reset it in a beforeEach at that point.
//
// P69g: Settings is a two-pane shell that renders ONE category at a time, so every
// test below names the category its control lives in via `initialCategory`. That is
// a genuine behaviour change, not a weakened assertion — the control is now behind
// one rail click. `initialCategory` rather than clicking the tab because several
// tests render the panel two or three times, which would make `getByRole('tab')`
// ambiguous; the rail-click path itself is covered in SettingsShell.test.tsx.
// `before` renders as a sibling AHEAD of the panel; only the effect-ordering
// case below passes it, and the order is the whole point (see that case).
export function renderPanel(over: Partial<SettingsPanelProps> = {}, before: ReactNode = null) {
  const props: SettingsPanelProps = {
    open: true,
    onClose: vi.fn(),
    requestSeq: 0,
    // P113 §17.3 — the idle shape: no MCP outcome, a silent announcer, and a
    // settings write that is not failing.
    mcpOutcomes: new Map(),
    mcpAnnounce: '',
    onResetMcpOutcomes: vi.fn(),
    settingsSaveFailed: false,
    onRetrySettingsSave: vi.fn(),
    theme: 'dark',
    listView: 'flat',
    panelDensity: 'cozy',
    primaryCommitAction: 'commit',
    autoFetch: { enabled: true, intervalMinutes: 10 },
    healthRefresh: { enabled: false, intervalMinutes: 30 },
    graph: GRAPH,
    graphStyle: 'standard',
    graphSeason: 'living',
    graphFirstParent: false,
    graphFoldLinear: false,
    graphMinimapAlwaysShow: false,
    graphColorMode: 'lane',
    graphRefFilter: null,
    onChange: vi.fn(),
    onToggleTheme: vi.fn(),
    onToggleListView: vi.fn(),
    aiEnabled: false,
    aiConflictAutonomy: 'proposeReview',
    aiConsented: false,
    aiAvailability: null,
    onRequestEnableAi: vi.fn(),
    aiRun: AI_RUN,
    mcpStatus: null,
    mcpConsented: false,
    onSetMcpEnabled: vi.fn(),
    onRequestEnableMcp: vi.fn(),
    mcpWriteConsented: false,
    onSetMcpAllowWrite: vi.fn(),
    onRequestEnableMcpWrite: vi.fn(),
    repoPath: null,
    profiles: [],
    // P112 §16.4a: the picker's two selections + the Browse flow's adopt.
    terminalTool: '',
    editorTool: '',
    onAdoptToolSelection: vi.fn(),
    dev: {
      enabled: false,
      level: 'debug',
      captureIpc: true,
      captureReact: true,
      captureFrames: false,
      includeRawNames: false,
    },
    onRegisterMcp: vi.fn(async () => {}),
    onShowOnboarding: vi.fn(),
    onOpenRepository: vi.fn(),
    updateCurrentVersion: '1.2.3',
    autoCheckUpdates: true,
    updateState: { status: 'idle' },
    onCheckUpdate: vi.fn(),
    onOpenUpdateDialog: vi.fn(),
    ...over,
  };
  return {
    ...render(
      <>
        {before}
        <SettingsPanel {...props} />
      </>,
    ),
    props,
  };
}
