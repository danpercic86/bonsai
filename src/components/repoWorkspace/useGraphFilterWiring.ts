// Spec-003 size-split: the graph-declutter wiring RepoWorkspace consumes as one
// bundle — the intent→wire controller (useGraphFilter), the ref refetchGraph
// reads (so its deps stay narrow), the meta chunk's truth flags, and the
// derived stale verdict for the chip. Spec-004 adds the fold controller
// (transient expansion state over the stream's FoldSpans) to the same bundle.
import { useRef, useState } from 'react';

import { useGraphFilter, type GraphFilterController } from '../../hooks/useGraphFilter';
import { useGraphFold, type GraphFoldController } from '../../hooks/useGraphFold';
import type { BranchesSnapshot, GraphFilter, GraphRefFilter, UiSettingsPatch } from '../../ipc';

export interface GraphFilterFlags {
  filtered: boolean;
  seedRefsApplied: boolean;
}

export interface GraphFilterWiring {
  graphFilter: GraphFilterController;
  /** Spec-004: fold model + expansion state + reveal/keyboard helpers. */
  fold: GraphFoldController;
  /** Latest derived wire filter — read by refetchGraph without widening deps. */
  graphFilterRef: React.RefObject<GraphFilter | null>;
  /** The meta chunk's truth flags (null until the first stream reports them). */
  setGraphFilterFlags(flags: GraphFilterFlags): void;
  /** §2.2: refs were requested but the backend says they did not apply. */
  stale: boolean;
}

export function useGraphFilterWiring(args: {
  graphFirstParent: boolean;
  /** Spec-004: the persisted fold-linear toggle. */
  graphFoldLinear: boolean;
  graphRefFilter: GraphRefFilter | null;
  onGraphFilterChange(patch: UiSettingsPatch): void;
  branches: BranchesSnapshot | null;
  // Spec-004: fold-controller inputs.
  repoId: string;
  /** Model row count of the current layout (0 while loading). */
  graphTotalRows: number;
  selectedIndexRef: { current: number | null };
  /** Reactive selection — clears the keyboard-active pill on any change (§3). */
  selectedIndex: number | null;
}): GraphFilterWiring {
  const graphFilter = useGraphFilter({
    graphFirstParent: args.graphFirstParent,
    graphFoldLinear: args.graphFoldLinear,
    graphRefFilter: args.graphRefFilter,
    onSettingsChange: args.onGraphFilterChange,
    branches: args.branches,
  });
  const graphFilterRef = useRef(graphFilter.filter);
  graphFilterRef.current = graphFilter.filter;
  const [flags, setGraphFilterFlags] = useState<GraphFilterFlags | null>(null);
  const fold = useGraphFold({
    graphFoldLinear: args.graphFoldLinear,
    totalRows: args.graphTotalRows,
    filterKey: graphFilter.filterKey,
    repoId: args.repoId,
    selectedIndexRef: args.selectedIndexRef,
    selectedIndex: args.selectedIndex,
  });
  return {
    graphFilter,
    fold,
    graphFilterRef,
    setGraphFilterFlags,
    stale: graphFilter.refsRequested && flags !== null && !flags.seedRefsApplied,
  };
}
