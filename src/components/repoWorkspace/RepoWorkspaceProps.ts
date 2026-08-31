import type {
  AiAutonomy,
  AiAvailability,
  GraphPrefs,
  ListView,
  PaneWidths,
  PanelDensity,
  PrimaryCommitAction,
  UiSettingsPatch,
} from '../../ipc';
import type { PaletteAction } from '../paletteActions';

export interface RepoWorkspaceProps {
  /** Canonical workdir path (== repoId, P3e §2). */
  repoId: string;
  /** True when this tab is visible (the others are display:none). Gates the
   *  keyboard shortcut + Esc effects, window-focus rescan, GraphCanvas remeasure
   *  and the activation self-heal refresh (§5.1/§7). */
  active: boolean;
  /** App-global display prefs / pane sizing threaded down. */
  listView: ListView;
  /** P67 §4: right-panel vertical density (applied as a `data-density`
   *  attribute on the right panel's `<aside>`). */
  panelDensity: PanelDensity;
  /** P80 D1: which commit button is emphasized in the Working tab footer. */
  primaryCommitAction: PrimaryCommitAction;
  themeVersion: number;
  paneWidths: PaneWidths;
  /** True when a global modal (shortcut overlay / tab menu) is open — the
   *  workspace suppresses its own shortcuts + Esc handling (§5.1). */
  globalModalOpen: boolean;
  /** P11d §3.3/§4: user graph geometry knobs (threaded into the canvas). */
  graph: GraphPrefs;
  /** P11d §4.3: bumped by App on every graph-knob change → GraphCanvas re-measure. */
  metricsVersion: number;
  /** P13 §8: AI assistance settings + CLI health (App owns these + consent). */
  aiEnabled: boolean;
  aiConflictAutonomy: AiAutonomy;
  aiConsented: boolean;
  /** CLI health status; null while App is probing. */
  aiAvailability: AiAvailability | null;
  /** P68e §8: persisted AI-dock geometry + `aiStreamLog`; `onAiDockChange` patches
   *  `aiDockHeight`/`aiDockCollapsed` (App debounces the write). */
  aiDockHeight: number;
  aiDockCollapsed: boolean;
  aiStreamLog: boolean;
  onAiDockChange(patch: UiSettingsPatch): void;
  onSidebarResize(delta: number): void;
  onRightPanelResize(delta: number): void;
  onPaneResizeEnd(): void;
  /** P19 §6.5: open `path` in a new/focused tab (App.openTab). Used by the
   *  submodule "Open in new tab" action; reuses the existing open-repo flow. */
  onOpenRepoPath(path: string): void;
  /** P40b: open Settings → Git config → Identity (commit-error linkage). */
  onOpenIdentitySettings(): void;
  /** P80: open Settings → Accounts (the PR panel's "Manage accounts…"). */
  onOpenAccountSettings(): void;
  /** P50c: App-level command-palette entries (toggle theme/lists, open Settings
   *  / AI Assets / Health, open repo / clone / new) — merged with the repo-scoped
   *  entries this workspace assembles. Built once in App. */
  appCommands: PaletteAction[];
}
