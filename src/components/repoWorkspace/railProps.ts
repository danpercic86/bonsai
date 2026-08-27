/** Spec-005: assembles GraphCanvas's `rail` bundle (RailInput) from the live
 *  rings channel. Extracted from RepoWorkspace/WorkspaceGraphPane (both sit at
 *  their size caps) — plan §"Files touched".
 *
 *  Channel pick mirrors WorkspaceGraphPane's matchRows pick (~line 377):
 *  historySearch wins while open — keep the two conditions in sync.
 *
 *  currentMatch trap (plan): `deriveMatchRows` drops oids absent from the
 *  layout, so `matchRows[currentMatch]` is NOT valid — the rows/jump pairs are
 *  rebuilt here aligned with the ORIGINAL `results.matches` indices, and the
 *  current row is resolved from the current match's oid directly. */
import { useMemo } from 'react';
import type { GraphLayout } from '../../ipc';
import type { RailInput } from '../../graph/rail/OverviewRail';
import type { UseCommitSearch } from './useCommitSearch';
import type { UseHistorySearch } from './useHistorySearch';

export interface UseRailInputDeps {
  search: UseCommitSearch;
  historySearch: UseHistorySearch;
  graph: GraphLayout | null;
  revealCommitByOid(oid: string): void;
  /** Bumped by RepoWorkspace when the graph stream reaches `done`. */
  generation: number;
  /** graphMinimapAlwaysShow (persisted). */
  alwaysShow: boolean;
}

const NO_ROWS: readonly number[] = [];

export function useRailInput(deps: UseRailInputDeps): RailInput {
  const { search, historySearch, graph, revealCommitByOid, generation, alwaysShow } = deps;

  // Commit-search channel: model rows + jump indices aligned pairwise. The
  // O(n) oid→row map builds only on results/layout changes — NOT per next/prev
  // step (the current row is its own cheap memo below). Only while open
  // (deriveMatchRows already pays the same cost there).
  const searchChannel = useMemo(() => {
    if (!search.open || search.results === null || graph === null) return null;
    const index = new Map<string, number>();
    for (let i = 0; i < graph.nodes.length; i += 1) index.set(graph.nodes[i].id, i);
    const rows: number[] = [];
    const jump: number[] = [];
    const matches = search.results.matches;
    for (let i = 0; i < matches.length; i += 1) {
      const row = index.get(matches[i].oid);
      if (row !== undefined) {
        rows.push(row);
        jump.push(i);
      }
    }
    return { rows, jump, index };
  }, [search.open, search.results, graph]);

  // Current match row: O(1) lookup per F3 step (null when it fell out of the
  // layout — never `matchRows[currentMatch]`, see the trap note above).
  const searchCurrentRow = useMemo(() => {
    if (searchChannel === null || search.currentMatch < 0) return null;
    const oid = search.results?.matches[search.currentMatch]?.oid;
    return oid !== undefined ? (searchChannel.index.get(oid) ?? null) : null;
  }, [searchChannel, search.results, search.currentMatch]);

  const historyOpen = historySearch.open;
  const historyRows = historySearch.matchRows;
  const goToMatch = search.goToMatch;

  return useMemo<RailInput>(() => {
    if (historyOpen) {
      // Ask-history channel: no current tick (UI §3.6); tick click resolves by
      // oid through the shared reveal path.
      return {
        generation,
        alwaysShow,
        ringsLive: historyRows.length > 0,
        matchRows: historyRows,
        currentMatchRow: null,
        onJumpToMatch: (i: number) => {
          const oid = graph?.nodes[historyRows[i]]?.id;
          if (oid !== undefined) revealCommitByOid(oid);
        },
      };
    }
    if (searchChannel === null) {
      return {
        generation,
        alwaysShow,
        ringsLive: false,
        matchRows: NO_ROWS,
        currentMatchRow: null,
        onJumpToMatch: () => {},
      };
    }
    return {
      generation,
      alwaysShow,
      ringsLive: searchChannel.rows.length > 0,
      matchRows: searchChannel.rows,
      currentMatchRow: searchCurrentRow,
      onJumpToMatch: (i: number) => {
        const matchIndex = searchChannel.jump[i];
        if (matchIndex !== undefined) goToMatch(matchIndex);
      },
    };
  }, [historyOpen, historyRows, searchChannel, searchCurrentRow, generation, alwaysShow, graph, revealCommitByOid, goToMatch]);
}
