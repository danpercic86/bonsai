// P69f §2.2 — the Settings context (types, contexts, hooks).
//
// `SettingsPanel` stays App's state-ownership boundary (its ~41 props are
// unchanged); its adapter hook builds ONE memoised value bag and ONE memoised
// action bag, and `SettingsProvider` publishes them here. The seven category
// pages read the context; the existing leaf sections keep their own props and
// are handed them by their page (§2.3), so their suites stay valid.
//
// Two contexts on purpose (§2.2 rule 1): a control that only dispatches never
// re-renders when an unrelated value changes.
//
// The provider component lives in `SettingsProvider.tsx` rather than here — the
// `ToastContext.ts` idiom, and what keeps `react-refresh/only-export-components`
// quiet in a module that also exports hooks.

import { createContext, useContext } from 'react';

import type { AiAvailability, McpStatus, UiSettings, UiSettingsPatch } from '../../ipc';
import type { McpScope } from '../../lib/mcpAddCommand';
import type { AiRunPrefs } from '../../settings/aiRunPrefs';
import type { UpdateUiState } from '../../hooks/useUpdateController';
import type { SettingsRowId } from './types';

/**
 * Persisted settings the pages read.
 *
 * `Pick<UiSettings, …>` per §2.2 — a rename in `ipc/types.ts` is then a compile
 * error here, not silent drift. Two notes on the shape:
 *   * the NAME differs from §2.2's `PersistedSettingsValues`, which is already
 *     taken in `settings/types.ts` (P69e) for the reset descriptors, where it
 *     aliases the whole `UiSettings`;
 *   * the eight AI-run keys stay folded into one `aiRun: AiRunPrefs` struct
 *     rather than being flattened, because that is the prop `SettingsPanel`
 *     receives and the struct `SettingsAiRunSection` consumes whole —
 *     flattening and re-assembling would mint a new object identity per render
 *     for no gain. `AiRunPrefs` is itself a `Pick<UiSettings, …>`, so those
 *     eight keys are drift-checked the same way.
 * The fields therefore mirror `SettingsPanel`'s value props exactly.
 */
export type SettingsPersistedValues = Pick<
  UiSettings,
  | 'theme'
  | 'listView'
  | 'panelDensity'
  | 'autoFetch'
  | 'healthRefresh'
  | 'graph'
  | 'aiEnabled'
  | 'aiConflictAutonomy'
  | 'aiConsented'
  | 'mcpConsented'
  | 'mcpWriteConsented'
  | 'autoCheckUpdates'
  | 'profiles'
  | 'terminalCommand'
  | 'editorCommand'
> & {
  /** The eight AI-run knobs, threaded whole (the `graph`/`autoFetch` idiom). */
  aiRun: AiRunPrefs;
};

/** Runtime facts that are NOT persisted settings. */
export interface SettingsRuntimeValues {
  repoPath: string | null;
  aiAvailability: AiAvailability | null;
  /** `aiEnabled && aiConsented` — computed once in the façade, never per page. */
  aiActive: boolean;
  mcpStatus: McpStatus | null;
  mcpEnabled: boolean;
  mcpAllowWrite: boolean;
  mcpRegistering: McpScope | null;
  updateCurrentVersion: string | null;
  updateState: UpdateUiState;
  /** Passed through verbatim (`undefined` included) so the Git-config section's
   *  scroll+focus effect sees exactly the value it sees today. */
  configInitialFocus: 'identity' | null | undefined;
  /** P69i: the Identities card to focus on open (null ⇒ none). */
  focusProfileId: string | null;
  /**
   * P69g — the whole-`UiSettings` view the catalog's reset descriptors compare
   * against (`SettingsRowReset.isDefault`), so `SettingsRow` can decide whether
   * to render `↺` without every page threading its own values. Built in the
   * adapter; see the doc comment there for the four keys it fills from defaults.
   */
  snapshot: UiSettings;
}

export type SettingsValues = SettingsPersistedValues & SettingsRuntimeValues;

export interface SettingsActions {
  change(patch: UiSettingsPatch): void;
  toggleTheme(): void;
  toggleListView(): void;
  /** Consent-aware wrapper (today's `handleEnableToggle`). */
  setAiEnabled(next: boolean): void;
  /** Consent-aware wrapper (today's `handleMcpEnableToggle`). */
  setMcpEnabled(next: boolean): void;
  /** Consent-aware wrapper (today's `handleMcpWriteToggle`). */
  setMcpAllowWrite(next: boolean): void;
  /** Holds `mcpRegistering` in the adapter while the run is in flight. */
  registerMcp(scope: McpScope): void;
  showOnboarding(): void;
  /** P69h / UI §1.2 — App's folder picker, offered by the Git-config empty block. */
  openRepository(): void;
  checkUpdate(): void;
  openUpdateDialog(): void;
  /** P69g / UI §5.7 — per-row reset. Resolves the patch from the catalog's
   *  `reset` descriptor + `DEFAULT_UI_SETTINGS`; a row with no descriptor is a
   *  no-op, never a throw. */
  resetRow(id: SettingsRowId): void;
}

/** `null` ⇒ no provider above. The hooks below throw on that. */
export const SettingsValuesContext = createContext<SettingsValues | null>(null);
export const SettingsActionsContext = createContext<SettingsActions | null>(null);

/** Throws (not `undefined`) outside a provider — a page rendered bare is a bug. */
export function useSettingsValues(): SettingsValues {
  const values = useContext(SettingsValuesContext);
  if (values === null) throw new Error('useSettingsValues must be used inside <SettingsProvider>');
  return values;
}

export function useSettingsActions(): SettingsActions {
  const actions = useContext(SettingsActionsContext);
  if (actions === null) {
    throw new Error('useSettingsActions must be used inside <SettingsProvider>');
  }
  return actions;
}
