import type { Dispatch, SetStateAction } from 'react';
import type { BranchInfo, BranchesSnapshot } from '../../ipc';
import type { RevealTarget } from '../../graph/reveal';
import type { TreeNode } from '../../utils/pathTree';
import { DeleteIcon } from '../menuIcons';
import { Tree } from '../Tree';
import { ListFilterInput } from '../ListFilterInput';
import { SectionHeader } from './SectionHeader';
import { BranchRow, DetachedHeadRow } from './rows';

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
  actionsDisabled: boolean;
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
  localFlatFiltered: BranchInfo[];
  localTreeFiltered: TreeNode<BranchInfo>[];
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

export function BranchesSection({
  data,
  branchesCollapsed,
  setBranchesCollapsed,
  actionsDisabled,
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
                <button
                  type="button"
                  className="sidebar-add sidebar-add-icon"
                  aria-label="Clean up branches…"
                  title="Clean up branches…"
                  disabled={actionsDisabled}
                  onClick={() => onCleanupBranches()}
                >
                  <DeleteIcon />
                </button>
              )}
              <button
                type="button"
                className="sidebar-add"
                aria-label="Create branch"
                title="Create branch"
                disabled={actionsDisabled}
                onClick={() => {
                  setBranchesCollapsed(false);
                  setCreateOpen(true);
                }}
              >
                {'+'}
              </button>
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
            <div className="branch-create-row">
              <input
                className="branch-create-input"
                type="text"
                placeholder="new-branch-name"
                autoFocus
                value={createValue}
                disabled={actionsDisabled}
                onChange={(e) => {
                  setCreateValue(e.target.value);
                  setCreateError(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    void submitCreate();
                  } else if (e.key === 'Escape') {
                    closeCreate();
                  }
                }}
                onBlur={() => {
                  if (createValue.trim() === '') closeCreate();
                }}
              />
              {createError !== null && (
                <div className="branch-create-error" role="alert">
                  {createError}
                </div>
              )}
            </div>
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
                    busy={actionsDisabled}
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
                  busy={actionsDisabled}
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
