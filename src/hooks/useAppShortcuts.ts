import { useEffect, type MutableRefObject } from 'react';
import type { TabMeta } from '../components/TabStrip';
import type { useSettingsRequest } from './useSettingsRequest';

type SettingsController = ReturnType<typeof useSettingsRequest>;

interface AppShortcutsOptions {
  menuOpen: boolean;
  overlayOpen: boolean;
  settings: SettingsController;
  aiAssetsOpen: boolean;
  healthOpen: boolean;
  onboardingOpen: boolean;
  closeOnboarding: () => void;
  setOverlayOpen: React.Dispatch<React.SetStateAction<boolean>>;
  setAiAssetsOpen: React.Dispatch<React.SetStateAction<boolean>>;
  setHealthOpen: React.Dispatch<React.SetStateAction<boolean>>;
  activeRepo: string | null;
  handleOpenRepository: () => Promise<void>;
  closeTab: (repoId: string) => void;
  globalModalOpen: boolean;
  tabsRef: MutableRefObject<TabMeta[]>;
  setActiveRepo: React.Dispatch<React.SetStateAction<string | null>>;
}

/** Global keyboard shortcuts extracted verbatim from App (§5.1). Esc peels the
 * topmost overlay; the second effect wires Ctrl+O / Ctrl+, / Ctrl+Tab / Ctrl+W
 * / ?. */
export function useAppShortcuts(opts: AppShortcutsOptions): void {
  const {
    menuOpen,
    overlayOpen,
    settings,
    aiAssetsOpen,
    healthOpen,
    onboardingOpen,
    closeOnboarding,
    setOverlayOpen,
    setAiAssetsOpen,
    setHealthOpen,
    activeRepo,
    handleOpenRepository,
    closeTab,
    globalModalOpen,
    tabsRef,
    setActiveRepo,
  } = opts;

  // Esc: close only the TOPMOST global overlay per keypress (LIFO peel:
  // shortcut overlay → settings → AI assets → health → onboarding). TabStrip's
  // own Esc handles its menu; skip when it consumed the keypress. Workspace
  // Esc-layering is separate.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (menuOpen) return;
      if (overlayOpen) {
        setOverlayOpen(false);
        return;
      }
      if (settings.open) {
        settings.close();
        return;
      }
      if (aiAssetsOpen) {
        setAiAssetsOpen(false);
        return;
      }
      if (healthOpen) {
        setHealthOpen(false);
        return;
      }
      if (onboardingOpen) closeOnboarding();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [menuOpen, overlayOpen, settings, aiAssetsOpen, healthOpen, onboardingOpen, closeOnboarding]);

  // Global shortcuts (§5.1): Ctrl+O open, ? overlay, Ctrl+Tab / Ctrl+Shift+Tab
  // cycle tabs, Ctrl+W close active tab.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;

      const target = e.target as HTMLElement | null;
      const typing =
        target !== null &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.tagName === 'SELECT' ||
          target.isContentEditable);

      if (menuOpen) return;

      if (ctrl && e.key.toLowerCase() === 'o') {
        e.preventDefault();
        void handleOpenRepository();
        return;
      }

      // UI §7.2: registered ABOVE the typing guard so it works from the commit
      // box. A no-op while ANY global modal is open — toggling a modal from a
      // shortcut is surprising, and opening Settings UNDERNEATH the AI-assets /
      // health / onboarding overlays would be worse (it would appear only after
      // the user dismissed something unrelated).
      if (ctrl && e.key === ',') {
        e.preventDefault();
        if (!globalModalOpen) settings.openAt(null);
        return;
      }

      if (ctrl && e.key === 'Tab') {
        e.preventDefault();
        const cur = tabsRef.current;
        if (cur.length === 0) return;
        const idx = cur.findIndex((t) => t.repoId === activeRepo);
        const base = idx === -1 ? 0 : idx;
        const nextIdx = (base + (e.shiftKey ? -1 : 1) + cur.length) % cur.length;
        setActiveRepo(cur[nextIdx].repoId);
        return;
      }

      if (typing) return;

      // Ctrl+W gated behind the typing guard: word-delete muscle memory in the
      // commit box must not close the tab (and lose the unsent message).
      if (ctrl && e.key.toLowerCase() === 'w') {
        e.preventDefault();
        if (activeRepo !== null) closeTab(activeRepo);
        return;
      }

      if (e.key === '?') {
        e.preventDefault();
        setOverlayOpen((cur) => !cur);
        return;
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [menuOpen, activeRepo, handleOpenRepository, closeTab, settings, globalModalOpen]);
}
