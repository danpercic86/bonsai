import type { PaneWidths, RepoInfo, Theme } from './ipc';

export function folderName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

export function isUsableRepo(info: RepoInfo): boolean {
  return info.isRepo && !info.bare;
}

export function unusableRepoMessage(info: RepoInfo): string {
  return info.isRepo
    ? `Bare repositories are not supported: ${info.path}`
    : `Not a Git repository: ${info.path}`;
}

// P2a §2.5: persisted-sanity clamp ranges (mirrors settings.rs clamp_pane_widths).
const SIDEBAR_MIN = 180;
const SIDEBAR_MAX = 480;
const RIGHT_PANEL_MIN = 280;
const RIGHT_PANEL_MAX = 640;
const GRAPH_MIN_WIDTH = 480;
export const DEFAULT_PANE_WIDTHS: PaneWidths = { sidebar: 240, rightPanel: 380 };

/** Live-drag clamp (§2.5): the persisted range intersected with the current
 * window size and the graph pane's floor. */
export function clampLive(value: number, side: 'sidebar' | 'rightPanel', otherWidth: number): number {
  const [min, max] = side === 'sidebar' ? [SIDEBAR_MIN, SIDEBAR_MAX] : [RIGHT_PANEL_MIN, RIGHT_PANEL_MAX];
  const dynamicMax = Math.min(max, window.innerWidth - otherWidth - GRAPH_MIN_WIDTH);
  return Math.max(min, Math.min(value, Math.max(min, dynamicMax)));
}

/** P2b §4.2: sets data-theme on <html>. */
export function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute('data-theme', theme === 'light' ? 'light' : 'dark');
}
