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
import type { SettingsOutcome } from './SettingsOutcomeNote';
import type { AiRunPrefs } from '../../settings/aiRunPrefs';
import type { ToolSelection } from '../../hooks/useUiSettings';
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
  | 'primaryCommitAction'
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
  /** P112 §16.4a: the detected-tool picker's two selections. Lookup keys, never
   *  program strings — the picker can only ever write a catalog id, `''` or
   *  `'custom'`, and Rust coerces anything else to `''`. */
  | 'terminalTool'
  | 'editorTool'
  | 'dev'
> &
  /** Spec-002: optional in `UiSettings` (frontend-only, absent from the Rust
   *  oracle), but the adapter resolves them to concrete defaults, so the pages
   *  read them non-optional. `Required<Pick<…>>` keeps a rename a compile error. */
  /** Spec-003: same optional-in-UiSettings, resolved-by-the-adapter treatment. */
  Required<
    Pick<
      UiSettings,
      | 'graphStyle'
      | 'graphSeason'
      | 'graphFirstParent'
      | 'graphFoldLinear'
      | 'graphMinimapAlwaysShow'
      | 'graphColorMode'
      | 'graphRefFilter'
    >
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
  /** P113 §17.3 — the AI-access section's outcome notes + its ONE announcement,
   *  owned by `useMcpControls` (which no longer has a `pushToast` to raise). */
  mcpOutcomes: ReadonlyMap<string, SettingsOutcome>;
  mcpAnnounce: string;
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
  /** P113 §7 — back the four MCP outcome notes out to their mount state.
   *  `AiCategory` calls it on mount AND on unmount, because the instance is
   *  owned by `useMcpControls` (§17.3) and that hook outlives the section:
   *  without it, closing Settings on `Could not register: …` and reopening
   *  tomorrow would show the same note with nothing having happened. */
  resetMcpOutcomes(): void;
  showOnboarding(): void;
  /** P69h / UI §1.2 — App's folder picker, offered by the Git-config empty block. */
  openRepository(): void;
  checkUpdate(): void;
  openUpdateDialog(): void;
  /** P69g / UI §5.7 — per-row reset. Resolves the patch from the catalog's
   *  `reset` descriptor + `DEFAULT_UI_SETTINGS`; a row with no descriptor is a
   *  no-op, never a throw. */
  resetRow(id: SettingsRowId): void;
  /** P112 §16.16-5 — adopt one or both external-tool selections read from disk
   *  WITHOUT queueing a write. App's `adoptToolSelection`, surfaced here because
   *  `pick_external_tool` persists the selection ITSELF: the renderer must
   *  re-read rather than patch, or it races the backend's own write
   *  (`ipc-api-tools.ts:24-25`).
   *
   *  NOT `hydrateUiSettings` (§17.3 reversed that): a whole-struct hydrate set
   *  every field from disk, and a successful write is never re-adopted
   *  (`useSettingsWriteQueue.ts:108-120`), so a patch still inside its 300 ms
   *  window was reverted ON SCREEN UNTIL THE NEXT LAUNCH — §16.4a's "one debounce
   *  window" was wrong. This setter touches only the field the caller names. */
  adoptToolSelection(selection: ToolSelection): void;
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
