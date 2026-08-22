// P-a11y §D: sidebar flat-row presentational components, extracted from
// Sidebar.tsx (kept under its size ceiling) and wired as `role="treeitem"` rows
// via `useSidebarTreeItem`. Each row is the single focusable element for its
// treeitem (roving tabindex); movement is owned by the tree root. Right-click
// (`onContextMenu`) and double-click (checkout) behaviour is byte-preserved from
// the original inline components — the keyboard path is additive.
import type { BranchInfo, RemoteInfo, WorktreeInfo } from '../../ipc';
import { relativeDate } from '../../graph/draw';
import { useSidebarTreeItem } from './useSidebarTreeItem';

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

export function BranchRow({
  branch,
  busy,
  onCheckout,
  onContextMenu,
  displayName,
  treeKey,
  level = 2,
}: {
  branch: BranchInfo;
  busy: boolean;
  onCheckout(name: string): void;
  onContextMenu: BranchContextMenu;
  /** P3b tree mode: visible basename; ALL semantics (title, checkout, badge,
   *  head glyph, menu) keep using the full branch.name. */
  displayName?: string;
  treeKey: string;
  level?: number;
}) {
  const isHead = branch.isHead;
  // HEAD branch: Enter no-op (already checked out) and no menu (branchMenuItems is
  // empty for HEAD, matching right-click). Non-HEAD: Enter = checkout (D.5).
  const item = useSidebarTreeItem({
    treeKey,
    level,
    kind: 'leaf',
    ariaCurrent: isHead,
    onPrimary: isHead || busy ? undefined : () => onCheckout(branch.name),
    openMenuAt: isHead ? undefined : (x, y) => onContextMenu(branch.name, 'localBranch', x, y),
  });
  return (
    <li
      {...item}
      role="treeitem"
      className={isHead ? 'branch-row branch-row-head' : 'branch-row'}
      onDoubleClick={() => {
        // GitKraken muscle memory: double-click checks out (contract §4.2).
        if (!isHead && !busy) onCheckout(branch.name);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(branch.name, 'localBranch', e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph">{isHead ? '●' : '⎇'}</span>
      <span className="branch-name" title={branch.name}>
        {displayName ?? branch.name}
      </span>
      <AheadBehindBadge branch={branch} />
    </li>
  );
}

export function RemoteRow({
  name,
  displayName,
  onContextMenu,
  treeKey,
  level = 2,
}: {
  name: string;
  displayName?: string;
  onContextMenu: BranchContextMenu;
  treeKey: string;
  level?: number;
}) {
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
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(name, 'remoteBranch', e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph">{'☁'}</span>
      <span className="branch-name branch-name-muted" title={name}>
        {displayName ?? name}
      </span>
    </li>
  );
}

/** P22 §6.2: a configured-remote row (name + fetch URL), distinct from the
 *  remote-tracking-branch rows. Right-click / Enter opens the manage menu. */
export function ConfiguredRemoteRow({
  remote,
  onContextMenu,
  treeKey,
  level = 2,
}: {
  remote: RemoteInfo;
  onContextMenu(name: string, clientX: number, clientY: number): void;
  treeKey: string;
  level?: number;
}) {
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
      <span className="branch-glyph">{'☁'}</span>
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

export function StashRow({
  index,
  oid,
  message,
  ts,
  onContextMenu,
  treeKey,
  level = 2,
}: {
  index: number;
  oid: string;
  message: string;
  ts: number;
  onContextMenu(index: number, oid: string, clientX: number, clientY: number): void;
  treeKey: string;
  level?: number;
}) {
  const label = `stash@{${index}}`;
  const now = Math.floor(Date.now() / 1000);
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
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(index, oid, e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph">{'⊟'}</span>
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

export function WorktreeRow({
  wt,
  onContextMenu,
  treeKey,
  level = 2,
}: {
  wt: WorktreeInfo;
  onContextMenu(name: string, clientX: number, clientY: number): void;
  treeKey: string;
  level?: number;
}) {
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
      <span className="branch-glyph">{'⌥'}</span>
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

/** Detached-HEAD info row (§D.2): a readable, action-less level-2 treeitem
 *  (`aria-disabled`), so keyboard nav lands on it but Enter/menu are no-ops. */
export function DetachedHeadRow({
  oid,
  treeKey,
  level = 2,
}: {
  oid: string;
  treeKey: string;
  level?: number;
}) {
  const item = useSidebarTreeItem({ treeKey, level, kind: 'leaf', ariaDisabled: true });
  return (
    <li {...item} role="treeitem" className="branch-row branch-row-detached" title={oid}>
      <span className="branch-glyph">{'◎'}</span>
      <span className="branch-name">
        HEAD detached @ <span className="mono">{oid.slice(0, 7)}</span>
      </span>
    </li>
  );
}

export function SkeletonRows() {
  return (
    <div className="skeleton-group" aria-hidden="true">
      {Array.from({ length: 3 }, (_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}
