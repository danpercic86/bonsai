import type { AiAutonomy, AiConflictTools } from './ai';
import type { IdentityProfile } from './config';
import type { GraphPrefs } from './graph';

export type Theme = 'dark' | 'light';

/** Flat vs tree-grouped list rendering (P3b §2) — pure display preference. */
export type ListView = 'tree' | 'flat';

/** P67 §4: right-panel vertical density. Independent of `GraphPrefs.compact`
 *  (graph row geometry). 'cozy' is the P67b tightened default. */
export type PanelDensity = 'cozy' | 'compact';

/** P80 D1: which commit button is emphasized in the Working tab footer. */
export type PrimaryCommitAction = 'commit' | 'commitPush';

export interface PaneWidths {
  sidebar: number;
  rightPanel: number;
}

/** Auto-fetch preference (P11 §2.3). OFF by default; interval in minutes. */
export interface AutoFetchSettings {
  enabled: boolean;
  intervalMinutes: number;
}

/** Periodic read-only refresh signal (P30 §5). OFF by default; interval in
 *  minutes. Mirrors the Rust `HealthRefresh`. */
export interface HealthRefreshSettings {
  enabled: boolean;
  intervalMinutes: number;
}

export interface UiSettings {
  theme: Theme;
  paneWidths: PaneWidths;
  listView: ListView;
  /** P67 §4: right-panel density; display-only, patches independently. */
  panelDensity: PanelDensity;
  /** P80 D1: which commit button is emphasized in the Working tab footer. */
  primaryCommitAction: PrimaryCommitAction;
  autoFetch: AutoFetchSettings;
  /** P30: periodic read-only refresh signal (backend scheduler). */
  healthRefresh: HealthRefreshSettings;
  graph: GraphPrefs;
  // AI assistance (P13).
  aiEnabled: boolean;
  aiConflictAutonomy: AiAutonomy;
  aiConsented: boolean;
  /** One-time consent to expose open repos to an external MCP client for
   *  reading (P16). */
  mcpConsented: boolean;
  /** One-time consent to let an external MCP client modify open repos (P16c). */
  mcpWriteConsented: boolean;
  /** P43: first-run onboarding has been shown+dismissed. Defaults false. */
  onboardingSeen: boolean;
  /** P42: auto-check for updates on launch. Defaults false. */
  autoCheckUpdates: boolean;
  /** P44: named identity profiles (global). */
  profiles: IdentityProfile[];
  /** P49: terminal launch command template ("{path}" placeholder). Empty ⇒
   *  per-OS auto-detect. */
  terminalCommand: string;
  /** P49: editor launch command template. Empty ⇒ auto-detect VS Code. */
  editorCommand: string;
  // ---- P68 §8.3: streaming AI-run knobs. Each patches independently; the two
  // LOCKED defaults are `aiHardCapSecs = 0` (unbounded — the user cancels instead)
  // and `aiMaxBudgetUsd = 0` (the `--max-budget-usd` flag is omitted entirely).
  /** Kill a run after this long with NO output from the CLI. `0` = disabled.
   *  PAUSED while the run awaits an answer (D3). Default 300. */
  aiIdleTimeoutSecs: number;
  /** Absolute wall-clock cap. `0` = unbounded (the default). Also paused while
   *  awaiting input. */
  aiHardCapSecs: number;
  /** Max turns before a still-questioning model is failed. Default 6. */
  aiMaxTurns: number;
  /** Stream `log` events at all. `false` suppresses them in RUST (no IPC cost);
   *  status-changing events always flow. Default true. */
  aiStreamLog: boolean;
  /** Pass `--include-partial-messages`. Default false (unverified line shape). */
  aiIncludePartialMessages: boolean;
  /** Repo access for a conflict run (D10). Default `readOnly`. */
  aiConflictTools: AiConflictTools;
  /** Bulk payload cap in bytes; over it the run SPLITS into sequential batches,
   *  never truncates. Default 400000. */
  aiBulkMaxBytes: number;
  /** `--max-budget-usd` when > 0; `0` ⇒ the flag is not passed. Default 0. */
  aiMaxBudgetUsd: number;
  /** Height of the AI activity dock in px. Default 180. */
  aiDockHeight: number;
  /** Dock starts collapsed (header only). Default false. */
  aiDockCollapsed: boolean;
}

export interface UiSettingsPatch {
  theme?: Theme;
  paneWidths?: PaneWidths;
  listView?: ListView;
  /** P67 §4: right-panel density (P67c). */
  panelDensity?: PanelDensity;
  /** P80 D1: primary commit action; patches independently. */
  primaryCommitAction?: PrimaryCommitAction;
  autoFetch?: AutoFetchSettings;
  /** Whole-struct patch, like autoFetch (P30 D7). */
  healthRefresh?: HealthRefreshSettings;
  graph?: GraphPrefs;
  // AI assistance (P13).
  aiEnabled?: boolean;
  aiConflictAutonomy?: AiAutonomy;
  aiConsented?: boolean;
  // Embedded MCP server (P16).
  mcpConsented?: boolean;
  // MCP write consent (P16c).
  mcpWriteConsented?: boolean;
  // First-run onboarding (P43).
  onboardingSeen?: boolean;
  // Auto-check-updates-on-launch (P42).
  autoCheckUpdates?: boolean;
  /** P44: identity profiles — whole-array replace (like paneWidths). */
  profiles?: IdentityProfile[];
  /** P49: terminal launch command template; patches independently. */
  terminalCommand?: string;
  /** P49: editor launch command template; patches independently. */
  editorCommand?: string;
  // P68 §8.3: the ten streaming AI-run knobs; each patches independently of
  // `graph` / `listView` / `panelDensity` and is clamped on write in Rust.
  aiIdleTimeoutSecs?: number;
  aiHardCapSecs?: number;
  aiMaxTurns?: number;
  aiStreamLog?: boolean;
  aiIncludePartialMessages?: boolean;
  aiConflictTools?: AiConflictTools;
  aiBulkMaxBytes?: number;
  aiMaxBudgetUsd?: number;
  aiDockHeight?: number;
  aiDockCollapsed?: boolean;
}
