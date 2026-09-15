/**
 * P69g — the two fixtures the DOM↔catalog guard renders
 * (`settingsCatalog.coverage.test.tsx`, Amendment A §AM-4a).
 *
 * Data only, in its own module per AM-7: the assertions live in the test file and
 * this is the table they run against, so neither grows the other.
 *
 * MAXIMAL turns every gate ON and pushes every resettable knob OFF its default —
 * that is what makes the `↺` half of the guard a real check rather than a
 * uniformly-absent one. MINIMAL is every gate OFF at exactly the production
 * defaults, so `↺` must be absent everywhere.
 */
import { cloneDefaultUiSettings } from '../../settings/defaults';
import type { ConfigView, IdentityProfile, McpStatus } from '../../ipc';
import type { SettingsPanelProps } from './useSettingsPanelAdapter';

/** Stable ids: the guard asserts the rendered profile-id SET, not a bare count. */
export const FIXTURE_PROFILES: readonly IdentityProfile[] = [
  {
    id: 'p-1',
    label: 'Work',
    userName: 'Ada Lovelace',
    userEmail: 'work@bonsai.dev',
    signingKey: null,
  },
  {
    id: 'p-2',
    label: 'Personal',
    userName: 'Ada Lovelace',
    userEmail: 'ada@home.dev',
    signingKey: 'ABC123',
  },
];

const MCP_RUNNING: McpStatus = {
  enabled: true,
  allowWrite: true,
  port: 8765,
  url: 'http://127.0.0.1:8765/mcp',
  token: 'tok-123',
  toolCount: 34,
};

const MCP_STOPPED: McpStatus = {
  enabled: false,
  allowWrite: false,
  port: 0,
  url: '',
  token: '',
  toolCount: 0,
};

/**
 * AM-8 obligation 2: at least one CURATED and one CUSTOM key, so the `'group'`
 * rows have controls inside them (FAIL-K) the moment `git-config` is migrated.
 */
export const FIXTURE_CONFIG_VIEW: ConfigView = {
  targetLevel: 'local',
  curated: [
    {
      key: 'user.name',
      kind: 'text',
      enumValues: [],
      effectiveValue: 'Ada Lovelace',
      effectiveLevel: 'local',
      targetValue: 'Ada Lovelace',
    },
    {
      key: 'user.email',
      kind: 'text',
      enumValues: [],
      effectiveValue: 'work@bonsai.dev',
      effectiveLevel: 'local',
      targetValue: 'work@bonsai.dev',
    },
    {
      key: 'pull.rebase',
      kind: 'enum',
      enumValues: ['true', 'false'],
      effectiveValue: 'true',
      effectiveLevel: 'local',
      targetValue: 'true',
    },
  ],
  advanced: [{ name: 'custom.thing', value: 'v1', level: 'local' }],
};

/** The value half of the fixtures — everything `SettingsPanelProps` carries. */
type FixtureValues = Omit<
  SettingsPanelProps,
  | 'open'
  | 'onClose'
  | 'initialCategory'
  | 'requestSeq'
  | 'onChange'
  | 'onToggleTheme'
  | 'onToggleListView'
  | 'onRequestEnableAi'
  | 'onSetMcpEnabled'
  | 'onRequestEnableMcp'
  | 'onSetMcpAllowWrite'
  | 'onRequestEnableMcpWrite'
  | 'onRegisterMcp'
  | 'onShowOnboarding'
  | 'onOpenRepository'
  | 'onCheckUpdate'
  | 'onOpenUpdateDialog'
>;

const D = cloneDefaultUiSettings();

/** P113 §17.3 — the four call sites the phase-1 sweep missed arrive as props, so
 *  both fixtures carry the idle shape: no MCP outcome, a silent announcer, and a
 *  settings write that is not failing. Every P69-era expectation in the suites
 *  that use these fixtures therefore holds byte for byte. */
const NO_OUTCOMES = {
  mcpOutcomes: new Map(),
  mcpAnnounce: '',
  settingsSaveFailed: false,
  onRetrySettingsSave: () => {},
} as const;

/** P112 §16.4a — the Browse flow's non-writing adopt. A handler, but it lives in
 *  the VALUE fixture (the `onRetrySettingsSave` precedent above) so the six
 *  suites that spread `MINIMAL`/`MAXIMAL` need no edit to keep compiling. */
const NO_ADOPT = { onAdoptToolSelection: () => {} } as const;

export const MINIMAL: FixtureValues = {
  theme: D.theme,
  listView: D.listView,
  panelDensity: D.panelDensity,
  primaryCommitAction: D.primaryCommitAction,
  autoFetch: D.autoFetch,
  healthRefresh: D.healthRefresh,
  graph: D.graph,
  // Spec-002: optional in UiSettings, so `cloneDefaultUiSettings()` omits them —
  // the fixture supplies the concrete defaults the SettingsPanel props require.
  graphStyle: 'standard',
  graphSeason: 'living',
  // Spec-003/004: same optional-in-UiSettings treatment.
  graphFirstParent: false,
  graphFoldLinear: false,
  // Spec-005: overview-rail always-show, same treatment.
  graphMinimapAlwaysShow: false,
  // Spec-006: edge/ring coloring, same treatment.
  graphColorMode: 'lane',
  graphRefFilter: null,
  aiEnabled: false,
  aiConflictAutonomy: D.aiConflictAutonomy,
  aiConsented: false,
  aiAvailability: null,
  aiRun: {
    aiConflictTools: D.aiConflictTools,
    aiStreamLog: D.aiStreamLog,
    aiIncludePartialMessages: D.aiIncludePartialMessages,
    aiIdleTimeoutSecs: D.aiIdleTimeoutSecs,
    aiHardCapSecs: D.aiHardCapSecs,
    aiMaxTurns: D.aiMaxTurns,
    aiMaxBudgetUsd: D.aiMaxBudgetUsd,
    aiBulkMaxBytes: D.aiBulkMaxBytes,
  },
  ...NO_OUTCOMES,
  mcpStatus: MCP_STOPPED,
  mcpConsented: false,
  mcpWriteConsented: false,
  repoPath: null,
  configInitialFocus: null,
  profiles: [],
  // P112: both at the default (`''` = Auto-detect), so no picker ↺ shows.
  terminalTool: '',
  editorTool: '',
  ...NO_ADOPT,
  // P91: Dev mode off at the production defaults, so the fieldset is disabled and
  // no dev ↺ shows in the minimal fixture.
  dev: D.dev,
  updateCurrentVersion: '1.2.3',
  autoCheckUpdates: D.autoCheckUpdates,
  updateState: { status: 'idle' },
};

export const MAXIMAL: FixtureValues = {
  ...MINIMAL,
  theme: 'light',
  listView: 'flat',
  panelDensity: 'compact',
  // P80 D1: OFF the default ('commit'), so `↺` must be PRESENT on the row.
  primaryCommitAction: 'commitPush',
  // Every resettable knob OFF its default, so `↺` must be PRESENT on each.
  autoFetch: { enabled: true, intervalMinutes: 11 },
  healthRefresh: { enabled: true, intervalMinutes: 17 },
  graph: {
    ...D.graph,
    avatarRadius: 6,
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
  // Spec-003: first-parent OFF its default (↺ present); a live ref filter so the
  // Branch-filters Clear button renders enabled.
  graphFirstParent: true,
  // Spec-004: fold ON its default too (↺ present on the fold row).
  graphFoldLinear: true,
  // Spec-005: rail always-show ON its default (↺ present on the row).
  graphMinimapAlwaysShow: true,
  // Spec-006: author coloring OFF its default.
  graphColorMode: 'author',
  graphRefFilter: { mode: 'solo', refs: ['refs/heads/main'] },
  aiEnabled: true,
  aiConsented: true,
  aiConflictAutonomy: 'autoResolve',
  aiAvailability: { installed: true, loggedIn: true, version: '1.0.0', detail: 'claude 1.0.0' },
  aiRun: {
    aiConflictTools: 'none',
    aiStreamLog: false,
    aiIncludePartialMessages: true,
    aiIdleTimeoutSecs: 120,
    aiHardCapSecs: 900,
    aiMaxTurns: 9,
    aiMaxBudgetUsd: 2.5,
    aiBulkMaxBytes: 200_000,
  },
  ...NO_OUTCOMES,
  mcpStatus: MCP_RUNNING,
  mcpConsented: true,
  mcpWriteConsented: true,
  repoPath: '/repo/fixture',
  profiles: [...FIXTURE_PROFILES],
  // P112: both OFF the default, so `↺` must be PRESENT on both picker rows —
  // which is the half of the guard a uniformly-absent ↺ could not check. Ids,
  // never program strings: `windows-terminal` and `vscode` are catalog ids.
  terminalTool: 'windows-terminal',
  editorTool: 'vscode',
  autoCheckUpdates: true,
  // P91: Dev mode ON with every resettable dev knob off its default, so each dev
  // row's ↺ is present. `level: 'info'` (not 'trace') keeps Frame timing a live
  // switch rather than entangling the trace force-on with the ↺ presence check.
  dev: {
    enabled: true,
    level: 'info',
    captureIpc: false,
    captureReact: false,
    captureFrames: true,
    includeRawNames: true,
  },
};
