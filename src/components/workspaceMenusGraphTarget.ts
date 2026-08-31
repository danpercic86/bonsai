/** The graph right-click dispatcher: `GraphContextTarget` → menu items.
 *
 *  Extracted from `workspaceMenus.ts` (P92) so that file stays under the
 *  file-size limit. Composes the already-bound per-ref builders it is handed —
 *  it holds no state and closes over nothing beyond them:
 *
 *  - `kind:'ref'`      → that ref's menu (a pill right-click; unchanged since P6).
 *  - `kind:'refPicker'`→ P92 §1.1: one row per HIDDEN entity of a "+N" chip.
 *  - `kind:'commit'`   → P92 §2.2: with ≥2 actionable refs, the picker level is
 *                        prepended above the (unchanged) commit actions; with
 *                        ≤1 the result is byte-identical to pre-P92.
 */

import { createElement } from 'react';
import type { ContextMenuItem } from './ContextMenu';
import { BranchIcon } from './menuIcons';
import { actionableEntities, refPickerItems } from './workspaceMenusRefPicker';
import type { WorkspaceMenuDeps } from './workspaceMenus';
import type { RefLabel } from '../ipc';
import type { GraphContextTarget } from '../graph/GraphCanvas';
import type { RefEntity } from '../graph/refLabels';
import { fallbackBranchRef, targetRefOf } from '../graph/hitTest';

/** The bound builders this dispatcher composes, plus the deps it reads directly
 *  (`branches` for the HEAD-branch fallback, `mutating`/`opActive` for its
 *  gate). Supplied by `createWorkspaceMenus`. */
export interface GraphTargetBuilders {
  deps: WorkspaceMenuDeps;
  branchMenuItems(name: string, kind: 'localBranch' | 'remoteBranch'): ContextMenuItem[];
  stashMenuItems(index: number, oid?: string): ContextMenuItem[];
  tagMenuItems(name: string, oid: string | null): ContextMenuItem[];
  commitMenuItems(oid: string): ContextMenuItem[];
}

/** P5 §5.2 / P6 §4.2: the menu ONE ref opens — shared by a pill right-click, a
 *  "+N" overflow row and a commit-menu picker row, so the three can never
 *  diverge. */
export function refItems(
  b: GraphTargetBuilders,
  r: RefLabel,
  oid: string,
): ContextMenuItem[] {
  // P10 §5: a stash pill → Apply/Pop/Drop menu (parse the index from the name).
  if (r.kind === 'stash') {
    const m = /^stash@\{(\d+)\}$/.exec(r.name);
    if (m === null) return []; // malformed name → no menu (defensive)
    return b.stashMenuItems(Number(m[1]));
  }
  if (r.kind === 'head') return [];
  // P22 §7.2: the graph tag pill opens the same menu as the sidebar tag row.
  // P47 (Fork-1): the graph tag pill carries the node oid → pass it so the
  // shared commit actions are appended (sidebar tag rows pass null).
  if (r.kind === 'tag') return b.tagMenuItems(r.name, oid);
  const kind = r.kind === 'remoteBranch' ? 'remoteBranch' : 'localBranch';
  const items = b.branchMenuItems(r.name, kind);
  if (items.length > 0) return items;
  // P18b: whole-row right-click resolved to a branch whose branch menu is
  // empty — the current HEAD branch. Fall back to the commit menu (resolving
  // the row's oid from the branch tip) so the row still opens a useful menu.
  const snapshot = b.deps.branches;
  if (snapshot === null) return [];
  const entry =
    kind === 'localBranch'
      ? snapshot.local.find((x) => x.name === r.name)
      : snapshot.remote.find((x) => x.name === r.name);
  if (entry === undefined) return [];
  const commitItems = b.commitMenuItems(entry.tip);
  // P60a: the current HEAD branch's own branch menu is empty (branchMenuItems
  // returns [] for isHead), so its graph pill / whole-row lands on this commit
  // fallback. PREPEND "Rename…" so the current branch — the most common rename
  // target — is renamable from the graph HEAD pill (exercising the wasHead
  // refresh path). Local branches only.
  // TODO(P60): sidebar HEAD-row rename parity.
  if (kind !== 'localBranch') return commitItems;
  return [
    {
      label: 'Rename…',
      icon: createElement(BranchIcon),
      disabled: b.deps.mutating || b.deps.opActive,
      onSelect: () => b.deps.setPendingRenameBranch({ name: r.name }),
    },
    ...commitItems,
  ];
}

/** P92 §1.2: an entity's menu — its wire ref is resolved exactly as the pill
 *  hit-test does (`targetRefOf`: local-first for a collapsed local+remote pair),
 *  so a picker row and that ref's pill open the identical menu. */
function refItemsForEntity(
  b: GraphTargetBuilders,
  entity: RefEntity,
  oid: string,
): ContextMenuItem[] {
  const ref = targetRefOf(entity);
  if (ref === null) return [];
  return refItems(b, ref, oid);
}

export function buildGraphTargetItems(
  b: GraphTargetBuilders,
  target: GraphContextTarget,
): ContextMenuItem[] {
  if (target.kind === 'ref') return refItems(b, target.ref, target.oid);
  const forEntity = (e: RefEntity, oid: string) => refItemsForEntity(b, e, oid);
  // P92 §1.1: the "+N" chip → one row per HIDDEN entity, in groupRefs order.
  if (target.kind === 'refPicker') {
    return refPickerItems(target.entities, target.oid, forEntity);
  }
  // P92 §2.2: a commit / whole-row target. With ≥2 actionable refs the target is
  // genuinely ambiguous → prepend the picker level above the (unchanged) commit
  // actions. With ≤1 the menu is byte-identical to pre-P92: the P18b whole-row
  // branch fallback, else the plain commit menu.
  const entities = target.entities ?? [];
  if (entities.length > 0) {
    const candidates = actionableEntities(entities, target.oid, forEntity);
    if (candidates.length >= 2) {
      return [
        ...refPickerItems(candidates, target.oid, forEntity),
        { label: '', separator: true },
        ...b.commitMenuItems(target.oid),
      ];
    }
    // P18b (unchanged): the row carries a branch → that branch's menu (the
    // superset). Stash/tag/head-only rows fall through to the commit target.
    const ref = fallbackBranchRef(entities);
    if (ref !== null) {
      const items = refItems(b, ref, target.oid);
      if (items.length > 0) return items;
    }
  }
  // Commit row → Create branch here + Compare with HEAD.
  return b.commitMenuItems(target.oid);
}
