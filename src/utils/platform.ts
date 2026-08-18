// Platform detection + shortcut-label rendering.
//
// The app binds every accelerator as `e.ctrlKey || e.metaKey` (see App.tsx and
// repoWorkspace/useWorkspaceKeyboard.ts), so the SAME binding is Ctrl on
// Windows/Linux and Cmd on macOS. This module is the single place that turns a
// binding spec into the label a user of THIS platform expects — it never
// influences key handling, only the text we show.
//
// Specs are written once, platform-neutrally: 'Mod+Shift+F', 'Mod+Enter'.
//   * `Mod`   -> 'Ctrl' / '⌘'   (the primary accelerator)
//   * `Shift` -> 'Shift' / '⇧'
//   * `Alt`   -> 'Alt' / '⌥'
//   * `Ctrl`  -> 'Ctrl' / '⌃'   (the LITERAL control key, rare)
// Any other token (Enter, Esc, F5, R, ↑, ?) passes through unchanged.

/** Modifier tokens recognised in a spec (case-insensitive). */
export type ModifierToken = 'mod' | 'shift' | 'alt' | 'ctrl';

const MAC_GLYPH: Record<ModifierToken, string> = {
  mod: '⌘', // ⌘ Command
  shift: '⇧', // ⇧ Shift
  alt: '⌥', // ⌥ Option
  ctrl: '⌃', // ⌃ Control
};

const PC_WORD: Record<ModifierToken, string> = {
  mod: 'Ctrl',
  shift: 'Shift',
  alt: 'Alt',
  ctrl: 'Ctrl',
};

/** The subset of `navigator` we probe — keeps `detectIsMac` testable. */
export interface PlatformProbe {
  platform?: string;
  userAgent?: string;
  userAgentData?: { platform?: string };
}

/**
 * True when running on macOS (or an Apple mobile webview). Defensive by design:
 * jsdom, SSR and locked-down webviews may expose no `navigator` at all, and
 * `navigator.platform` is deprecated — so we probe several sources, swallow any
 * throw, and default to NOT-mac when nothing is knowable.
 */
export function detectIsMac(probe?: PlatformProbe): boolean {
  try {
    const nav =
      probe ?? (typeof navigator === 'undefined' ? undefined : (navigator as PlatformProbe));
    if (!nav) return false;
    const hints = [nav.userAgentData?.platform, nav.platform, nav.userAgent];
    for (const hint of hints) {
      if (typeof hint !== 'string' || hint.length === 0) continue;
      if (/mac|iphone|ipad|ipod/i.test(hint)) return true;
    }
    return false;
  } catch {
    return false;
  }
}

/** Resolved once per session — the platform cannot change under a running app. */
export const isMac: boolean = detectIsMac();

/** Separator between key caps: macOS runs its glyphs together (⌘⇧F). */
export function shortcutSeparator(mac: boolean = isMac): string {
  return mac ? '' : '+';
}

/** One spec token -> its display label on this platform. */
export function keyLabel(token: string, mac: boolean = isMac): string {
  const key = token.toLowerCase();
  if (key === 'mod' || key === 'shift' || key === 'alt' || key === 'ctrl') {
    return (mac ? MAC_GLYPH : PC_WORD)[key];
  }
  return token;
}

/** Splits a spec into its per-cap labels: 'Mod+Shift+F' -> ['Ctrl','Shift','F']. */
export function shortcutKeys(spec: string, mac: boolean = isMac): string[] {
  return spec
    .split('+')
    .map((token) => token.trim())
    .filter((token) => token.length > 0)
    .map((token) => keyLabel(token, mac));
}

/** A whole spec as one string: 'Mod+Shift+F' -> 'Ctrl+Shift+F' or '⌘⇧F'. */
export function shortcutLabel(spec: string, mac: boolean = isMac): string {
  return shortcutKeys(spec, mac).join(shortcutSeparator(mac));
}
