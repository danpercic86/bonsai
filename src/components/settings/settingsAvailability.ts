/**
 * P69k — which catalogued rows are actually RENDERABLE right now.
 *
 * Search may not offer a row the pane cannot show. A `requires` row is absent
 * from the DOM whenever its precondition is false, so counting it produced a
 * status line, a rail count and a result block that contained nothing at all
 * (`bearer` with the MCP server stopped said "1 settings match" over an empty
 * AI block, and never reached the §3.3 zero-match state).
 *
 * These predicates are therefore the ONE definition of those preconditions:
 * `searchSettings` filters with them and the DOM↔catalog coverage guard imports
 * them as its AM-4a table, so the two cannot silently disagree about whether a
 * row exists.
 *
 * Pure data, like `settingsCatalog.ts`: no React, no IPC, no DOM.
 */
import type { SettingsIndexEntry, SettingsRowRequirement } from './types';

/**
 * The runtime facts the `requires` predicates read.
 *
 * Structural on purpose — both the live `SettingsValues` bag (`SettingsShell`)
 * and the coverage fixtures satisfy it without either having to know about this
 * type, and widening the union in `types.ts` is then a compile error here.
 */
export interface SettingsAvailability {
  repoPath: string | null;
  aiEnabled: boolean;
  aiConsented: boolean;
  mcpStatus: { enabled: boolean } | null;
  profiles: readonly unknown[];
}

/** One predicate per `SettingsRowRequirement` — the union IS the contract, so
 *  all five are implemented even though the catalog uses three today. */
export const REQUIREMENT_PREDICATES: Readonly<
  Record<SettingsRowRequirement, (available: SettingsAvailability) => boolean>
> = {
  repo: (a) => a.repoPath !== null,
  aiActive: (a) => a.aiEnabled && a.aiConsented,
  mcpRunning: (a) => a.mcpStatus?.enabled === true,
  mcpStopped: (a) => a.mcpStatus?.enabled !== true,
  profile: (a) => a.profiles.length > 0,
};

/** True when this row renders under these conditions. No `requires` ⇒ always. */
export function isRowAvailable(
  entry: SettingsIndexEntry,
  availability: SettingsAvailability,
): boolean {
  const requirement = entry.requires;
  return requirement === undefined || REQUIREMENT_PREDICATES[requirement](availability);
}
