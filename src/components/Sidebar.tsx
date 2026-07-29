import { useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import type { BranchInfo, BranchesSnapshot, ListView } from '../ipc';
import { errorMessage } from '../utils/errors';
import { buildPathTree } from '../utils/pathTree';
import { ConfirmDialog } from './ConfirmDialog';
import { Tree } from './Tree';

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
  /** P3c §8.6: merge this branch (local or remote shorthand) into current. */
  onMergeBranch(name: string): void;
  /** P3d §8.6: rebase the current branch onto this branch (local or remote shorthand). */
  onRebaseBranch(name: string): void;
  /** Called ONLY after the confirmation dialog is confirmed (contract §4.3). */
  onDelete(name: string): void;
  /** Resolves on success (input clears+closes); rejects with AppError (shown inline). */
  onCreateBranch(name: string): Promise<void>;
  /** P1 §6.2: lifted so App's global shortcut handler can suppress bindings
   *  while the delete-branch ConfirmDialog is open. */
  onDialogOpenChange?(open: boolean): void;
  /** P2a: persisted sidebar width in px, applied as inline style on the root. */
  width: number;
  /** P3b: flat (backend order) vs tree-grouped-by-'/' rendering of refs. */
  listView: ListView;
}

function TrashIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M6.3 1.5h3.4l.6 1.2h3.2v1.4H2.5V2.7h3.2l.6-1.2zM3.4 5.4h9.2l-.7 8.6a1 1 0 0 1-1 .9H5.1a1 1 0 0 1-1-.9l-.7-8.6zM6 7v6h1.2V7H6zm2.8 0v6H10V7H8.8z" />
    </svg>
  );
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
  currentBranch,
  onCheckout,
  onMerge,
  onRebase,
  onAskDelete,
  displayName,
}: {
  branch: BranchInfo;
  busy: boolean;
  /** null = detached/unborn — the merge/rebase affordances are hidden. */
  currentBranch: string | null;
  onCheckout(name: string): void;
  onMerge(name: string): void;
  onRebase(name: string): void;
  onAskDelete(name: string): void;
  /** P3b tree mode: visible basename; ALL semantics (title, checkout, delete,
   *  badge, head glyph) keep using the full branch.name. */
  displayName?: string;
}) {
  return (
    <li
      className={branch.isHead ? 'branch-row branch-row-head' : 'branch-row'}
      onDoubleClick={() => {
        // GitKraken muscle memory: double-click checks out (contract §4.2).
        if (!branch.isHead && !busy) onCheckout(branch.name);
      }}
    >
      <span className="branch-glyph">{branch.isHead ? '●' : '⎇'}</span>
      <span className="branch-name" title={branch.name}>
        {displayName ?? branch.name}
      </span>
      <AheadBehindBadge branch={branch} />
      {!branch.isHead && (
        <>
          <button
            type="button"
            className="row-action"
            aria-label={`Checkout ${branch.name}`}
            title={`Checkout ${branch.name}`}
            disabled={busy}
            onClick={() => onCheckout(branch.name)}
          >
            {'⇄'}
          </button>
          {currentBranch !== null && (
            <button
              type="button"
              className="row-action"
              aria-label={`Merge ${branch.name} into ${currentBranch}`}
              title={`Merge ${branch.name} into ${currentBranch}`}
              disabled={busy}
              onClick={() => onMerge(branch.name)}
            >
              {'⇋'}
            </button>
          )}
          {currentBranch !== null && (
            <button
              type="button"
              className="row-action"
              aria-label={`Rebase ${currentBranch} onto ${branch.name}`}
              title={`Rebase ${currentBranch} onto ${branch.name}`}
              disabled={busy}
              onClick={() => onRebase(branch.name)}
            >
              {'⤵'}
            </button>
          )}
          <button
            type="button"
            className="row-action"
            aria-label={`Delete ${branch.name}`}
            title={`Delete ${branch.name}`}
            disabled={busy}
            onClick={() => onAskDelete(branch.name)}
          >
            <TrashIcon />
          </button>
        </>
      )}
    </li>
  );
}

function RemoteRow({
  name,
  displayName,
  busy,
  currentBranch,
  onMerge,
  onRebase,
}: {
  name: string;
  displayName?: string;
  busy: boolean;
  currentBranch: string | null;
  onMerge(name: string): void;
  onRebase(name: string): void;
}) {
  return (
    <li className="branch-row branch-row-readonly">
      <span className="branch-glyph">{'☁'}</span>
      <span className="branch-name branch-name-muted" title={name}>
        {displayName ?? name}
      </span>
      {currentBranch !== null && (
        <button
          type="button"
          className="row-action"
          aria-label={`Merge ${name} into ${currentBranch}`}
          title={`Merge ${name} into ${currentBranch}`}
          disabled={busy}
          onClick={() => onMerge(name)}
        >
          {'⇋'}
        </button>
      )}
      {currentBranch !== null && (
        <button
          type="button"
          className="row-action"
          aria-label={`Rebase ${currentBranch} onto ${name}`}
          title={`Rebase ${currentBranch} onto ${name}`}
          disabled={busy}
          onClick={() => onRebase(name)}
        >
          {'⤵'}
        </button>
      )}
    </li>
  );
}

function TagRow({ name, displayName }: { name: string; displayName?: string }) {
  return (
    <li className="branch-row branch-row-readonly">
      <span className="branch-glyph">{'#'}</span>
      <span className="branch-name branch-name-muted" title={name}>
        {displayName ?? name}
      </span>
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
  onMergeBranch,
  onRebaseBranch,
  onDelete,
  onCreateBranch,
  onDialogOpenChange,
  width,
  listView,
}: SidebarProps) {
  const [branchesCollapsed, setBranchesCollapsed] = useState(false);
  const [remotesCollapsed, setRemotesCollapsed] = useState(false);
  const [tagsCollapsed, setTagsCollapsed] = useState(false);

  const [createOpen, setCreateOpen] = useState(false);
  const [createValue, setCreateValue] = useState('');
  const [createError, setCreateError] = useState<string | null>(null);

  const [pendingDelete, setPendingDelete] = useState<string | null>(null);

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

  useEffect(() => {
    onDialogOpenChange?.(pendingDelete !== null);
  }, [pendingDelete, onDialogOpenChange]);

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
                )
              }
            />
            {!branchesCollapsed && (
              <>
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
                    localFlat.map((branch) => (
                      <BranchRow
                        key={branch.name}
                        branch={branch}
                        busy={actionsDisabled}
                        currentBranch={currentBranch}
                        onCheckout={onCheckout}
                        onMerge={onMergeBranch}
                        onRebase={onRebaseBranch}
                        onAskDelete={setPendingDelete}
                      />
                    ))}
                </ul>
                )}
                {treeMode && data.local.length > 0 && (
                  <Tree
                    key={`local:${currentBranch ?? 'none'}`}
                    nodes={localTree}
                    leafKey={(l) => l.item.name}
                    defaultCollapsed
                    initiallyExpanded={
                      currentBranch !== null ? ancestorPrefixes(currentBranch) : []
                    }
                    renderLeaf={(l) => (
                      <BranchRow
                        branch={l.item}
                        busy={actionsDisabled}
                        currentBranch={currentBranch}
                        onCheckout={onCheckout}
                        onMerge={onMergeBranch}
                        onRebase={onRebaseBranch}
                        onAskDelete={setPendingDelete}
                        displayName={l.name}
                      />
                    )}
                  />
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
            />
            {!remotesCollapsed &&
              (data.remote.length === 0 ? (
                <p className="branch-muted">No remotes</p>
              ) : treeMode ? (
                <Tree
                  nodes={remoteTree}
                  leafKey={(l) => l.item.name}
                  defaultCollapsed
                  initiallyExpanded={[]}
                  renderLeaf={(l) => (
                    <RemoteRow
                      name={l.item.name}
                      displayName={l.name}
                      busy={actionsDisabled}
                      currentBranch={currentBranch}
                      onMerge={onMergeBranch}
                      onRebase={onRebaseBranch}
                    />
                  )}
                />
              ) : (
                <ul className="branch-list">
                  {data.remote.map((r) => (
                    <RemoteRow
                      key={r.name}
                      name={r.name}
                      busy={actionsDisabled}
                      currentBranch={currentBranch}
                      onMerge={onMergeBranch}
                      onRebase={onRebaseBranch}
                    />
                  ))}
                </ul>
              ))}
          </section>

          <section className="sidebar-section">
            <SectionHeader
              label="Tags"
              collapsed={tagsCollapsed}
              onToggle={() => setTagsCollapsed((c) => !c)}
            />
            {!tagsCollapsed &&
              (data.tags.length === 0 ? (
                <p className="branch-muted">No tags</p>
              ) : treeMode ? (
                <Tree
                  nodes={tagTree}
                  leafKey={(l) => l.item}
                  defaultCollapsed
                  initiallyExpanded={[]}
                  renderLeaf={(l) => <TagRow name={l.item} displayName={l.name} />}
                />
              ) : (
                <ul className="branch-list">
                  {data.tags.map((tag) => (
                    <TagRow key={tag} name={tag} />
                  ))}
                </ul>
              ))}
          </section>
        </>
      )}

      <ConfirmDialog
        open={pendingDelete !== null}
        title="Delete branch"
        confirmLabel="Delete branch"
        busy={busy}
        onConfirm={() => {
          const name = pendingDelete;
          setPendingDelete(null);
          if (name !== null) onDelete(name);
        }}
        onCancel={() => setPendingDelete(null)}
      >
        <div>
          Delete branch {'"'}
          <span className="mono">{pendingDelete ?? ''}</span>
          {'"'}?
        </div>
        <div className="dialog-body-note">
          The branch is fully merged, but this cannot be undone from Bonsai.
        </div>
      </ConfirmDialog>
    </aside>
  );
}
