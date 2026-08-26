// Spec-003 (UI contract §1): the graph-declutter INTENT state and its wire
// derivation. The persisted settings (`graphFirstParent`, `graphRefFilter`) are
// owned by useUiSettings/App and arrive as props; this hook derives the wire
// `GraphFilter` per request (solo → refs verbatim; hide → known refs − refs, so
// branches created while a hide set is active still appear), exposes the menu
// actions and the chip's `activeSummary`. No graph topology lives here — the
// backend owns staleness, HEAD-always-included, and the layout itself.
import { useCallback, useEffect, useMemo, useRef, type RefObject } from 'react';

import type { GraphLayout } from '../ipc';

import type {
  BranchesSnapshot,
  GraphFilter,
  GraphRefFilter,
  UiSettingsPatch,
} from '../ipc';

export interface UseGraphFilterArgs {
  graphFirstParent: boolean;
  /** Spec-004: fold linear runs (persisted; gates span computation only). */
  graphFoldLinear: boolean;
  graphRefFilter: GraphRefFilter | null;
  /** App's debounced settings path (live state update + persist). */
  onSettingsChange(patch: UiSettingsPatch): void;
  /** The freshest sidebar snapshot — hide-mode derivation reads it per request. */
  branches: BranchesSnapshot | null;
}

export interface GraphFilterController {
  firstParent: boolean;
  /** Spec-004: the persisted fold-linear toggle. */
  foldLinear: boolean;
  refFilter: GraphRefFilter | null;
  /** Derived wire filter; `null` when nothing is requested (default walk). */
  filter: GraphFilter | null;
  /** Stable serialization of `filter` — effect key (identity churns per render). */
  filterKey: string;
  /** Spec-004: serialization of the WALK subset (firstParent + seedRefs — the
   *  mirror of Rust `walk_eq`). Fold toggles never change it, so keying the
   *  refetch on it keeps toggle-OFF a zero-IPC local operation (plan lock). */
  walkKey: string;
  /** Any filter requested (chip shows the active state). */
  requested: boolean;
  /** True when the request carries a seed-ref restriction (stale detection). */
  refsRequested: boolean;
  /** Chip label (UI contract §2). */
  activeSummary: string;
  /** Sidebar row marker for a FULL ref name (§3.3); null = no marker. */
  markerFor(fullRef: string): 'solo' | 'hidden' | null;
  toggleFirstParent(): void;
  /** Spec-004: flip the persisted fold-linear toggle. */
  toggleFoldLinear(): void;
  soloRef(fullRef: string): void;
  addToSolo(fullRef: string): void;
  hideRef(fullRef: string): void;
  unfilterRef(fullRef: string): void;
  clearAll(): void;
}

/** Short display name: full ref minus its `refs/heads/`-style prefix. */
export function shortRefName(fullRef: string): string {
  for (const p of ['refs/heads/', 'refs/remotes/', 'refs/tags/']) {
    if (fullRef.startsWith(p)) return fullRef.slice(p.length);
  }
  return fullRef;
}

/** All currently-known full ref names (the same list the sidebar renders). */
export function knownFullRefs(branches: BranchesSnapshot | null): string[] {
  if (branches === null) return [];
  return [
    ...branches.local.map((b) => `refs/heads/${b.name}`),
    ...branches.remote.map((r) => `refs/remotes/${r.name}`),
    ...branches.tags.map((t) => `refs/tags/${t}`),
  ];
}

/** Pure derivation: intent → wire filter (null = default walk). Exported for
 *  tests; the hook memoizes it. */
export function deriveGraphFilter(
  firstParent: boolean,
  refFilter: GraphRefFilter | null,
  knownRefs: string[],
  foldLinear = false,
): GraphFilter | null {
  let seedRefs: string[] | null = null;
  if (refFilter !== null && refFilter.refs.length > 0) {
    seedRefs =
      refFilter.mode === 'solo'
        ? [...refFilter.refs]
        : knownRefs.filter((r) => !refFilter.refs.includes(r)); // may be [] = hide-all
  }
  if (!firstParent && seedRefs === null && !foldLinear) return null;
  return { firstParent, seedRefs, foldLinear };
}

/** Chip label per UI contract §2 + spec-004 §4 ("First-parent" / "Folded" /
 *  "Solo: x +n" / "n branches hidden", joined with " · "). Fold is FRONTEND
 *  truth — the segment ORs in the local toggle, never a backend flag. Empty
 *  when nothing is requested. */
export function graphFilterSummary(
  firstParent: boolean,
  refFilter: GraphRefFilter | null,
  foldLinear = false,
): string {
  const parts: string[] = [];
  if (firstParent) parts.push('First-parent');
  if (foldLinear) parts.push('Folded');
  if (refFilter !== null && refFilter.refs.length > 0) {
    const n = refFilter.refs.length;
    if (refFilter.mode === 'solo') {
      const first = shortRefName(refFilter.refs[0]);
      parts.push(n === 1 ? `Solo: ${first}` : `Solo: ${first} +${n - 1}`);
    } else {
      parts.push(n === 1 ? '1 branch hidden' : `${n} branches hidden`);
    }
  }
  return parts.join(' · ');
}

/** Spec-003: reload the graph when the declutter filter CHANGES. Keyed on the
 *  stable `filterKey` string (hide-mode derivation mints a new array per
 *  branches refresh; identity must not refetch); the first run is skipped (the
 *  initial load already used the hydrated filter). When the previous selection
 *  survives the reload, scroll it into view via the shared reveal path —
 *  refetchGraph itself clears a selection that didn't. */
export function useGraphFilterRefetch(args: {
  /** Spec-004: the WALK-subset key (`controller.walkKey`), NOT the full
   *  filterKey — fold toggles must not re-key this effect (toggle-OFF is a
   *  zero-IPC local collapse; toggle-ON re-requests via the effect below). */
  filterKey: string;
  /** Spec-004: rising edge (false→true) re-requests the stream so the backend
   *  can attach `foldSpans` (a cache Hit replay). Falling edge: nothing. */
  foldLinear?: boolean;
  refetchGraph(): Promise<void>;
  selectedIndexRef: RefObject<number | null>;
  graphDataRef: RefObject<GraphLayout | null>;
  revealCommitByOid(oid: string): void;
}): void {
  const { filterKey, foldLinear = false, refetchGraph, selectedIndexRef, graphDataRef, revealCommitByOid } = args;
  // Spec-004: fold-on re-request. Skipped on mount (initial load already used
  // the hydrated filter, fold included).
  const prevFoldRef = useRef<boolean | null>(null);
  useEffect(() => {
    const prev = prevFoldRef.current;
    prevFoldRef.current = foldLinear;
    if (prev === null || prev === foldLinear || !foldLinear) return;
    void refetchGraph();
  }, [foldLinear, refetchGraph]);
  const prevKeyRef = useRef<string | null>(null);
  useEffect(() => {
    const prev = prevKeyRef.current;
    prevKeyRef.current = filterKey;
    if (prev === null || prev === filterKey) return;
    const i = selectedIndexRef.current;
    const oid = i !== null ? (graphDataRef.current?.nodes[i]?.id ?? null) : null;
    void refetchGraph().then(() => {
      if (oid !== null && graphDataRef.current?.nodes.some((n) => n.id === oid) === true) {
        revealCommitByOid(oid);
      }
    });
  }, [filterKey, refetchGraph, revealCommitByOid, selectedIndexRef, graphDataRef]);
}

export function useGraphFilter({
  graphFirstParent,
  graphFoldLinear,
  graphRefFilter,
  onSettingsChange,
  branches,
}: UseGraphFilterArgs): GraphFilterController {
  const knownRefs = useMemo(() => knownFullRefs(branches), [branches]);

  const filter = useMemo(() => {
    // Hide-mode needs the known-ref list to derive its whitelist. Before the
    // first branches snapshot lands, an empty list would derive `seedRefs: []`
    // — indistinguishable from an intentional hide-all on the wire — so the
    // initial paint would be a wrong HEAD-only graph. Derive the honest full
    // graph instead; the snapshot's arrival re-keys the refetch.
    const effective =
      branches === null && graphRefFilter?.mode === 'hide' ? null : graphRefFilter;
    return deriveGraphFilter(graphFirstParent, effective, knownRefs, graphFoldLinear);
  }, [graphFirstParent, graphFoldLinear, graphRefFilter, knownRefs, branches]);
  // Hide-mode derivation mints a new array per branches refresh — key effects on
  // this stable string so a content-identical refilter never refetches the graph.
  const filterKey = useMemo(() => JSON.stringify(filter), [filter]);
  // Spec-004: the walk subset (fold cleared) — mirror of Rust `walk_eq`.
  // NORMALIZED so `null` (no filter) and a fold-only filter serialize the same:
  // toggling fold must never change this key (the fold effect owns that edge).
  const walkKey = useMemo(
    () =>
      JSON.stringify({
        firstParent: filter?.firstParent ?? false,
        seedRefs: filter?.seedRefs ?? null,
      }),
    [filter],
  );

  const setRefFilter = useCallback(
    (next: GraphRefFilter | null) => onSettingsChange({ graphRefFilter: next }),
    [onSettingsChange],
  );

  const toggleFirstParent = useCallback(
    () => onSettingsChange({ graphFirstParent: !graphFirstParent }),
    [onSettingsChange, graphFirstParent],
  );
  // Spec-004: one setting, two controls (popover switch + Settings row).
  const toggleFoldLinear = useCallback(
    () => onSettingsChange({ graphFoldLinear: !graphFoldLinear }),
    [onSettingsChange, graphFoldLinear],
  );
  // Modes are exclusive (§1): a fresh solo/hide replaces the other mode's set.
  const soloRef = useCallback(
    (fullRef: string) => setRefFilter({ mode: 'solo', refs: [fullRef] }),
    [setRefFilter],
  );
  const addToSolo = useCallback(
    (fullRef: string) => {
      if (graphRefFilter?.mode !== 'solo') {
        setRefFilter({ mode: 'solo', refs: [fullRef] });
        return;
      }
      if (graphRefFilter.refs.includes(fullRef)) return;
      setRefFilter({ mode: 'solo', refs: [...graphRefFilter.refs, fullRef] });
    },
    [setRefFilter, graphRefFilter],
  );
  const hideRef = useCallback(
    (fullRef: string) => {
      if (graphRefFilter?.mode !== 'hide') {
        setRefFilter({ mode: 'hide', refs: [fullRef] });
        return;
      }
      if (graphRefFilter.refs.includes(fullRef)) return;
      setRefFilter({ mode: 'hide', refs: [...graphRefFilter.refs, fullRef] });
    },
    [setRefFilter, graphRefFilter],
  );
  const unfilterRef = useCallback(
    (fullRef: string) => {
      if (graphRefFilter === null) return;
      const refs = graphRefFilter.refs.filter((r) => r !== fullRef);
      // Removing the last ref clears the ref filter (§2.1).
      setRefFilter(refs.length === 0 ? null : { ...graphRefFilter, refs });
    },
    [setRefFilter, graphRefFilter],
  );
  // Spec-004: "Show full graph" clears every declutter control, fold included.
  const clearAll = useCallback(
    () =>
      onSettingsChange({ graphFirstParent: false, graphFoldLinear: false, graphRefFilter: null }),
    [onSettingsChange],
  );

  const markerFor = useCallback(
    (fullRef: string): 'solo' | 'hidden' | null => {
      if (graphRefFilter === null || !graphRefFilter.refs.includes(fullRef)) return null;
      return graphRefFilter.mode === 'solo' ? 'solo' : 'hidden';
    },
    [graphRefFilter],
  );

  const activeSummary = useMemo(
    () => graphFilterSummary(graphFirstParent, graphRefFilter, graphFoldLinear),
    [graphFirstParent, graphRefFilter, graphFoldLinear],
  );
  // Memoized controller: consumers (palette registry, chip) depend on the
  // object identity, so it must only churn when a field actually changes.
  return useMemo(
    () => ({
      firstParent: graphFirstParent,
      foldLinear: graphFoldLinear,
      refFilter: graphRefFilter,
      filter,
      filterKey,
      walkKey,
      requested: filter !== null,
      refsRequested: filter !== null && filter.seedRefs !== null,
      activeSummary,
      markerFor,
      toggleFirstParent,
      toggleFoldLinear,
      soloRef,
      addToSolo,
      hideRef,
      unfilterRef,
      clearAll,
    }),
    [
      graphFirstParent,
      graphFoldLinear,
      graphRefFilter,
      filter,
      filterKey,
      walkKey,
      activeSummary,
      markerFor,
      toggleFirstParent,
      toggleFoldLinear,
      soloRef,
      addToSolo,
      hideRef,
      unfilterRef,
      clearAll,
    ],
  );
}
