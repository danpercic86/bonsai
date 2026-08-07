import { useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import type {
  BranchInfo,
  BranchesSnapshot,
  ListView,
  RemoteInfo,
  StashEntry,
  SubmoduleInfo,
  SubmoduleStatus,
  WorktreeInfo,
} from '../ipc';
import { relativeDate } from '../graph/draw';
import { DeleteIcon } from './menuIcons';
import { errorMessage } from '../utils/errors';
import { buildPathTree } from '../utils/pathTree';
import { Tree } from './Tree';
import { ListFilterInput } from './ListFilterInput';
import { filterByName, filterItems, filterTree } from './repoWorkspace/listFilter';

/** P50d: show a section's inline type-to-filter box only once the list is long
 *  enough to warrant it — keeps short lists uncluttered (contract §7). */
const FILTER_MIN_ROWS = 6;

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

/** P4d: proper ancestor folder prefixes of a branch name.
 *  "a/b/c" -> ["a", "a/b"]; root-level branch -> []. */
function ancestorPrefixes(name: string): string[] {
  const segs = name.split('/').filter(Boolean);
  const out: string[] = [];
  for (let i = 1; i < segs.length; i++) out.push(segs.slice(0, i).join('/'));
  return out;
}

export interface SidebarProps {
  data: BranchesSnapshot | null;
  loading: boolean;
  /** Sidebar-level op/list error; rendered as a dismissible banner at the top. */
  error: string | null;
  onDismissError(): void;
  /** Global mutating flag — disables every action. */
  busy: boolean;
  /** P3c §8.5: an operation (merge/rebase/…) is in progress — disables
   * checkout, delete, create-branch, and merge actions. */
  opActive: boolean;
  /** Current branch name (null when detached/unborn) — merge target; the
   * merge affordance is hidden without one. */
  currentBranch: string | null;
  onCheckout(name: string): void;
  /** P6 §4.6: right-click a branch/remote row → open the shared context menu at
   *  the cursor. */
  onContextMenu(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
    clientX: number,
    clientY: number,
  ): void;
  /** Resolves on success (input clears+closes); rejects with AppError (shown inline). */
  onCreateBranch(name: string): Promise<void>;
  /** P2a: persisted sidebar width in px, applied as inline style on the root. */
  width: number;
  /** P3b: flat (backend order) vs tree-grouped-by-'/' rendering of refs. */
  listView: ListView;
  /** P9 §6.2: stash stack, index 0 (most recent) first. */
  stashes: StashEntry[];
  /** "Stash changes" action — stash the dirty worktree. */
  onCreateStash(): void;
  /** Right-click a stash row → open the shared context menu at the cursor. */
  onStashContextMenu(index: number, clientX: number, clientY: number): void;
  /** P19 §6.1: submodules with classified status. */
  submodules: SubmoduleInfo[];
  /** Right-click a submodule row → open the shared context menu at the cursor. */
  onSubmoduleContextMenu(name: string, clientX: number, clientY: number): void;
  /** P27 §6.1: worktrees (main first) with resolved branch/badges. */
  worktrees: WorktreeInfo[];
  /** Right-click a worktree row → open the shared context menu at the cursor. */
  onWorktreeContextMenu(name: string, clientX: number, clientY: number): void;
  /** Click the section "+" → open the new-worktree branch picker dialog. */
  onNewWorktree(): void;
  /** P22 §6.1: right-click a tag row → open the shared context menu. */
  onTagContextMenu(name: string, clientX: number, clientY: number): void;
  /** P22 §6.2: configured remotes (name + fetch URL), rendered above the
   *  remote-tracking-branch tree. */
  remotes: RemoteInfo[];
  /** Right-click a configured-remote row → open the shared context menu. */
  onRemoteContextMenu(name: string, clientX: number, clientY: number): void;
  /** "Add remote" header action → opens the RemoteEditDialog. */
  onAddRemote(): void;
  /** P25d §6: "Clean up branches…" header action → opens the StaleBranchesDialog.
   *  Rendered only when there is a branch list (data present, not unborn). */
  onCleanupBranches?(): void;
}

function SectionHeader({
  label,
  collapsed,
  onToggle,
  extra,
}: {
  label: string;
  collapsed: boolean;
  onToggle(): void;
  extra?: ReactNode;
}) {
  return (
    <div className="sidebar-section-header">
      <button
        type="button"
        className="sidebar-section-toggle section-label"
        aria-expanded={!collapsed}
        onClick={onToggle}
      >
        <span className={`file-chevron${collapsed ? '' : ' file-chevron-open'}`}>{'›'}</span>
        {label}
      </button>
      {extra}
    </div>
  );
}

function AheadBehindBadge({ branch }: { branch: BranchInfo }) {
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

function BranchRow({
  branch,
  busy,
  onCheckout,
  onContextMenu,
  displayName,
}: {
  branch: BranchInfo;
  busy: boolean;
  onCheckout(name: string): void;
  onContextMenu(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
    clientX: number,
    clientY: number,
  ): void;
  /** P3b tree mode: visible basename; ALL semantics (title, checkout, badge,
   *  head glyph, menu) keep using the full branch.name. */
  displayName?: string;
}) {
  return (
    <li
      className={branch.isHead ? 'branch-row branch-row-head' : 'branch-row'}
      onDoubleClick={() => {
        // GitKraken muscle memory: double-click checks out (contract §4.2).
        if (!branch.isHead && !busy) onCheckout(branch.name);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(branch.name, 'localBranch', e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph">{branch.isHead ? '●' : '⎇'}</span>
      <span className="branch-name" title={branch.name}>
        {displayName ?? branch.name}
      </span>
      <AheadBehindBadge branch={branch} />
    </li>
  );
}

function RemoteRow({
  name,
  displayName,
  onContextMenu,
}: {
  name: string;
  displayName?: string;
  onContextMenu(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
    clientX: number,
    clientY: number,
  ): void;
}) {
  return (
    <li
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

function TagRow({
  name,
  displayName,
  onContextMenu,
}: {
  name: string;
  displayName?: string;
  onContextMenu(name: string, clientX: number, clientY: number): void;
}) {
  return (
    <li
      className="branch-row branch-row-readonly"
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(name, e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph">{'#'}</span>
      <span className="branch-name branch-name-muted" title={name}>
        {displayName ?? name}
      </span>
    </li>
  );
}

/** P22 §6.2: a configured-remote row (name + fetch URL), distinct from the
 *  remote-tracking-branch rows. Right-click opens the manage menu. */
function ConfiguredRemoteRow({
  remote,
  onContextMenu,
}: {
  remote: RemoteInfo;
  onContextMenu(name: string, clientX: number, clientY: number): void;
}) {
  return (
    <li
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

function StashRow({
  index,
  message,
  ts,
  onContextMenu,
}: {
  index: number;
  message: string;
  ts: number;
  onContextMenu(index: number, clientX: number, clientY: number): void;
}) {
  const label = `stash@{${index}}`;
  const now = Math.floor(Date.now() / 1000);
  return (
    <li
      className="branch-row"
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(index, e.clientX, e.clientY);
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

/** P19 §6.2: display-only status pill. Label + intent class per status. */
const SUBMODULE_BADGE: Record<SubmoduleStatus, { label: string; intent: string }> = {
  uninitialized: { label: 'not initialized', intent: 'submodule-badge-muted' },
  upToDate: { label: 'up to date', intent: 'submodule-badge-ok' },
  outOfSync: { label: 'out of sync', intent: 'submodule-badge-warn' },
  modifiedWorkdir: { label: 'modified', intent: 'submodule-badge-warn' },
};

function SubmoduleRow({
  sub,
  onContextMenu,
}: {
  sub: SubmoduleInfo;
  onContextMenu(name: string, clientX: number, clientY: number): void;
}) {
  const badge = SUBMODULE_BADGE[sub.status];
  return (
    <li
      className="branch-row"
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(sub.name, e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph">{'⊡'}</span>
      <span className="branch-name" title={sub.path}>
        {sub.name}
      </span>
      <span className={`branch-badge ${badge.intent}`} title={badge.label}>
        {badge.label}
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

function WorktreeRow({
  wt,
  onContextMenu,
}: {
  wt: WorktreeInfo;
  onContextMenu(name: string, clientX: number, clientY: number): void;
}) {
  return (
    <li
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
        <span
          className="branch-name branch-name-muted"
          title={wt.branch ?? 'detached HEAD'}
        >
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

function SkeletonRows() {
  return (
    <div className="skeleton-group" aria-hidden="true">
      {Array.from({ length: 3 }, (_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}

/** Left sidebar: branches / remotes / tags (M5 contract §4.2). Presentational
 *  only — all fetching and git operations live in App. */
export function Sidebar({
  data,
  loading,
  error,
  onDismissError,
  busy,
  opActive,
  currentBranch,
  onCheckout,
  onContextMenu,
  onCreateBranch,
  width,
  listView,
  stashes,
  onCreateStash,
  onStashContextMenu,
  submodules,
  onSubmoduleContextMenu,
  worktrees,
  onWorktreeContextMenu,
  onNewWorktree,
  onTagContextMenu,
  remotes,
  onRemoteContextMenu,
  onAddRemote,
  onCleanupBranches,
}: SidebarProps) {
  const [branchesCollapsed, setBranchesCollapsed] = useState(false);
  const [remotesCollapsed, setRemotesCollapsed] = useState(false);
  // P11a: Tags start collapsed by default (they are the least-used section and
  // can be long); the other sections stay expanded. Local/ephemeral state.
  const [tagsCollapsed, setTagsCollapsed] = useState(true);
  const [stashesCollapsed, setStashesCollapsed] = useState(false);
  const [submodulesCollapsed, setSubmodulesCollapsed] = useState(false);
  const [worktreesCollapsed, setWorktreesCollapsed] = useState(false);

  // P50d: per-section inline filter queries (display-only; applied via
  // listFilter helpers below). Each is ignored while its box is hidden.
  const [branchFilter, setBranchFilter] = useState('');
  const [remoteFilter, setRemoteFilter] = useState('');
  const [tagFilter, setTagFilter] = useState('');

  const [createOpen, setCreateOpen] = useState(false);
  const [createValue, setCreateValue] = useState('');
  const [createError, setCreateError] = useState<string | null>(null);

  // P3c §8.5: while an operation is in progress, every branch mutation
  // (checkout / delete / create / merge) is disabled.
  const actionsDisabled = busy || opActive;

  // P3b §5.3 — tree-grouped refs (display-only; full names drive all actions).
  const treeMode = listView === 'tree';
  const localTree = useMemo(
    () =>
      treeMode && data !== null
        ? buildPathTree(data.local, (b) => b.name, { priorityPath: currentBranch ?? undefined })
        : [],
    [treeMode, data, currentBranch],
  );
  // P4d: flat mode — current (HEAD) branch first, rest in backend order.
  const localFlat = useMemo(() => {
    if (data === null) return [];
    const head = data.local.filter((b) => b.isHead);
    const rest = data.local.filter((b) => !b.isHead);
    return [...head, ...rest];
  }, [data]);
  const remoteTree = useMemo(
    () => (treeMode && data !== null ? buildPathTree(data.remote, (r) => r.name) : []),
    [treeMode, data],
  );
  const tagTree = useMemo(
    () => (treeMode && data !== null ? buildPathTree(data.tags, (t) => t) : []),
    [treeMode, data],
  );

  // P50d — apply the per-section filters. The box shows only when the section
  // is expanded AND has ≥ FILTER_MIN_ROWS rows; while hidden, any stale query is
  // ignored so the full list is always restored. Filtering is display-only.
  const showBranchFilter = !branchesCollapsed && (data?.local.length ?? 0) >= FILTER_MIN_ROWS;
  const branchQuery = showBranchFilter ? branchFilter : '';
  const branchFiltering = branchQuery.trim() !== '';
  const localFlatFiltered = filterItems(localFlat, branchQuery, (b) => b.name);
  const localTreeFiltered = filterTree(localTree, branchQuery, (b) => b.name);
  const branchNoMatch = branchFiltering && localFlatFiltered.length === 0;

  // The Remotes section counts configured remotes + tracking rows and filters
  // both by the same query.
  const showRemoteFilter =
    !remotesCollapsed && remotes.length + (data?.remote.length ?? 0) >= FILTER_MIN_ROWS;
  const remoteQuery = showRemoteFilter ? remoteFilter : '';
  const remoteFiltering = remoteQuery.trim() !== '';
  const remotesFiltered = filterItems(remotes, remoteQuery, (r) => r.name);
  const remoteFlatFiltered = filterItems(data?.remote ?? [], remoteQuery, (r) => r.name);
  const remoteTreeFiltered = filterTree(remoteTree, remoteQuery, (r) => r.name);
  const remoteNoMatch =
    remoteFiltering && remotesFiltered.length === 0 && remoteFlatFiltered.length === 0;

  const showTagFilter = !tagsCollapsed && (data?.tags.length ?? 0) >= FILTER_MIN_ROWS;
  const tagQuery = showTagFilter ? tagFilter : '';
  const tagFiltering = tagQuery.trim() !== '';
  const tagsFiltered = filterByName(data?.tags ?? [], tagQuery);
  const tagTreeFiltered = filterTree(tagTree, tagQuery, (t) => t);
  const tagNoMatch = tagFiltering && tagsFiltered.length === 0;

  function closeCreate() {
    setCreateOpen(false);
    setCreateValue('');
    setCreateError(null);
  }

  async function submitCreate() {
    const trimmed = createValue.trim();
    if (trimmed === '' || actionsDisabled) return;
    try {
      await onCreateBranch(trimmed);
      closeCreate();
    } catch (e) {
      setCreateError(errorMessage(e));
    }
  }

  return (
    <aside className="sidebar" style={{ width }}>
      {error !== null && (
        <div className="error-banner error-banner-dismissible sidebar-error" role="alert">
          <span className="error-banner-text">{error}</span>
          <button
            type="button"
            className="error-dismiss"
            aria-label="Dismiss error"
            onClick={onDismissError}
          >
            {'×'}
          </button>
        </div>
      )}

      {data === null ? (
        loading && <SkeletonRows />
      ) : (
        <>
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
                <ul className="branch-list">
                  {data.head.detached && (
                    <li className="branch-row branch-row-detached" title={data.head.oid}>
                      <span className="branch-glyph">{'◎'}</span>
                      <span className="branch-name">
                        HEAD detached @ <span className="mono">{shortOid(data.head.oid)}</span>
                      </span>
                    </li>
                  )}
                  {!treeMode &&
                    localFlatFiltered.map((branch) => (
                      <BranchRow
                        key={branch.name}
                        branch={branch}
                        busy={actionsDisabled}
                        onCheckout={onCheckout}
                        onContextMenu={onContextMenu}
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
                    renderLeaf={(l) => (
                      <BranchRow
                        branch={l.item}
                        busy={actionsDisabled}
                        onCheckout={onCheckout}
                        onContextMenu={onContextMenu}
                        displayName={l.name}
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

          <section className="sidebar-section">
            <SectionHeader
              label="Remotes"
              collapsed={remotesCollapsed}
              onToggle={() => setRemotesCollapsed((c) => !c)}
              extra={
                <button
                  type="button"
                  className="sidebar-add"
                  aria-label="Add remote"
                  title="Add remote"
                  disabled={actionsDisabled}
                  onClick={() => {
                    setRemotesCollapsed(false);
                    onAddRemote();
                  }}
                >
                  {'+'}
                </button>
              }
            />
            {!remotesCollapsed && (
              <>
                {showRemoteFilter && (
                  <ListFilterInput
                    value={remoteFilter}
                    onChange={setRemoteFilter}
                    ariaLabel="Filter remotes"
                    count={
                      remoteFiltering
                        ? remotesFiltered.length + remoteFlatFiltered.length
                        : undefined
                    }
                  />
                )}
                {/* P22 §6.2: configured remotes on top (each right-clickable for
                    Rename / Edit URL / Remove), independent of tracking refs. */}
                {remotesFiltered.length > 0 && (
                  <ul className="branch-list">
                    {remotesFiltered.map((r) => (
                      <ConfiguredRemoteRow
                        key={r.name}
                        remote={r}
                        onContextMenu={onRemoteContextMenu}
                      />
                    ))}
                  </ul>
                )}
                {/* Existing remote-tracking-branch tree, filtered display only. */}
                {(treeMode ? remoteTreeFiltered.length > 0 : remoteFlatFiltered.length > 0) &&
                  (treeMode ? (
                    <Tree
                      key={remoteFiltering ? 'remote-filter' : 'remote'}
                      nodes={remoteTreeFiltered}
                      leafKey={(l) => l.item.name}
                      defaultCollapsed={!remoteFiltering}
                      initiallyExpanded={[]}
                      renderLeaf={(l) => (
                        <RemoteRow
                          name={l.item.name}
                          displayName={l.name}
                          onContextMenu={onContextMenu}
                        />
                      )}
                    />
                  ) : (
                    <ul className="branch-list">
                      {remoteFlatFiltered.map((r) => (
                        <RemoteRow key={r.name} name={r.name} onContextMenu={onContextMenu} />
                      ))}
                    </ul>
                  ))}
                {remoteNoMatch && (
                  <p className="branch-muted">{`No remotes match '${remoteFilter.trim()}'`}</p>
                )}
                {remotes.length === 0 && data.remote.length === 0 && (
                  <p className="branch-muted">No remotes</p>
                )}
              </>
            )}
          </section>

          <section className="sidebar-section">
            <SectionHeader
              label="Tags"
              collapsed={tagsCollapsed}
              onToggle={() => setTagsCollapsed((c) => !c)}
            />
            {!tagsCollapsed && (
              <>
                {showTagFilter && (
                  <ListFilterInput
                    value={tagFilter}
                    onChange={setTagFilter}
                    ariaLabel="Filter tags"
                    count={tagFiltering ? tagsFiltered.length : undefined}
                  />
                )}
                {data.tags.length === 0 ? (
                  <p className="branch-muted">No tags</p>
                ) : tagNoMatch ? (
                  <p className="branch-muted">{`No tags match '${tagFilter.trim()}'`}</p>
                ) : treeMode ? (
                  <Tree
                    key={tagFiltering ? 'tags-filter' : 'tags'}
                    nodes={tagTreeFiltered}
                    leafKey={(l) => l.item}
                    defaultCollapsed={!tagFiltering}
                    initiallyExpanded={[]}
                    renderLeaf={(l) => (
                      <TagRow name={l.item} displayName={l.name} onContextMenu={onTagContextMenu} />
                    )}
                  />
                ) : (
                  <ul className="branch-list">
                    {tagsFiltered.map((tag) => (
                      <TagRow key={tag} name={tag} onContextMenu={onTagContextMenu} />
                    ))}
                  </ul>
                )}
              </>
            )}
          </section>

          <section className="sidebar-section">
            <SectionHeader
              label="Stashes"
              collapsed={stashesCollapsed}
              onToggle={() => setStashesCollapsed((c) => !c)}
              extra={
                !data.head.unborn && (
                  <button
                    type="button"
                    className="sidebar-add"
                    aria-label="Stash changes"
                    title="Stash changes"
                    disabled={actionsDisabled}
                    onClick={() => {
                      setStashesCollapsed(false);
                      onCreateStash();
                    }}
                  >
                    {'⊟'}
                  </button>
                )
              }
            />
            {!stashesCollapsed &&
              (stashes.length === 0 ? (
                <p className="branch-muted">No stashes</p>
              ) : (
                <ul className="branch-list">
                  {stashes.map((s) => (
                    <StashRow
                      key={s.index}
                      index={s.index}
                      message={s.message}
                      ts={s.ts}
                      onContextMenu={onStashContextMenu}
                    />
                  ))}
                </ul>
              ))}
          </section>

          {submodules.length > 0 && (
            <section className="sidebar-section">
              <SectionHeader
                label="Submodules"
                collapsed={submodulesCollapsed}
                onToggle={() => setSubmodulesCollapsed((c) => !c)}
              />
              {!submodulesCollapsed && (
                <ul className="branch-list">
                  {submodules.map((s) => (
                    <SubmoduleRow
                      key={s.name}
                      sub={s}
                      onContextMenu={onSubmoduleContextMenu}
                    />
                  ))}
                </ul>
              )}
            </section>
          )}

          {/* P27 §6.1: Worktrees — always shown when a repo is open (the main
              row is always present in real repos). */}
          <section className="sidebar-section">
            <SectionHeader
              label="Worktrees"
              collapsed={worktreesCollapsed}
              onToggle={() => setWorktreesCollapsed((c) => !c)}
              extra={
                <button
                  type="button"
                  className="sidebar-add"
                  aria-label="New worktree"
                  title="New worktree"
                  disabled={actionsDisabled}
                  onClick={() => {
                    setWorktreesCollapsed(false);
                    onNewWorktree();
                  }}
                >
                  {'+'}
                </button>
              }
            />
            {!worktreesCollapsed &&
              (worktrees.length === 0 ? (
                <p className="branch-muted">No worktrees</p>
              ) : (
                <ul className="branch-list">
                  {worktrees.map((w) => (
                    <WorktreeRow key={w.name} wt={w} onContextMenu={onWorktreeContextMenu} />
                  ))}
                </ul>
              ))}
          </section>
        </>
      )}
    </aside>
  );
}
