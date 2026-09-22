import { memo } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import type { BranchInfo, BranchesSnapshot } from '../../ipc';
import type { RevealTarget } from '../../graph/reveal';
import type { TreeNode } from '../../utils/pathTree';
import { DeleteIcon } from '../menuIcons';
import { Tree } from '../Tree';
import { ListFilterInput } from '../ListFilterInput';
import { BranchCreateRow } from './BranchCreateRow';
import { SectionHeader } from './SectionHeader';
import { SidebarActionButton } from './SidebarActionButton';
import { BranchRow, DetachedHeadRow } from './rows';
import { useRenderCount } from '../../obs/react';

/** P4d: proper ancestor folder prefixes of a branch name.
 *  "a/b/c" -> ["a", "a/b"]; root-level branch -> []. */
function ancestorPrefixes(name: string): string[] {
  const segs = name.split('/').filter(Boolean);
  const out: string[] = [];
  for (let i = 1; i < segs.length; i++) out.push(segs.slice(0, i).join('/'));
  return out;
}

export interface BranchesSectionProps {
  data: BranchesSnapshot;
  branchesCollapsed: boolean;
  setBranchesCollapsed: Dispatch<SetStateAction<boolean>>;
  /** NO `actionsDisabled` PROP (P118) — the two header buttons and the create
   *  input read it from `SidebarBusyContext` themselves, so a mutation's
   *  true→false flip re-renders those leaves instead of this whole section. */
  onCleanupBranches?: () => void;
  treeMode: boolean;
  currentBranch: string | null;
  onCheckout(name: string): void;
  onContextMenu(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
    clientX: number,
    clientY: number,
  ): void;
  onReveal?: (t: RevealTarget) => void;
  showBranchFilter: boolean;
  branchFilter: string;
  setBranchFilter: Dispatch<SetStateAction<string>>;
  branchFiltering: boolean;
  localFlatFiltered: readonly BranchInfo[];
  localTreeFiltered: readonly TreeNode<BranchInfo>[];
  branchNoMatch: boolean;
  createOpen: boolean;
  setCreateOpen: Dispatch<SetStateAction<boolean>>;
  createValue: string;
  setCreateValue: Dispatch<SetStateAction<string>>;
  createError: string | null;
  setCreateError: Dispatch<SetStateAction<string | null>>;
  closeCreate(): void;
  submitCreate(): void | Promise<void>;
}

function BranchesSectionImpl({
  data,
  branchesCollapsed,
  setBranchesCollapsed,
  onCleanupBranches,
  treeMode,
  currentBranch,
  onCheckout,
  onContextMenu,
  onReveal,
  showBranchFilter,
  branchFilter,
  setBranchFilter,
  branchFiltering,
  localFlatFiltered,
  localTreeFiltered,
  branchNoMatch,
  createOpen,
  setCreateOpen,
  createValue,
  setCreateValue,
  createError,
  setCreateError,
  closeCreate,
  submitCreate,
}: BranchesSectionProps) {
  useRenderCount('BranchesSection', undefined, 'aggregate'); // §9.2
  return (
    <section className="sidebar-section">
      <SectionHeader
        label="Branches"
        collapsed={branchesCollapsed}
        onToggle={() => setBranchesCollapsed((c) => !c)}
        extra={
          !data.head.unborn && (
            <>
              {onCleanupBranches && (
                <SidebarActionButton
                  label="Clean up branches…"
                  iconOnly
                  onClick={() => onCleanupBranches()}
                >
                  <DeleteIcon />
                </SidebarActionButton>
              )}
              <SidebarActionButton
                label="Create branch"
                onClick={() => {
                  setBranchesCollapsed(false);
                  setCreateOpen(true);
                }}
              >
                {'+'}
              </SidebarActionButton>
            </>
          )
        }
      />
      {!branchesCollapsed && (
        <>
          {showBranchFilter && (
            <ListFilterInput
              value={branchFilter}
              onChange={setBranchFilter}
              ariaLabel="Filter branches"
              count={branchFiltering ? localFlatFiltered.length : undefined}
            />
          )}
          {createOpen && (
            <BranchCreateRow
              value={createValue}
              setValue={setCreateValue}
              error={createError}
              setError={setCreateError}
              close={closeCreate}
              submit={submitCreate}
            />
          )}
          {(data.head.detached || !treeMode) && (
            <ul className="branch-list" role="group">
              {data.head.detached && (
                <DetachedHeadRow oid={data.head.oid} treeKey="detached" />
              )}
              {!treeMode &&
                localFlatFiltered.map((branch) => (
                  <BranchRow
                    key={branch.name}
                    branch={branch}
                    onCheckout={onCheckout}
                    onContextMenu={onContextMenu}
                    onReveal={onReveal}
                    treeKey={`branch:${branch.name}`}
                  />
                ))}
            </ul>
          )}
          {treeMode && localTreeFiltered.length > 0 && (
            <Tree
              // A filter-active key remounts with everything expanded so
              // matching leaves are visible (not hidden in collapsed dirs).
              key={
                branchFiltering
                  ? `local-filter:${currentBranch ?? 'none'}`
                  : `local:${currentBranch ?? 'none'}`
              }
              asGroup
              nodes={localTreeFiltered}
              leafKey={(l) => l.item.name}
              defaultCollapsed={!branchFiltering}
              initiallyExpanded={
                branchFiltering
                  ? []
                  : currentBranch !== null
                    ? ancestorPrefixes(currentBranch)
                    : []
              }
              renderLeaf={(l, level) => (
                <BranchRow
                  branch={l.item}
                  onCheckout={onCheckout}
                  onContextMenu={onContextMenu}
                  onReveal={onReveal}
                  displayName={l.name}
                  treeKey={`branch:${l.item.name}`}
                  level={level}
                />
              )}
            />
          )}
          {branchNoMatch && (
            <p className="branch-muted">{`No branches match '${branchFilter.trim()}'`}</p>
          )}
          {!data.head.detached && data.local.length === 0 && (
            <p className="branch-muted">No branches yet</p>
          )}
        </>
      )}
    </section>
  );
}

/** Render-storm fix: a plain `memo` is enough here — every prop is either a
 *  scalar, a `useState` setter, or one of the Sidebar's memoised arrays, so an
 *  unchanged round hands this component byte-identical props BY REFERENCE. */
export const BranchesSection = memo(BranchesSectionImpl);
