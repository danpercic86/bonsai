// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import { AUTO_FETCH_INTERVAL_MAX, AUTO_FETCH_INTERVAL_MIN, AVATAR_RADIUS_MAX, AVATAR_RADIUS_MIN, HEALTH_REFRESH_INTERVAL_MAX, HEALTH_REFRESH_INTERVAL_MIN, LANE_WIDTH_MAX, LANE_WIDTH_MIN, ROW_HEIGHT_MAX, ROW_HEIGHT_MIN } from '../../settings/ranges';
import type { AiAutonomy, AutoFetchSettings, GraphDateBasis, GraphPrefs, HealthRefreshSettings, IdentityProfile, ListView, PaneWidths, PanelDensity, RecentRepo, SessionState, Theme, UiSettings } from '../types';

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

export const DEFAULT_UI_SETTINGS: UiSettings = {
  theme: 'dark',
  paneWidths: { sidebar: 240, rightPanel: 380 },
  listView: 'tree',
  // P67 §4: right-panel density; 'cozy' is the tightened default.
  panelDensity: 'cozy',
  autoFetch: { enabled: false, intervalMinutes: 5 },
  // P30: backend-scheduler healthRefresh signal; disabled by default.
  healthRefresh: { enabled: false, intervalMinutes: 30 },
  // P51: geometry knobs + per-row detail toggles (defaults mirror settings.rs
  // GraphPrefs::default — compact off, SHA/date/ahead-behind on).
  graph: {
    avatarRadius: 10,
    rowHeight: 32,
    laneWidth: 16,
    showSha: true,
    showAuthor: false,
    showDate: true,
    dateBasis: 'author',
    showAheadBehind: true,
    compact: false,
    // P58c: signature badge on by default (mirrors GraphPrefs::default).
    showSignatureBadge: true,
    // P63: forge signal badges off by default (mirrors GraphPrefs::default).
    showPrBadge: false,
    showCiStatus: false,
  },
  // AI assistance (P13): enabled by default, but consent gates the feature.
  aiEnabled: true,
  aiConflictAutonomy: 'proposeReview',
  aiConsented: false,
  // Embedded MCP server (P16): consent gates the enable toggle.
  mcpConsented: false,
  // MCP write consent (P16c): a separate, stronger gate for the write toggle.
  mcpWriteConsented: false,
  // P43: onboarding unseen by default so a fresh browser harness shows it.
  onboardingSeen: false,
  // P42: auto-check-updates-on-launch OFF by default (privacy / opt-in).
  autoCheckUpdates: false,
  // P44: two seeded identity profiles so the harness shows a populated list
  // and Apply is exercisable (fixed string ids).
  profiles: [
    {
      id: 'mock-work',
      label: 'Work',
      userName: 'Mock Fixture User',
      userEmail: 'work@bonsai.dev',
      signingKey: null,
    },
    {
      id: 'mock-personal',
      label: 'Personal',
      userName: 'Mock Personal',
      userEmail: 'me@personal.dev',
      signingKey: 'ABC123',
    },
  ],
  // P49: external-tool templates default to "" ⇒ per-OS auto-detect.
  terminalCommand: '',
  editorCommand: '',
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
  return valid.length > 0 ? valid : null;
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
    // P49 external-tool templates (additive): fall back to default ("").
    const terminalCommand =
      typeof parsed.terminalCommand === 'string'
        ? parsed.terminalCommand
        : DEFAULT_UI_SETTINGS.terminalCommand;
    const editorCommand =
      typeof parsed.editorCommand === 'string'
        ? parsed.editorCommand
        : DEFAULT_UI_SETTINGS.editorCommand;
    return {
      theme,
      paneWidths,
      listView,
      panelDensity,
      autoFetch,
      healthRefresh,
      graph,
      aiEnabled,
      aiConflictAutonomy,
      aiConsented,
      mcpConsented,
      mcpWriteConsented,
      onboardingSeen,
      autoCheckUpdates,
      profiles,
      terminalCommand,
      editorCommand,
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
