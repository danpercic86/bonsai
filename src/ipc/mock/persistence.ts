// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import { AUTO_FETCH_INTERVAL_MAX, AUTO_FETCH_INTERVAL_MIN, AVATAR_RADIUS_MAX, AVATAR_RADIUS_MIN, HEALTH_REFRESH_INTERVAL_MAX, HEALTH_REFRESH_INTERVAL_MIN, LANE_WIDTH_MAX, LANE_WIDTH_MIN, ROW_HEIGHT_MAX, ROW_HEIGHT_MIN } from '../../settings/ranges';
import { DEFAULT_UI_SETTINGS as PRODUCTION_DEFAULT_UI_SETTINGS } from '../../settings/defaults';
import { parseAiRunSettings } from './aiRunSettings';
import type { AiAutonomy, AutoFetchSettings, DevSettings, GraphColorMode, GraphDateBasis, GraphPrefs, GraphRefFilter, GraphSeason, GraphStyle, HealthRefreshSettings, IdentityProfile, ListView, LogLevel, PaneWidths, PanelDensity, PrimaryCommitAction, ProfileColor, RecentRepo, SessionState, Theme, UiSettings } from '../types';

/** P91 §3: the closed log-level set (mirrors Rust `LogLevel`). */
const LOG_LEVELS: ReadonlySet<string> = new Set<LogLevel>([
  'error',
  'warn',
  'info',
  'debug',
  'trace',
]);

/** Spec-002: closed enum guards for the two additive graph-theme prefs. */
const GRAPH_SEASONS: ReadonlySet<string> = new Set<GraphSeason>(['living', 'spring', 'autumn']);

/** P82: the closed palette (mirrors Rust `ProfileColor`). Used to validate a
 *  persisted profile's `color` field — an invalid value normalizes to neutral. */
const PROFILE_COLOR_SET: ReadonlySet<string> = new Set<ProfileColor>([
  'neutral',
  'slate',
  'blue',
  'teal',
  'green',
  'amber',
  'orange',
  'purple',
  'pink',
]);

// Recents persistence (P1 contract §3.4): localStorage-backed so the harness
// reopen-on-launch story is verifiable — open once, reload, auto-reopen.
const RECENTS_KEY = 'bonsai.mockRecents';
const MAX_RECENTS = 10;

/** Corrupt/missing storage degrades to [] — mirrors the backend's load_from. */
export function readRecents(): RecentRepo[] {
  try {
    const raw = window.localStorage.getItem(RECENTS_KEY);
    if (raw === null) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (r): r is RecentRepo =>
        typeof r === 'object' &&
        r !== null &&
        typeof (r as RecentRepo).path === 'string' &&
        typeof (r as RecentRepo).lastOpened === 'number',
    );
  } catch {
    return [];
  }
}

export function writeRecents(list: RecentRepo[]): void {
  try {
    window.localStorage.setItem(RECENTS_KEY, JSON.stringify(list));
  } catch {
    // Best-effort, like the backend's non-fatal save.
  }
}

// Session persistence (P3e contract §6/§8.1): localStorage-backed like recents /
// ui-settings so reopen-all survives a harness reload.
const SESSION_KEY = 'bonsai.mockSession';

/** Corrupt/missing storage degrades to an empty session — mirrors load_from. */
export function readSession(): SessionState {
  try {
    const raw = window.localStorage.getItem(SESSION_KEY);
    if (raw === null) return { openRepos: [], activeRepo: null };
    const parsed = JSON.parse(raw) as Partial<SessionState>;
    const openRepos = Array.isArray(parsed.openRepos)
      ? parsed.openRepos.filter((r): r is string => typeof r === 'string')
      : [];
    const activeRepo = typeof parsed.activeRepo === 'string' ? parsed.activeRepo : null;
    return { openRepos, activeRepo };
  } catch {
    return { openRepos: [], activeRepo: null };
  }
}

export function writeSession(session: SessionState): void {
  try {
    window.localStorage.setItem(SESSION_KEY, JSON.stringify(session));
  } catch {
    // Best-effort, like the backend's non-fatal save.
  }
}

// UI settings persistence (P2a contract §2.4): mirrors bonsai.mockRecents —
// localStorage-backed so the harness drag/toggle-then-reload story is
// verifiable. Ranges mirror settings.rs's clamp_pane_widths — the ONE place
// the mock duplicates a Rust-side clamp, acceptable because it's a pure
// numeric guard, not git/layout logic (contract §2.4).
const UI_SETTINGS_KEY = 'bonsai.mockUiSettings';
const SIDEBAR_MIN = 180;
const SIDEBAR_MAX = 480;
const RIGHT_PANEL_MIN = 280;
const RIGHT_PANEL_MAX = 640;

/**
 * The MOCK's default seed = the PRODUCTION defaults (`src/settings/defaults.ts`,
 * pinned to the Rust oracle) + harness-only fixture data.
 *
 * P69 §3.3: this composes rather than re-exports because the two genuinely
 * differ, and the divergence list must stay short, explicit and reviewed:
 *
 *   - `profiles` — production (and Rust) default to an EMPTY list; the harness
 *     seeds two fixed-id profiles so the identity list is populated and both
 *     the header identity menu (P69i) and the pane's "Use in this repository"
 *     action are exercisable in the browser.
 *
 * That is the ONLY permitted divergence; `src/settings/defaults.test.ts` iterates
 * every other key and asserts equality, so adding a second one fails there.
 */
export const DEFAULT_UI_SETTINGS: UiSettings = {
  ...structuredClone(PRODUCTION_DEFAULT_UI_SETTINGS),
  // P44: two seeded identity profiles so the harness shows a populated list
  // and Apply is exercisable (fixed string ids).
  profiles: [
    {
      id: 'mock-work',
      label: 'Work',
      userName: 'Mock Fixture User',
      userEmail: 'work@bonsai.dev',
      signingKey: null,
      // P82: distinct seeded colors so the harness demonstrates the feature.
      color: 'blue',
    },
    {
      id: 'mock-personal',
      label: 'Personal',
      userName: 'Mock Personal',
      userEmail: 'me@personal.dev',
      signingKey: 'ABC123',
      color: 'green',
    },
  ],
};

export function clampPaneWidths(w: PaneWidths): PaneWidths {
  return {
    sidebar: Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, w.sidebar)),
    rightPanel: Math.min(RIGHT_PANEL_MAX, Math.max(RIGHT_PANEL_MIN, w.rightPanel)),
  };
}

/** Mirrors Rust `clamp_auto_fetch` (settings.rs). */
export function clampAutoFetch(a: AutoFetchSettings): AutoFetchSettings {
  return {
    enabled: a.enabled,
    intervalMinutes: Math.min(
      AUTO_FETCH_INTERVAL_MAX,
      Math.max(AUTO_FETCH_INTERVAL_MIN, a.intervalMinutes),
    ),
  };
}

/** Mirrors Rust `clamp_health_refresh` (settings.rs, P30). */
export function clampHealthRefresh(h: HealthRefreshSettings): HealthRefreshSettings {
  return {
    enabled: h.enabled,
    intervalMinutes: Math.min(
      HEALTH_REFRESH_INTERVAL_MAX,
      Math.max(HEALTH_REFRESH_INTERVAL_MIN, h.intervalMinutes),
    ),
  };
}

/** Mirrors Rust `clamp_graph_prefs` (settings.rs): clamps the geometry knobs;
 *  the P51 detail toggles + `dateBasis` pass through unclamped (spread). */
export function clampGraphPrefs(g: GraphPrefs): GraphPrefs {
  return {
    ...g, // toggles + dateBasis pass through unclamped
    avatarRadius: Math.min(AVATAR_RADIUS_MAX, Math.max(AVATAR_RADIUS_MIN, g.avatarRadius)),
    rowHeight: Math.min(ROW_HEIGHT_MAX, Math.max(ROW_HEIGHT_MIN, g.rowHeight)),
    laneWidth: Math.min(LANE_WIDTH_MAX, Math.max(LANE_WIDTH_MIN, g.laneWidth)),
  };
}

/** Per-element validation for persisted identity profiles (mirrors readRecents):
 *  keep only objects carrying the required IdentityProfile string fields
 *  (`signingKey` may be null). Returns null when the input is not an array or
 *  when a NON-empty array yields no survivors (all corrupt) — the caller falls
 *  back to defaults. A legitimately empty list stays empty (the user deleted
 *  all profiles; don't resurrect the seeds). Pure. */
export function sanitizeProfiles(raw: unknown): IdentityProfile[] | null {
  if (!Array.isArray(raw)) return null;
  if (raw.length === 0) return [];
  const valid = raw.filter(
    (p): p is IdentityProfile =>
      typeof p === 'object' &&
      p !== null &&
      typeof (p as IdentityProfile).id === 'string' &&
      typeof (p as IdentityProfile).label === 'string' &&
      typeof (p as IdentityProfile).userName === 'string' &&
      typeof (p as IdentityProfile).userEmail === 'string' &&
      ((p as IdentityProfile).signingKey === null ||
        typeof (p as IdentityProfile).signingKey === 'string'),
  );
  if (valid.length === 0) return null;
  // P82: `color` is non-essential — normalize (never reject) a per-element value.
  // Missing color stays undefined (read as neutral); an invalid color coerces to
  // 'neutral' rather than dropping the whole profile.
  return valid.map((p) => {
    const raw = p.color;
    if (raw === undefined) return p;
    return PROFILE_COLOR_SET.has(raw) ? p : { ...p, color: 'neutral' as ProfileColor };
  });
}

/** Spec-003: shape-check a persisted `graphRefFilter` intent — mode must be
 *  'solo' | 'hide' and refs a string array (non-string entries dropped);
 *  anything else (incl. a malformed object) degrades to null. Pure. */
export function sanitizeGraphRefFilter(raw: unknown): GraphRefFilter | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const { mode, refs } = raw as { mode?: unknown; refs?: unknown };
  if (mode !== 'solo' && mode !== 'hide') return null;
  if (!Array.isArray(refs)) return null;
  return { mode, refs: refs.filter((r): r is string => typeof r === 'string') };
}

/** P91 §10: shape-check a persisted `dev` object. Every field falls back to the
 *  production default independently (mirrors the Rust per-field
 *  `#[serde(default)]`), so a pre-P91 blob, a missing key or a garbled object all
 *  read back as Dev mode OFF with strict redaction — never a partial struct and
 *  never a throw. Pure. */
export function sanitizeDevSettings(raw: unknown): DevSettings {
  const d = (typeof raw === 'object' && raw !== null ? raw : {}) as Partial<DevSettings>;
  const fallback = DEFAULT_UI_SETTINGS.dev;
  const level: LogLevel =
    typeof d.level === 'string' && LOG_LEVELS.has(d.level) ? (d.level as LogLevel) : fallback.level;
  return {
    enabled: typeof d.enabled === 'boolean' ? d.enabled : fallback.enabled,
    level,
    captureIpc: typeof d.captureIpc === 'boolean' ? d.captureIpc : fallback.captureIpc,
    captureReact: typeof d.captureReact === 'boolean' ? d.captureReact : fallback.captureReact,
    captureFrames: typeof d.captureFrames === 'boolean' ? d.captureFrames : fallback.captureFrames,
    includeRawNames:
      typeof d.includeRawNames === 'boolean' ? d.includeRawNames : fallback.includeRawNames,
  };
}

/** Corrupt/missing storage degrades to the default — mirrors load_from. */
export function readUiSettings(): UiSettings {
  try {
    const raw = window.localStorage.getItem(UI_SETTINGS_KEY);
    if (raw === null) return structuredClone(DEFAULT_UI_SETTINGS);
    const parsed = JSON.parse(raw) as Partial<UiSettings>;
    const theme: Theme = parsed.theme === 'light' ? 'light' : 'dark';
    const paneWidths = clampPaneWidths({
      sidebar:
        typeof parsed.paneWidths?.sidebar === 'number'
          ? parsed.paneWidths.sidebar
          : DEFAULT_UI_SETTINGS.paneWidths.sidebar,
      rightPanel:
        typeof parsed.paneWidths?.rightPanel === 'number'
          ? parsed.paneWidths.rightPanel
          : DEFAULT_UI_SETTINGS.paneWidths.rightPanel,
    });
    const listView: ListView = parsed.listView === 'flat' ? 'flat' : 'tree';
    const panelDensity: PanelDensity = parsed.panelDensity === 'compact' ? 'compact' : 'cozy';
    // P80 D1: primary commit action (additive); fall back to default ('commit').
    const primaryCommitAction: PrimaryCommitAction =
      parsed.primaryCommitAction === 'commitPush' ? 'commitPush' : 'commit';
    const autoFetch = clampAutoFetch({
      enabled:
        typeof parsed.autoFetch?.enabled === 'boolean'
          ? parsed.autoFetch.enabled
          : DEFAULT_UI_SETTINGS.autoFetch.enabled,
      intervalMinutes:
        typeof parsed.autoFetch?.intervalMinutes === 'number'
          ? parsed.autoFetch.intervalMinutes
          : DEFAULT_UI_SETTINGS.autoFetch.intervalMinutes,
    });
    // P30 healthRefresh (additive, like autoFetch): fall back to defaults.
    const healthRefresh = clampHealthRefresh({
      enabled:
        typeof parsed.healthRefresh?.enabled === 'boolean'
          ? parsed.healthRefresh.enabled
          : DEFAULT_UI_SETTINGS.healthRefresh.enabled,
      intervalMinutes:
        typeof parsed.healthRefresh?.intervalMinutes === 'number'
          ? parsed.healthRefresh.intervalMinutes
          : DEFAULT_UI_SETTINGS.healthRefresh.intervalMinutes,
    });
    // P51: geometry + per-row toggles. Each field tolerant-parses independently
    // (mirrors the Rust per-field `#[serde(default)]`): a legacy `graph` object
    // missing a key — or still carrying `dotRadius` — falls back to the default.
    const g = parsed.graph;
    const dateBasis: GraphDateBasis = g?.dateBasis === 'committer' ? 'committer' : 'author';
    const graph = clampGraphPrefs({
      avatarRadius:
        typeof g?.avatarRadius === 'number'
          ? g.avatarRadius
          : DEFAULT_UI_SETTINGS.graph.avatarRadius,
      rowHeight:
        typeof g?.rowHeight === 'number' ? g.rowHeight : DEFAULT_UI_SETTINGS.graph.rowHeight,
      laneWidth:
        typeof g?.laneWidth === 'number' ? g.laneWidth : DEFAULT_UI_SETTINGS.graph.laneWidth,
      showSha: typeof g?.showSha === 'boolean' ? g.showSha : DEFAULT_UI_SETTINGS.graph.showSha,
      showAuthor:
        typeof g?.showAuthor === 'boolean' ? g.showAuthor : DEFAULT_UI_SETTINGS.graph.showAuthor,
      showDate: typeof g?.showDate === 'boolean' ? g.showDate : DEFAULT_UI_SETTINGS.graph.showDate,
      dateBasis,
      showAheadBehind:
        typeof g?.showAheadBehind === 'boolean'
          ? g.showAheadBehind
          : DEFAULT_UI_SETTINGS.graph.showAheadBehind,
      compact: typeof g?.compact === 'boolean' ? g.compact : DEFAULT_UI_SETTINGS.graph.compact,
      // P58c: legacy `graph` object without the key ⇒ default true.
      showSignatureBadge:
        typeof g?.showSignatureBadge === 'boolean'
          ? g.showSignatureBadge
          : DEFAULT_UI_SETTINGS.graph.showSignatureBadge,
      // P63: legacy `graph` object without the forge-badge keys ⇒ default false.
      showPrBadge:
        typeof g?.showPrBadge === 'boolean'
          ? g.showPrBadge
          : DEFAULT_UI_SETTINGS.graph.showPrBadge,
      showCiStatus:
        typeof g?.showCiStatus === 'boolean'
          ? g.showCiStatus
          : DEFAULT_UI_SETTINGS.graph.showCiStatus,
    });
    // Spec-002 (additive): commit-graph style + season. Fall back to the shared
    // defaults when the blob omits/garbles them — same as every other additive
    // field below, so a wrong-shape or missing blob reads back as the complete
    // DEFAULT_UI_SETTINGS (the keys are pinned in the defaults oracle + native
    // settings). Season is ignored while graphStyle === 'standard'.
    const graphStyle: GraphStyle =
      parsed.graphStyle === 'bonsai' || parsed.graphStyle === 'standard'
        ? parsed.graphStyle
        : (DEFAULT_UI_SETTINGS.graphStyle ?? 'standard');
    const graphSeason: GraphSeason =
      typeof parsed.graphSeason === 'string' && GRAPH_SEASONS.has(parsed.graphSeason)
        ? (parsed.graphSeason as GraphSeason)
        : (DEFAULT_UI_SETTINGS.graphSeason ?? 'living');
    // Spec-003 (additive): first-parent toggle + solo/hide intent. Malformed
    // graphRefFilter degrades to null (no ref filter), never throws.
    const graphFirstParent = parsed.graphFirstParent === true;
    // Spec-004 (additive): fold-linear toggle. Malformed → false, never throws.
    const graphFoldLinear = parsed.graphFoldLinear === true;
    // Spec-005 (additive): always-show overview rail. Malformed → false.
    const graphMinimapAlwaysShow = parsed.graphMinimapAlwaysShow === true;
    // Spec-006 (additive): edge/ring coloring. Malformed → 'lane', never throws.
    const graphColorMode: GraphColorMode =
      parsed.graphColorMode === 'author' || parsed.graphColorMode === 'lane'
        ? parsed.graphColorMode
        : 'lane';
    const graphRefFilter = sanitizeGraphRefFilter(parsed.graphRefFilter);
    // P13 AI fields (additive, like autoFetch/graph): fall back to defaults.
    const aiEnabled =
      typeof parsed.aiEnabled === 'boolean' ? parsed.aiEnabled : DEFAULT_UI_SETTINGS.aiEnabled;
    const aiConflictAutonomy: AiAutonomy =
      parsed.aiConflictAutonomy === 'autoResolve' ? 'autoResolve' : 'proposeReview';
    const aiConsented =
      typeof parsed.aiConsented === 'boolean' ? parsed.aiConsented : DEFAULT_UI_SETTINGS.aiConsented;
    // P16 MCP consent (additive, like the AI fields): fall back to default.
    const mcpConsented =
      typeof parsed.mcpConsented === 'boolean'
        ? parsed.mcpConsented
        : DEFAULT_UI_SETTINGS.mcpConsented;
    // P16c MCP write consent (additive): fall back to default.
    const mcpWriteConsented =
      typeof parsed.mcpWriteConsented === 'boolean'
        ? parsed.mcpWriteConsented
        : DEFAULT_UI_SETTINGS.mcpWriteConsented;
    // P43 onboarding seen (additive): fall back to default (false ⇒ show).
    const onboardingSeen =
      typeof parsed.onboardingSeen === 'boolean'
        ? parsed.onboardingSeen
        : DEFAULT_UI_SETTINGS.onboardingSeen;
    // P42 auto-check-updates (additive): fall back to default (false).
    const autoCheckUpdates =
      typeof parsed.autoCheckUpdates === 'boolean'
        ? parsed.autoCheckUpdates
        : DEFAULT_UI_SETTINGS.autoCheckUpdates;
    // P44 identity profiles (additive): validate per-element (like readRecents);
    // degrade to default when absent/malformed or when no element survives.
    const profiles: IdentityProfile[] =
      sanitizeProfiles(parsed.profiles) ?? structuredClone(DEFAULT_UI_SETTINGS.profiles);
    // P112 §5.1 external-tool SELECTIONS (additive): fall back to default ("" =
    // the auto ladder). A blob still carrying the pre-P112 free-text command
    // keys is ignored here on purpose — the real migration is Rust's, it runs
    // once on `load_from`, and mirroring it would be inventing a second one.
    const terminalTool =
      typeof parsed.terminalTool === 'string'
        ? parsed.terminalTool
        : DEFAULT_UI_SETTINGS.terminalTool;
    const editorTool =
      typeof parsed.editorTool === 'string' ? parsed.editorTool : DEFAULT_UI_SETTINGS.editorTool;
    // P68 §8.3 (additive, like the P13 AI fields): per-field tolerant parse +
    // the clamp mirror; a pre-P68 blob loads every default.
    const aiRun = parseAiRunSettings(parsed);
    // P91 §10 (additive, like autoFetch/healthRefresh): per-field tolerant parse
    // so a pre-P91 blob — or a garbled `dev` object — reads back as Dev mode OFF
    // with strict redaction rather than throwing or minting a partial struct.
    const dev = sanitizeDevSettings(parsed.dev);
    return {
      theme,
      paneWidths,
      listView,
      panelDensity,
      primaryCommitAction,
      autoFetch,
      healthRefresh,
      graph,
      graphStyle,
      graphSeason,
      graphFirstParent,
      graphFoldLinear,
      graphMinimapAlwaysShow,
      graphColorMode,
      graphRefFilter,
      aiEnabled,
      aiConflictAutonomy,
      aiConsented,
      mcpConsented,
      mcpWriteConsented,
      onboardingSeen,
      autoCheckUpdates,
      profiles,
      terminalTool,
      editorTool,
      dev,
      ...aiRun,
    };
  } catch {
    return structuredClone(DEFAULT_UI_SETTINGS);
  }
}

export function writeUiSettings(s: UiSettings): void {
  try {
    window.localStorage.setItem(UI_SETTINGS_KEY, JSON.stringify(s));
  } catch {
    // Best-effort, like the backend's non-fatal save.
  }
}
/** Upsert at front, dedupe case-insensitively, cap 10 (mirrors record_recent). */
export function recordRecent(path: string): void {
  const list = readRecents().filter((r) => r.path.toLowerCase() !== path.toLowerCase());
  list.unshift({ path, lastOpened: Math.floor(Date.now() / 1000) });
  writeRecents(list.slice(0, MAX_RECENTS));
}
