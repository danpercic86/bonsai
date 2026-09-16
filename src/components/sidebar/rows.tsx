// P-a11y §D: sidebar flat-row presentational components, extracted from
// Sidebar.tsx (kept under its size ceiling) and wired as `role="treeitem"` rows
// via `useSidebarTreeItem`. Each row is the single focusable element for its
// treeitem (roving tabindex); movement is owned by the tree root. Right-click
// (`onContextMenu`) and double-click (checkout) behaviour is byte-preserved from
// the original inline components — the keyboard path is additive.
//
// EVERY ROW IS `React.memo`d (render-storm fix, P91 follow-up). A repo with 500
// branches used to re-render all 500 rows on every container commit — ~26k
// BranchRow renders in one 6-minute Dev session. Two things make the memo
// actually hold, and both are load-bearing:
//   1. The callback props must be identity-stable across a container commit
//      (repoWorkspace/useSidebarCallbacks.ts + the stable context-menu openers).
//   2. The DATA prop must be compared STRUCTURALLY, not by reference: git2 +
//      serde hand back a brand-new `BranchInfo` on every `list_branches`, so a
//      plain shallow memo would never bail out. Hence `rowPropsEqual([...])`,
//      which names exactly the fresh-every-round object props.
// Rows taking only scalars (StashRow, RemoteRow, DetachedHeadRow) use the
// default shallow comparison.
import { memo } from 'react';
import type { BranchInfo, RemoteInfo, WorktreeInfo } from '../../ipc';
import type { RevealTarget } from '../../graph/reveal';
import { relativeDate } from '../../graph/draw';
import {
  CloudIcon,
  DetachedIcon,
  RefBranchIcon,
  RefDotIcon,
  StashIcon,
  WorktreeIcon,
} from '../appIcons';
import { RefFilterMarker } from './RefFilterMarker';
import { useSidebarTreeItem } from './useSidebarTreeItem';
import { useRenderCount } from '../../obs/react';
import { rowPropsEqual } from '../../utils/structuralEqual';

type BranchContextMenu = (
  name: string,
  kind: 'localBranch' | 'remoteBranch',
  clientX: number,
  clientY: number,
) => void;

export function AheadBehindBadge({ branch }: { branch: BranchInfo }) {
  const ahead = branch.ahead ?? 0;
  const behind = branch.behind ?? 0;
  if (branch.upstream === null || (ahead === 0 && behind === 0)) return null;
  const parts: string[] = [];
  if (ahead > 0) parts.push(`↑${ahead}`);
  if (behind > 0) parts.push(`↓${behind}`);
  return (
    <span className="branch-badge" title={`vs ${branch.upstream}`}>
      {parts.join(' ')}
    </span>
  );
}

export interface BranchRowProps {
  branch: BranchInfo;
  busy: boolean;
  onCheckout(name: string): void;
  onContextMenu: BranchContextMenu;
  /** P84: single-click reveals the branch tip in the graph (additive to
   *  double-click checkout; never hijacks keyboard). */
  onReveal?: (t: RevealTarget) => void;
  /** P3b tree mode: visible basename; ALL semantics (title, checkout, badge,
   *  head glyph, menu) keep using the full branch.name. */
  displayName?: string;
  treeKey: string;
  level?: number;
}

function BranchRowImpl({
  branch,
  busy,
  onCheckout,
  onContextMenu,
  onReveal,
  displayName,
  treeKey,
  level = 2,
}: BranchRowProps) {
  useRenderCount('BranchRow', undefined, 'aggregate'); // §9.2 — one shared tally key
  const isHead = branch.isHead;
  // HEAD branch: Enter no-op (already checked out). The keyboard menu opens for
  // every row — spec-003 gives the HEAD row a (filter-only) menu, matching
  // right-click, so the old `!isHead` gate would desync the two paths.
  const item = useSidebarTreeItem({
    treeKey,
    level,
    kind: 'leaf',
    ariaCurrent: isHead,
    onPrimary: isHead || busy ? undefined : () => onCheckout(branch.name),
    openMenuAt: (x, y) => onContextMenu(branch.name, 'localBranch', x, y),
  });
  return (
    <li
      {...item}
      role="treeitem"
      className={isHead ? 'branch-row branch-row-head' : 'branch-row'}
      onClick={() => onReveal?.({ kind: 'ref', name: branch.name })}
      onDoubleClick={() => {
        // GitKraken muscle memory: double-click checks out (contract §4.2).
        if (!isHead && !busy) onCheckout(branch.name);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(branch.name, 'localBranch', e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph">{isHead ? <RefDotIcon /> : <RefBranchIcon />}</span>
      <span className="branch-name" title={branch.name}>
        {displayName ?? branch.name}
      </span>
      {/* Spec-003 §3.3: solo/hidden marker after the name, before status pills. */}
      <RefFilterMarker fullRef={`refs/heads/${branch.name}`} />
      <AheadBehindBadge branch={branch} />
    </li>
  );
}

/** `branch` is a fresh object on every `list_branches` — compare it by value. */
export const BranchRow = memo(BranchRowImpl, rowPropsEqual<BranchRowProps>(['branch']));

function RemoteRowImpl({
  name,
  displayName,
  onContextMenu,
  onReveal,
  treeKey,
  level = 2,
}: {
  name: string;
  displayName?: string;
  onContextMenu: BranchContextMenu;
  /** P84: `name` is the full "origin/…" shorthand, matching RefLabel.name. */
  onReveal?: (t: RevealTarget) => void;
  treeKey: string;
  level?: number;
}) {
  useRenderCount('RemoteRow', undefined, 'aggregate'); // §9.2
  const item = useSidebarTreeItem({
    treeKey,
    level,
    kind: 'leaf',
    menuIsPrimary: true,
    openMenuAt: (x, y) => onContextMenu(name, 'remoteBranch', x, y),
  });
  return (
    <li
      {...item}
      role="treeitem"
      className="branch-row branch-row-readonly"
      onClick={() => onReveal?.({ kind: 'ref', name })}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(name, 'remoteBranch', e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph"><CloudIcon /></span>
      <span className="branch-name branch-name-muted" title={name}>
        {displayName ?? name}
      </span>
      {/* Spec-003 §3.3: `name` is the "origin/…" shorthand → refs/remotes/<name>. */}
      <RefFilterMarker fullRef={`refs/remotes/${name}`} />
    </li>
  );
}

/** Scalar props only → the default shallow comparison is enough. */
export const RemoteRow = memo(RemoteRowImpl);

/** P22 §6.2: a configured-remote row (name + fetch URL), distinct from the
 *  remote-tracking-branch rows. Right-click / Enter opens the manage menu. */
export interface ConfiguredRemoteRowProps {
  remote: RemoteInfo;
  onContextMenu(name: string, clientX: number, clientY: number): void;
  treeKey: string;
  level?: number;
}

function ConfiguredRemoteRowImpl({
  remote,
  onContextMenu,
  treeKey,
  level = 2,
}: ConfiguredRemoteRowProps) {
  useRenderCount('ConfiguredRemoteRow', undefined, 'aggregate'); // §9.2
  const item = useSidebarTreeItem({
    treeKey,
    level,
    kind: 'leaf',
    menuIsPrimary: true,
    openMenuAt: (x, y) => onContextMenu(remote.name, x, y),
  });
  return (
    <li
      {...item}
      role="treeitem"
      className="branch-row"
      title={remote.url ?? ''}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(remote.name, e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph"><CloudIcon /></span>
      <span className="branch-name" title={remote.name}>
        {remote.name}
      </span>
      {remote.url !== null && (
        <span className="branch-name branch-name-muted remote-url" title={remote.url}>
          {remote.url}
        </span>
      )}
    </li>
  );
}

/** `remote` is fresh on every `list_remotes` — compare it by value. */
export const ConfiguredRemoteRow = memo(
  ConfiguredRemoteRowImpl,
  rowPropsEqual<ConfiguredRemoteRowProps>(['remote']),
);

function StashRowImpl({
  index,
  oid,
  message,
  ts,
  now,
  onContextMenu,
  onReveal,
  treeKey,
  level = 2,
}: {
  index: number;
  oid: string;
  message: string;
  ts: number;
  /** Unix SECONDS, ticked by the container (see `SidebarProps.now`). Read from a
   *  prop, never `Date.now()`: this row is memoised, so a clock read in the
   *  render body would freeze the age label until the row changed for some
   *  other reason. */
  now: number;
  onContextMenu(index: number, oid: string, clientX: number, clientY: number): void;
  /** P84: stashes aren't ref-labelled in the graph → reveal by oid. */
  onReveal?: (t: RevealTarget) => void;
  treeKey: string;
  level?: number;
}) {
  useRenderCount('StashRow', undefined, 'aggregate'); // §9.2
  const label = `stash@{${index}}`;
  const item = useSidebarTreeItem({
    treeKey,
    level,
    kind: 'leaf',
    menuIsPrimary: true,
    // F-A6-B: pass the oid THIS row rendered so a later apply/pop/drop hits
    // exactly the entry the user saw, even if the stack shifts meanwhile.
    openMenuAt: (x, y) => onContextMenu(index, oid, x, y),
  });
  return (
    <li
      {...item}
      role="treeitem"
      className="branch-row"
      onClick={() => onReveal?.({ kind: 'oid', oid, label })}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(index, oid, e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph"><StashIcon /></span>
      <span className="mono" title={label}>
        {label}
      </span>
      <span className="branch-name branch-name-muted" title={message}>
        {message}
      </span>
      <span className="branch-badge" title={label}>
        {relativeDate(ts, now)}
      </span>
    </li>
  );
}

/** Scalar props only → the default shallow comparison is enough.
 *
 *  KNOWN CONSEQUENCE: `now` is read in the render body, so the relative age
 *  ("5 minutes ago") only re-reads the clock when this row actually re-renders.
 *  The render storm used to refresh it incidentally, every few seconds. Filed as
 *  a follow-up — the fix is an explicit slow tick, not un-memoising a row to get
 *  a clock for free. */
export const StashRow = memo(StashRowImpl);

/** P27 §6.2: display-only badge pills for a worktree row. A row may show more
 *  than one (e.g. current + main). Reuses the P19 badge intent classes. */
function worktreeBadges(wt: WorktreeInfo): { label: string; intent: string; title?: string }[] {
  const out: { label: string; intent: string; title?: string }[] = [];
  if (wt.isCurrent) out.push({ label: 'current', intent: 'submodule-badge-ok' });
  if (wt.isMain) out.push({ label: 'main', intent: 'submodule-badge-muted' });
  if (wt.locked)
    out.push({ label: 'locked', intent: 'submodule-badge-warn', title: wt.lockReason ?? 'locked' });
  if (wt.prunable || !wt.valid) out.push({ label: 'stale', intent: 'submodule-badge-warn' });
  return out;
}

export interface WorktreeRowProps {
  wt: WorktreeInfo;
  onContextMenu(name: string, clientX: number, clientY: number): void;
  treeKey: string;
  level?: number;
}

function WorktreeRowImpl({ wt, onContextMenu, treeKey, level = 2 }: WorktreeRowProps) {
  useRenderCount('WorktreeRow', undefined, 'aggregate'); // §9.2
  const item = useSidebarTreeItem({
    treeKey,
    level,
    kind: 'leaf',
    menuIsPrimary: true,
    openMenuAt: (x, y) => onContextMenu(wt.name, x, y),
  });
  return (
    <li
      {...item}
      role="treeitem"
      className="branch-row"
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(wt.name, e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph"><WorktreeIcon /></span>
      <span className="branch-name" title={wt.absPath}>
        {wt.name}
      </span>
      {wt.valid && (
        <span className="branch-name branch-name-muted" title={wt.branch ?? 'detached HEAD'}>
          {wt.branch ?? 'detached'}
        </span>
      )}
      {worktreeBadges(wt).map((b) => (
        <span key={b.label} className={`branch-badge ${b.intent}`} title={b.title ?? b.label}>
          {b.label}
        </span>
      ))}
    </li>
  );
}

/** `wt` is fresh on every `list_worktrees` — compare it by value. */
export const WorktreeRow = memo(WorktreeRowImpl, rowPropsEqual<WorktreeRowProps>(['wt']));

/** Detached-HEAD info row (§D.2): a readable, action-less level-2 treeitem
 *  (`aria-disabled`), so keyboard nav lands on it but Enter/menu are no-ops. */
function DetachedHeadRowImpl({
  oid,
  treeKey,
  level = 2,
}: {
  oid: string;
  treeKey: string;
  level?: number;
}) {
  useRenderCount('DetachedHeadRow', undefined, 'aggregate'); // §9.2
  const item = useSidebarTreeItem({ treeKey, level, kind: 'leaf', ariaDisabled: true });
  return (
    <li {...item} role="treeitem" className="branch-row branch-row-detached" title={oid}>
      <span className="branch-glyph"><DetachedIcon /></span>
      <span className="branch-name">
        HEAD detached @ <span className="mono">{oid.slice(0, 7)}</span>
      </span>
    </li>
  );
}

/** Scalar props only → the default shallow comparison is enough. */
export const DetachedHeadRow = memo(DetachedHeadRowImpl);

export function SkeletonRows() {
  return (
    <div className="skeleton-group" aria-hidden="true">
      {Array.from({ length: 3 }, (_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}
