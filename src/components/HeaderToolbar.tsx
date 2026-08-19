// P69i §5.2 (UI §4.1) — the whole `.header-toolbar`, lifted out of `App.tsx`.
//
// Two reasons it is its own file rather than five more lines in App: `App.tsx`
// sits at its size ratchet and may not grow, and the identity control needs its
// own open/anchor state — which, inline, would have been App state.
//
// Order (ui-reference §1): theme · list view · AI assets · health · settings ·
// identity. The identity control is the far-right account slot; the three
// repo-scoped controls render only when a repo is open.

import type { ListView, Theme, IdentityProfile } from '../ipc';
import { IdentityMenu } from './IdentityMenu';
import type { SettingsCategoryId } from './settings/types';

export interface HeaderToolbarProps {
  theme: Theme;
  onToggleTheme(): void;
  listView: ListView;
  onToggleListView(): void;
  /** Gates 🤖 / 📊 / the identity trigger, exactly as App did inline. */
  activeRepo: string | null;
  onOpenAiAssets(): void;
  onOpenHealth(): void;
  onOpenSettings(): void;
  /** Deep link for the identity menu's items 2/3/5 (UI §4.3). `focusProfileId`
   *  lands Settings on a specific identity card (item 2's saved draft). */
  onOpenSettingsAt(
    category: SettingsCategoryId,
    focus?: 'identity' | null,
    focusProfileId?: string | null,
  ): void;
  /** Lifted menu-open state — App suppresses global shortcuts while open. */
  onMenuOpenChange(open: boolean): void;
  profiles: IdentityProfile[];
  /** Whole-array replace — `Save “…” as an identity…` appends a draft (§4.3). */
  onProfilesChange(next: IdentityProfile[]): void;
}

export function HeaderToolbar({
  theme,
  onToggleTheme,
  listView,
  onToggleListView,
  activeRepo,
  onOpenAiAssets,
  onOpenHealth,
  onOpenSettings,
  onOpenSettingsAt,
  onMenuOpenChange,
  profiles,
  onProfilesChange,
}: HeaderToolbarProps) {
  return (
    <div className="header-toolbar">
      <button
        type="button"
        className="btn-icon theme-toggle"
        onClick={onToggleTheme}
        title={theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
        aria-label={theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
      >
        {theme === 'dark' ? '☀' : '☾'}
      </button>
      <button
        type="button"
        className="btn-icon list-view-toggle"
        onClick={onToggleListView}
        title={listView === 'tree' ? 'Switch to flat lists' : 'Switch to tree lists'}
        aria-label={listView === 'tree' ? 'Switch to flat lists' : 'Switch to tree lists'}
      >
        {listView === 'tree' ? '☰' : '⋔'}
      </button>
      {activeRepo !== null && (
        <button
          type="button"
          className="btn-icon ai-assets-toggle"
          onClick={onOpenAiAssets}
          title="AI Assets"
          aria-label="AI Assets"
        >
          {'🤖'}
        </button>
      )}
      {activeRepo !== null && (
        <button
          type="button"
          className="btn-icon repo-health-toggle"
          onClick={onOpenHealth}
          title="Health"
          aria-label="Health"
        >
          {'📊'}
        </button>
      )}
      <button
        type="button"
        className="btn-icon settings-toggle"
        onClick={onOpenSettings}
        title="Settings"
        aria-label="Settings"
      >
        {'⚙'}
      </button>
      {activeRepo !== null && (
        <IdentityMenu
          repoId={activeRepo}
          profiles={profiles}
          onProfilesChange={onProfilesChange}
          onOpenSettingsAt={onOpenSettingsAt}
          onMenuOpenChange={onMenuOpenChange}
        />
      )}
    </div>
  );
}
