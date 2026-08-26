// Spec-003 size-split: the graph-declutter wiring RepoWorkspace consumes as one
// bundle — the intent→wire controller (useGraphFilter), the ref refetchGraph
// reads (so its deps stay narrow), the meta chunk's truth flags, and the
// derived stale verdict for the chip. Pure extraction; no behavior change.
import { useRef, useState } from 'react';

import { useGraphFilter, type GraphFilterController } from '../../hooks/useGraphFilter';
import type { BranchesSnapshot, GraphFilter, GraphRefFilter, UiSettingsPatch } from '../../ipc';

export interface GraphFilterFlags {
  filtered: boolean;
  seedRefsApplied: boolean;
}

export interface GraphFilterWiring {
  graphFilter: GraphFilterController;
  /** Latest derived wire filter — read by refetchGraph without widening deps. */
  graphFilterRef: React.RefObject<GraphFilter | null>;
  /** The meta chunk's truth flags (null until the first stream reports them). */
  setGraphFilterFlags(flags: GraphFilterFlags): void;
  /** §2.2: refs were requested but the backend says they did not apply. */
  stale: boolean;
}

export function useGraphFilterWiring(args: {
  graphFirstParent: boolean;
  graphRefFilter: GraphRefFilter | null;
  onGraphFilterChange(patch: UiSettingsPatch): void;
  branches: BranchesSnapshot | null;
}): GraphFilterWiring {
  const graphFilter = useGraphFilter({
    graphFirstParent: args.graphFirstParent,
    graphRefFilter: args.graphRefFilter,
    onSettingsChange: args.onGraphFilterChange,
    branches: args.branches,
  });
  const graphFilterRef = useRef(graphFilter.filter);
  graphFilterRef.current = graphFilter.filter;
  const [flags, setGraphFilterFlags] = useState<GraphFilterFlags | null>(null);
  return {
    graphFilter,
    graphFilterRef,
    setGraphFilterFlags,
    stale: graphFilter.refsRequested && flags !== null && !flags.seedRefsApplied,
  };
}
