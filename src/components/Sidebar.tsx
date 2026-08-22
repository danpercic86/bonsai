import { useMemo, useState } from 'react';
import type {
  BranchesSnapshot,
  ListView,
  RemoteInfo,
  StashEntry,
  SubmoduleInfo,
  TagSyncReport,
  WorktreeInfo,
} from '../ipc';
import { DeleteIcon } from './menuIcons';
import { StashIcon } from './appIcons';
import { errorMessage } from '../utils/errors';
import { buildPathTree } from '../utils/pathTree';
import { Tree } from './Tree';
import { ListFilterInput } from './ListFilterInput';
import { SubmoduleRow } from './sidebar/SubmoduleRow';
import { SectionHeader } from './sidebar/SectionHeader';
import { TagsSection, type TagSyncState } from './sidebar/TagsSection';
import type { SubmoduleBusy } from './repoWorkspace/types';
import { filterItems, filterTree } from './repoWorkspace/listFilter';
import {
  BranchRow,
  ConfiguredRemoteRow,
  DetachedHeadRow,
  RemoteRow,
  SkeletonRows,
  StashRow,
  WorktreeRow,
} from './sidebar/rows';
import { SidebarTreeProvider } from './sidebar/SidebarTreeContext';
import { useSidebarTreeNav } from './sidebar/useSidebarTreeNav';

/** P50d: show a section's inline type-to-filter box only once the list is long
 *  enough to warrant it — keeps short lists uncluttered (contract §7). */
const FILTER_MIN_ROWS = 6;

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
  onStashContextMenu(index: number, oid: string, clientX: number, clientY: number): void;
  /** P19 §6.1: submodules with classified status. */
  submodules: SubmoduleInfo[];
  /** Right-click a submodule row → open the shared context menu at the cursor. */
  onSubmoduleContextMenu(name: string, clientX: number, clientY: number): void;
  /** P73 §6.1: the submodule row with an op in flight + its participle label. */
  submoduleBusy: SubmoduleBusy | null;
  /** P60d: click the section "+" → open the add-submodule (url + path) dialog. */
  onNewSubmodule(): void;
  /** P27 §6.1: worktrees (main first) with resolved branch/badges. */
  worktrees: WorktreeInfo[];
  /** Right-click a worktree row → open the shared context menu at the cursor. */
  onWorktreeContextMenu(name: string, clientX: number, clientY: number): void;
  /** Click the section "+" → open the new-worktree branch picker dialog. */
  onNewWorktree(): void;
  /** P22 §6.1: right-click a tag row → open the shared context menu. */
  onTagContextMenu(name: string, clientX: number, clientY: number): void;
  /** P77: best-effort tag-sync report (null until first check). The tags list
   *  NEVER blocks on it — it only augments rows with a status badge. */
  tagSyncReport: TagSyncReport | null;
  /** P77: the ls-remote lifecycle driving badge visibility (§2.2/§2.3). */
  tagSyncState: TagSyncState;
  /** P77 §2.3: the remote the check targets, available even without a successful
   *  report so the offline line can name it on cold-start-offline. */
  tagSyncRemote: string | null;
  /** P77: unix secs of the last successful check (for the "last checked" tip). */
  tagSyncCheckedAt: number | null;
  /** P77 §6: fired when the Tags section expands → the container runs listTagSync
   *  (cached ~10s). */
  onTagsExpand(): void;
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

/** Left sidebar: branches / remotes / tags (M5 contract §4.2). Presentational
 *  only — all fetching and git operations live in App.
 *
 *  P-a11y §D: the six sections compose one `role="tree"` (roving tabindex, single
 *  Tab stop). `useSidebarTreeNav` owns focus movement; each row wires itself as a
 *  `treeitem` via `useSidebarTreeItem`. */
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
  submoduleBusy,
  onNewSubmodule,
  worktrees,
  onWorktreeContextMenu,
  onNewWorktree,
  onTagContextMenu,
  tagSyncReport,
  tagSyncState,
  tagSyncRemote,
  tagSyncCheckedAt,
  onTagsExpand,
  remotes,
  onRemoteContextMenu,
  onAddRemote,
  onCleanupBranches,
}: SidebarProps) {
  const [branchesCollapsed, setBranchesCollapsed] = useState(false);
  const [remotesCollapsed, setRemotesCollapsed] = useState(false);
  const [stashesCollapsed, setStashesCollapsed] = useState(false);
  const [submodulesCollapsed, setSubmodulesCollapsed] = useState(false);
  const [worktreesCollapsed, setWorktreesCollapsed] = useState(false);

  // P50d: per-section inline filter queries (display-only; applied via
  // listFilter helpers below). Each is ignored while its box is hidden.
  const [branchFilter, setBranchFilter] = useState('');
  const [remoteFilter, setRemoteFilter] = useState('');

  const [createOpen, setCreateOpen] = useState(false);
  const [createValue, setCreateValue] = useState('');
  const [createError, setCreateError] = useState<string | null>(null);

  // P-a11y §D.4: the composite tree focus controller (roving tabindex + D.3
  // movement + D.6 context-menu focus restore).
  const nav = useSidebarTreeNav({ currentBranch });

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
        <SidebarTreeProvider value={nav.context}>
          <div className="sidebar-tree" {...nav.rootProps}>
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
                    <ul className="branch-list" role="group">
                      {remotesFiltered.map((r) => (
                        <ConfiguredRemoteRow
                          key={r.name}
                          remote={r}
                          onContextMenu={onRemoteContextMenu}
                          treeKey={`remote:${r.name}`}
                        />
                      ))}
                    </ul>
                  )}
                  {/* Existing remote-tracking-branch tree, filtered display only. */}
                  {(treeMode ? remoteTreeFiltered.length > 0 : remoteFlatFiltered.length > 0) &&
                    (treeMode ? (
                      <Tree
                        key={remoteFiltering ? 'remote-filter' : 'remote'}
                        asGroup
                        nodes={remoteTreeFiltered}
                        leafKey={(l) => l.item.name}
                        defaultCollapsed={!remoteFiltering}
                        initiallyExpanded={[]}
                        renderLeaf={(l, level) => (
                          <RemoteRow
                            name={l.item.name}
                            displayName={l.name}
                            onContextMenu={onContextMenu}
                            treeKey={`remote:${l.item.name}`}
                            level={level}
                          />
                        )}
                      />
                    ) : (
                      <ul className="branch-list" role="group">
                        {remoteFlatFiltered.map((r) => (
                          <RemoteRow
                            key={r.name}
                            name={r.name}
                            onContextMenu={onContextMenu}
                            treeKey={`remote:${r.name}`}
                          />
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

            <TagsSection
              tags={data.tags}
              treeMode={treeMode}
              onTagContextMenu={onTagContextMenu}
              tagSyncReport={tagSyncReport}
              tagSyncState={tagSyncState}
              tagSyncRemote={tagSyncRemote}
              tagSyncCheckedAt={tagSyncCheckedAt}
              onExpand={onTagsExpand}
            />

            <section className="sidebar-section">
              <SectionHeader
                label="Stashes"
                collapsed={stashesCollapsed}
                onToggle={() => setStashesCollapsed((c) => !c)}
                extra={
                  !data.head.unborn && (
                    <button
                      type="button"
                      className="sidebar-add sidebar-add-icon"
                      aria-label="Stash changes"
                      title="Stash changes"
                      disabled={actionsDisabled}
                      onClick={() => {
                        setStashesCollapsed(false);
                        onCreateStash();
                      }}
                    >
                      <StashIcon />
                    </button>
                  )
                }
              />
              {!stashesCollapsed &&
                (stashes.length === 0 ? (
                  <p className="branch-muted">No stashes</p>
                ) : (
                  <ul className="branch-list" role="group">
                    {stashes.map((s) => (
                      <StashRow
                        key={s.index}
                        index={s.index}
                        oid={s.oid}
                        message={s.message}
                        ts={s.ts}
                        onContextMenu={onStashContextMenu}
                        treeKey={`stash:${s.index}`}
                      />
                    ))}
                  </ul>
                ))}
            </section>

            {/* P60d: always shown so a submodule can be added even when none
                exist yet; the "+" opens the add (url + path) dialog. */}
            <section className="sidebar-section">
              <SectionHeader
                label="Submodules"
                collapsed={submodulesCollapsed}
                onToggle={() => setSubmodulesCollapsed((c) => !c)}
                extra={
                  <button
                    type="button"
                    className="sidebar-add"
                    aria-label="Add submodule"
                    title="Add submodule"
                    disabled={actionsDisabled}
                    onClick={() => {
                      setSubmodulesCollapsed(false);
                      onNewSubmodule();
                    }}
                  >
                    {'+'}
                  </button>
                }
              />
              {!submodulesCollapsed &&
                (submodules.length === 0 ? (
                  <p className="branch-muted">No submodules</p>
                ) : (
                  <ul className="branch-list" role="group">
                    {submodules.map((s) => (
                      <SubmoduleRow
                        key={s.name}
                        sub={s}
                        submoduleBusy={submoduleBusy}
                        onContextMenu={onSubmoduleContextMenu}
                        treeKey={`submodule:${s.name}`}
                      />
                    ))}
                  </ul>
                ))}
            </section>

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
                  <ul className="branch-list" role="group">
                    {worktrees.map((w) => (
                      <WorktreeRow
                        key={w.name}
                        wt={w}
                        onContextMenu={onWorktreeContextMenu}
                        treeKey={`worktree:${w.name}`}
                      />
                    ))}
                  </ul>
                ))}
            </section>
          </div>
        </SidebarTreeProvider>
      )}
    </aside>
  );
}
