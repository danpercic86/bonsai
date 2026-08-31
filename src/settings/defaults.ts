/**
 * P69 §3 — the production `UiSettings` defaults.
 *
 * WHY THIS EXISTS. Until P69 the only defaults table on the frontend lived in
 * `src/ipc/mock/persistence.ts`, i.e. harness-only code. The per-row reset
 * affordance (`↺`, UI §5.7) needs the same numbers in PRODUCTION code, so the
 * table moves here and the mock composes from it (§3.3).
 *
 * AUTHORITY. Rust stays authoritative: this literal is a declared MIRROR of
 * `settings::Settings::default()` as projected by `ui_settings_of`, exactly like
 * `src/settings/ranges.ts` mirrors the Rust clamp constants. What makes the mirror
 * safe rather than a second source of truth is that BOTH sides are pinned to one
 * checked-in artefact — `uiSettingsDefaults.json`:
 *
 *   - `defaults.test.ts` deep-equals this table against the oracle;
 *   - `src-tauri/src/settings_ui_tests.rs` compares the oracle against serde's view
 *     of the Rust defaults (DEFERRED — see the header of `defaults.test.ts`).
 *
 * Change a serde default and the Rust test fails; change this file and vitest
 * fails; change the oracle and both fail until both sides follow.
 */
import type { UiSettings } from '../ipc/types';

/**
 * The production defaults.
 *
 * `profiles` is EMPTY here — Rust defaults it to `Vec::new()`. The mock's two
 * seeded fixture profiles are a harness-only addition layered on top of this
 * constant (see `src/ipc/mock/persistence.ts`); they are NOT a default and must
 * never leak into production reset behaviour.
 *
 * FROZEN, top level and nested: these objects are shared with every reader, so a
 * stray `DEFAULT_UI_SETTINGS.graph.rowHeight = x` would corrupt every later reset.
 * Under ES modules (always strict) such a write throws instead of passing
 * silently. Use {@link cloneDefaultUiSettings} when you need something to mutate.
 */
const DEFAULTS: UiSettings = {
  theme: 'dark',
  paneWidths: { sidebar: 240, rightPanel: 380 },
  listView: 'tree',
  /** P67 §4: `cozy` is the tightened right-panel default. */
  panelDensity: 'cozy',
  /** P80 D1: the always-safe, non-network action is the default primary. */
  primaryCommitAction: 'commit',
  /** P11: auto-fetch OFF, 5-minute interval. */
  autoFetch: { enabled: false, intervalMinutes: 5 },
  /** P30: periodic status/health refresh OFF, 30-minute interval. */
  healthRefresh: { enabled: false, intervalMinutes: 30 },
  /** P51/P58c/P63: geometry equals the canvas METRICS baseline; SHA/date/
   *  ahead-behind/signature on, author/compact/forge badges off. */
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
    showSignatureBadge: true,
    showPrBadge: false,
    showCiStatus: false,
  },
  /** P13: AI features are ON, but `aiConsented: false` gates every call. */
  aiEnabled: true,
  aiConflictAutonomy: 'proposeReview',
  aiConsented: false,
  /** P16/P16c: both MCP consents start unset. */
  mcpConsented: false,
  mcpWriteConsented: false,
  /** P43: a fresh install shows the onboarding tour once. */
  onboardingSeen: false,
  /** P42 D4: update checks are opt-in. */
  autoCheckUpdates: false,
  profiles: [],
  /** P49 / §3.4: `''` means "backend auto-detects per OS" — a real, stable
   *  default, surfaced on the `↺` as `auto-detect`. */
  terminalCommand: '',
  editorCommand: '',
  /** P68 §8.3: the ten streaming AI-run knobs. `aiHardCapSecs: 0` (unbounded)
   *  and `aiMaxBudgetUsd: 0` (no `--max-budget-usd` flag) are LOCKED mode
   *  sentinels, not "unset" — which is why §3.4 gives those rows no `↺`. */
  aiIdleTimeoutSecs: 300,
  aiHardCapSecs: 0,
  aiMaxTurns: 6,
  aiStreamLog: true,
  aiIncludePartialMessages: false,
  aiConflictTools: 'readOnly',
  aiBulkMaxBytes: 400_000,
  aiMaxBudgetUsd: 0,
  aiDockHeight: 180,
  aiDockCollapsed: false,
};

for (const nested of [
  DEFAULTS.paneWidths,
  DEFAULTS.autoFetch,
  DEFAULTS.healthRefresh,
  DEFAULTS.graph,
  DEFAULTS.profiles,
]) {
  Object.freeze(nested);
}

export const DEFAULT_UI_SETTINGS: UiSettings = Object.freeze(DEFAULTS);

/**
 * §3.4 escape hatch — settings whose default is environment- or repo-derived.
 *
 * EMPTY today, and that is the point: `terminalCommand` / `editorCommand` default
 * to `''`, which is a REAL, stable default meaning "auto-detect in the backend",
 * so parity holds for them like any other field. If Rust ever computes a genuinely
 * env-dependent default, that key moves in here AND the Rust parity test drops it
 * from both sides. Adding a key here is a reviewed change, never a silent one.
 */
export const ENV_DERIVED_DEFAULT_KEYS: readonly (keyof UiSettings)[] = [];

/** A fresh, structurally-independent copy — for callers that need to mutate. */
export function cloneDefaultUiSettings(): UiSettings {
  return structuredClone(DEFAULT_UI_SETTINGS);
}
