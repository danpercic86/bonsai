/**
 * P68g §1 — the read-side view of the eight AI-run settings.
 *
 * `Pick<UiSettings, …>` rather than a hand-written interface, so adding or renaming a
 * field in `ipc/types.ts` breaks this at compile time instead of drifting.
 *
 * Why a GROUP and not eight props: the house idiom for a cluster of persisted values
 * is one whole-struct prop (`graph: GraphPrefs`, `autoFetch: AutoFetchSettings`), and
 * threading eight separate props through App + SettingsPanel would push `App.tsx` past
 * its file-size baseline for no gain. This is the READ channel only — the WRITE
 * channel stays per-field: every control patches exactly one key of `UiSettingsPatch`
 * (unlike `graph`/`autoFetch`, which patch whole-struct).
 */
import type { UiSettings } from '../ipc';

export type AiRunPrefs = Pick<
  UiSettings,
  | 'aiConflictTools'
  | 'aiStreamLog'
  | 'aiIncludePartialMessages'
  | 'aiIdleTimeoutSecs'
  | 'aiHardCapSecs'
  | 'aiMaxTurns'
  | 'aiMaxBudgetUsd'
  | 'aiBulkMaxBytes'
>;
