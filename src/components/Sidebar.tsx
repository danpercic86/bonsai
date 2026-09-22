import { useCallback, useMemo, useRef, useState } from 'react';
import type {
  BranchesSnapshot,
  ListView,
  RemoteBranchInfo,
  RemoteInfo,
  StashEntry,
  SubmoduleInfo,
  TagSyncReport,
  WorktreeInfo,
} from '../ipc';
import type { RevealTarget } from '../graph/reveal';
import { errorMessage } from '../utils/errors';
import { useRenderCount } from '../obs/react';
import { buildPathTree } from '../utils/pathTree';
import { SubmoduleRow } from './sidebar/SubmoduleRow';
import { SectionHeader } from './sidebar/SectionHeader';
import { StashesSection } from './sidebar/StashesSection';
import { TagsSection, type TagSyncState } from './sidebar/TagsSection';
import { BranchesSection } from './sidebar/BranchesSection';
import { RemotesSection } from './sidebar/RemotesSection';
import type { SubmoduleBusy } from './repoWorkspace/types';
import { filterItems, filterTree } from './repoWorkspace/listFilter';
import { SkeletonRows, WorktreeRow } from './sidebar/rows';
import { SidebarTreeProvider } from './sidebar/SidebarTreeContext';
import { SidebarBusyContext, SidebarBusyRefContext } from './sidebar/sidebarBusyContext';
import { useSidebarTreeNav } from './sidebar/useSidebarTreeNav';

/** P50d: show a section's inline type-to-filter box only once the list is long
 *  enough to warrant it — keeps short lists uncluttered (contract §7). */
const FILTER_MIN_ROWS = 6;

/** One stable empty array for the no-snapshot case, so the memo below does not
 *  mint a fresh `[]` identity on every render. */
const NO_REMOTE_BRANCHES: readonly RemoteBranchInfo[] = Object.freeze([]);

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
  /** P84: single-click a branch / remote / tag / stash row → reveal it in the
   *  graph (scroll + flash). Additive to double-click checkout; keyboard is
   *  intentionally out of scope. */
  onReveal?: (t: RevealTarget) => void;
  /** Render-storm fix (P91 follow-up): the CLOCK as an explicit prop — unix
   *  SECONDS, re-sampled by the container's 30 s `jobNow` ticker. The sections
   *  and rows are memoised, so a `Date.now()` in a render body freezes the
   *  relative-age labels; a ticked prop is what the memo comparators can SEE
   *  change. 30 s is the cadence these labels already had (that same ticker
   *  re-rendered the unmemoised Sidebar) and `relativeDate`'s finest bucket is
   *  one minute, so a label is never more than 30 s late. */
  now: number;
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
  onReveal,
  now,
}: SidebarProps) {
  // P91 §9.2 surface 6 — container render churn (each mode). The observed flicker
  // site; the section/row tallies aggregate underneath this record.
  useRenderCount('Sidebar', { data, loading, error, busy, currentBranch, listView });
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
  // P118: the same flag, in a stable box, for the handlers that only need it at
  // event time (BranchRow's checkout gesture, `submitCreate` below). Written in
  // the render body on purpose — this container re-renders on every flip, and no
  // reader looks at it during render, so the ref is always current by the time
  // an event can fire. Same idiom as repoWorkspace/useSidebarCallbacks.ts.
  const busyRef = useRef(actionsDisabled);
  busyRef.current = actionsDisabled;

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
  // Memoised like `localFlat`/`localTree` above: these five arrays are props of
  // the memoised sections, so a fresh identity per render would re-render every
  // section (and, before the row memos, every row) on any container commit.
  const localFlatFiltered = useMemo(
    () => filterItems(localFlat, branchQuery, (b) => b.name),
    [localFlat, branchQuery],
  );
  const localTreeFiltered = useMemo(
    () => filterTree(localTree, branchQuery, (b) => b.name),
    [localTree, branchQuery],
  );
  const branchNoMatch = branchFiltering && localFlatFiltered.length === 0;

  // The Remotes section counts configured remotes + tracking rows and filters
  // both by the same query.
  const showRemoteFilter =
    !remotesCollapsed && remotes.length + (data?.remote.length ?? 0) >= FILTER_MIN_ROWS;
  const remoteQuery = showRemoteFilter ? remoteFilter : '';
  const remoteFiltering = remoteQuery.trim() !== '';
  const remotesFiltered = useMemo(
    () => filterItems(remotes, remoteQuery, (r) => r.name),
    [remotes, remoteQuery],
  );
  // `data.remote` directly (not `?? []`) so an absent snapshot yields ONE stable
  // empty array instead of a fresh literal per render.
  const remoteFlatFiltered = useMemo(
    () => filterItems(data?.remote ?? NO_REMOTE_BRANCHES, remoteQuery, (r) => r.name),
    [data?.remote, remoteQuery],
  );
  const remoteTreeFiltered = useMemo(
    () => filterTree(remoteTree, remoteQuery, (r) => r.name),
    [remoteTree, remoteQuery],
  );
  const remoteNoMatch =
    remoteFiltering && remotesFiltered.length === 0 && remoteFlatFiltered.length === 0;

  // Both are props of the memoised BranchesSection — plain function
  // declarations would hand it a fresh identity on every container render.
  const closeCreate = useCallback(() => {
    setCreateOpen(false);
    setCreateValue('');
    setCreateError(null);
  }, []);

  const submitCreate = useCallback(async () => {
    const trimmed = createValue.trim();
    // `busyRef`, NOT `actionsDisabled` (P118): as a dep it reminted this
    // callback on every mutation flip, which re-rendered the memoised
    // BranchesSection — the very churn the context split removes.
    if (trimmed === '' || busyRef.current) return;
    try {
      await onCreateBranch(trimmed);
      closeCreate();
    } catch (e) {
      setCreateError(errorMessage(e));
    }
  }, [createValue, onCreateBranch, closeCreate]);

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
          {/* P118: two views of the in-flight flag — a stable ref box for
              event-time readers (rows), the boolean for the leaf controls that
              render `disabled`. Sections take neither as a prop. */}
          <SidebarBusyRefContext.Provider value={busyRef}>
          <SidebarBusyContext.Provider value={actionsDisabled}>
          <div className="sidebar-tree" {...nav.rootProps}>
            <BranchesSection
              data={data}
              branchesCollapsed={branchesCollapsed}
              setBranchesCollapsed={setBranchesCollapsed}
              onCleanupBranches={onCleanupBranches}
              treeMode={treeMode}
              currentBranch={currentBranch}
              onCheckout={onCheckout}
              onContextMenu={onContextMenu}
              onReveal={onReveal}
              showBranchFilter={showBranchFilter}
              branchFilter={branchFilter}
              setBranchFilter={setBranchFilter}
              branchFiltering={branchFiltering}
              localFlatFiltered={localFlatFiltered}
              localTreeFiltered={localTreeFiltered}
              branchNoMatch={branchNoMatch}
              createOpen={createOpen}
              setCreateOpen={setCreateOpen}
              createValue={createValue}
              setCreateValue={setCreateValue}
              createError={createError}
              setCreateError={setCreateError}
              closeCreate={closeCreate}
              submitCreate={submitCreate}
            />

            <RemotesSection
              data={data}
              remotes={remotes}
              remotesCollapsed={remotesCollapsed}
              setRemotesCollapsed={setRemotesCollapsed}
              treeMode={treeMode}
              onAddRemote={onAddRemote}
              onContextMenu={onContextMenu}
              onRemoteContextMenu={onRemoteContextMenu}
              onReveal={onReveal}
              showRemoteFilter={showRemoteFilter}
              remoteFilter={remoteFilter}
              setRemoteFilter={setRemoteFilter}
              remoteFiltering={remoteFiltering}
              remotesFiltered={remotesFiltered}
              remoteFlatFiltered={remoteFlatFiltered}
              remoteTreeFiltered={remoteTreeFiltered}
              remoteNoMatch={remoteNoMatch}
            />

            <TagsSection
              tags={data.tags}
              treeMode={treeMode}
              onTagContextMenu={onTagContextMenu}
              onReveal={onReveal}
              tagSyncReport={tagSyncReport}
              tagSyncState={tagSyncState}
              tagSyncRemote={tagSyncRemote}
              tagSyncCheckedAt={tagSyncCheckedAt}
              onExpand={onTagsExpand}
              now={now}
            />

            <StashesSection
              stashes={stashes}
              collapsed={stashesCollapsed}
              setCollapsed={setStashesCollapsed}
              headUnborn={data.head.unborn}
              onCreateStash={onCreateStash}
              onContextMenu={onStashContextMenu}
              onReveal={onReveal}
              now={now}
            />

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
          </SidebarBusyContext.Provider>
          </SidebarBusyRefContext.Provider>
        </SidebarTreeProvider>
      )}
    </aside>
  );
}
