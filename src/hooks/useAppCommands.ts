// P50c: App-level command-palette entries — everything valid app-wide. Threaded
// down to every RepoWorkspace, which merges them with its repo-scoped actions.
//
// Extracted from `App.tsx` by P69h: the file sits at its size ratchet, and this
// list is a self-contained table of {id, title, keywords, run} that grows with
// every feature (P69h itself adds two Settings deep links). Behaviour is
// verbatim — same ids, titles, keywords, order, and memo dependencies.
//
// ⚠️ IDENTITY IS LOAD-BEARING. The returned array is `CommandPalette`'s `actions`
// prop, and the palette re-runs `setHighlight(firstEnabledIndex(...))` whenever
// that array's identity changes — so a memo that recomputes per render would
// snap the highlight back to row 0 while the user is typing. Every dependency
// below must therefore be render-stable (a `useCallback`, a `useState` setter,
// or a plain value): the one-shot closures (`() => setHealthOpen(true)`) are
// built INSIDE the memo, exactly as they were inline in `App.tsx` before the
// extraction, and must never be accepted as props.

import { useMemo } from 'react';

import type { PaletteAction } from '../components/paletteActions';
import type { SettingsCategoryId } from '../components/settings/types';
import { shortcutLabel } from '../utils/platform';

/** Every field is required to be render-stable — see the header note. */
export interface AppCommandDeps {
  /** Null when no repo is open — gates `app.gitConfig` (UI §7.3). */
  activeRepo: string | null;
  openRepository(): Promise<void>;
  cloneOpen(): void;
  initRepository(): Promise<void>;
  /** Opens Settings on a category (P69h §5.3). `null` ⇒ wherever it was. */
  openSettingsAt(category: SettingsCategoryId | null): void;
  /** `useState` setters — stable by construction, and the reason the openers are
   *  closures built in the memo body rather than props. */
  setAiAssetsOpen(open: boolean): void;
  setHealthOpen(open: boolean): void;
  setOverlayOpen(open: boolean): void;
  toggleTheme(): void;
  toggleListView(): void;
  /** Re-probe git; resolves with the fresh availability (null ⇒ still probing). */
  gitRecheck(): Promise<{ found: boolean; detail: string } | null | undefined>;
  pushToast(tone: 'info', text: string): void;
}

export function useAppCommands(deps: AppCommandDeps): PaletteAction[] {
  const {
    activeRepo,
    openRepository,
    cloneOpen,
    initRepository,
    openSettingsAt,
    setAiAssetsOpen,
    setHealthOpen,
    setOverlayOpen,
    toggleTheme,
    toggleListView,
    gitRecheck,
    pushToast,
  } = deps;

  return useMemo<PaletteAction[]>(
    () => [
      {
        id: 'app.openRepo',
        title: 'Open repository…',
        hint: shortcutLabel('Mod+O'),
        group: 'action',
        keywords: 'folder browse',
        run: () => void openRepository(),
      },
      {
        id: 'app.clone',
        title: 'Clone repository…',
        group: 'action',
        keywords: 'git url download',
        run: cloneOpen,
      },
      {
        id: 'app.init',
        title: 'New repository…',
        group: 'action',
        keywords: 'init create',
        run: () => void initRepository(),
      },
      {
        id: 'app.settings',
        title: 'Open Settings',
        hint: shortcutLabel('Mod+,'),
        group: 'action',
        keywords:
          'preferences config options ai claude limits budget tools spend identity graph appearance updates',
        run: () => openSettingsAt(null),
      },
      {
        // P69h / UI §7.3: a deep link straight to the per-repository config.
        // Disabled with no repo — the pane would be the §1.2 empty block.
        id: 'app.gitConfig',
        title: 'Open Git config…',
        group: 'action',
        keywords: 'settings local global user.name user.email hooks',
        disabled: activeRepo === null,
        run: () => openSettingsAt('git-config'),
      },
      {
        id: 'app.identities',
        title: 'Manage identities…',
        group: 'action',
        keywords: 'profile user name email author committer signing',
        run: () => openSettingsAt('identities'),
      },
      {
        id: 'app.aiAssets',
        title: 'AI Assets',
        group: 'action',
        keywords: 'agents claude context',
        run: () => setAiAssetsOpen(true),
      },
      {
        id: 'app.health',
        title: 'Repository Health',
        group: 'action',
        keywords: 'stats status',
        run: () => setHealthOpen(true),
      },
      {
        id: 'app.toggleTheme',
        title: 'Toggle theme (light / dark)',
        group: 'action',
        keywords: 'appearance dark light',
        run: toggleTheme,
      },
      {
        id: 'app.toggleListView',
        title: 'Toggle tree / flat lists',
        group: 'action',
        keywords: 'sidebar view branches',
        run: toggleListView,
      },
      {
        // P70 (UI §8): the only surface on which a HEALTHY git ever reports
        // itself — the banner covers the unhealthy case, so a failed re-check
        // pushes nothing here.
        id: 'app.checkGit',
        title: 'Check Git availability',
        group: 'action',
        keywords: 'git missing path version diagnose recheck',
        run: () =>
          void gitRecheck().then((next) => {
            if (next?.found === true) pushToast('info', next.detail);
          }),
      },
      {
        id: 'app.shortcuts',
        title: 'Keyboard shortcuts',
        hint: '?',
        group: 'action',
        keywords: 'help keys',
        run: () => setOverlayOpen(true),
      },
    ],
    [
      activeRepo,
      openRepository,
      cloneOpen,
      initRepository,
      openSettingsAt,
      setAiAssetsOpen,
      setHealthOpen,
      setOverlayOpen,
      toggleTheme,
      toggleListView,
      gitRecheck,
      pushToast,
    ],
  );
}
