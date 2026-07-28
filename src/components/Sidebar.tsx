import { useState } from 'react';
import type { ReactNode } from 'react';
import type { AppError, BranchInfo, BranchesSnapshot } from '../ipc';
import { ConfirmDialog } from './ConfirmDialog';

function isAppError(e: unknown): e is AppError {
  return (
    typeof e === 'object' &&
    e !== null &&
    'kind' in e &&
    'message' in e &&
    typeof (e as { message: unknown }).message === 'string'
  );
}

function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

export interface SidebarProps {
  data: BranchesSnapshot | null;
  loading: boolean;
  /** Sidebar-level op/list error; rendered as a dismissible banner at the top. */
  error: string | null;
  onDismissError(): void;
  /** Global mutating flag — disables every action. */
  busy: boolean;
  onCheckout(name: string): void;
  /** Called ONLY after the confirmation dialog is confirmed (contract §4.3). */
  onDelete(name: string): void;
  /** Resolves on success (input clears+closes); rejects with AppError (shown inline). */
  onCreateBranch(name: string): Promise<void>;
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
  onCheckout,
  onAskDelete,
}: {
  branch: BranchInfo;
  busy: boolean;
  onCheckout(name: string): void;
  onAskDelete(name: string): void;
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
        {branch.name}
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

function SkeletonRows() {
  return (
    <div aria-hidden="true">
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
  onCheckout,
  onDelete,
  onCreateBranch,
}: SidebarProps) {
  const [branchesCollapsed, setBranchesCollapsed] = useState(false);
  const [remotesCollapsed, setRemotesCollapsed] = useState(false);
  const [tagsCollapsed, setTagsCollapsed] = useState(false);

  const [createOpen, setCreateOpen] = useState(false);
  const [createValue, setCreateValue] = useState('');
  const [createError, setCreateError] = useState<string | null>(null);

  const [pendingDelete, setPendingDelete] = useState<string | null>(null);

  function closeCreate() {
    setCreateOpen(false);
    setCreateValue('');
    setCreateError(null);
  }

  async function submitCreate() {
    const trimmed = createValue.trim();
    if (trimmed === '' || busy) return;
    try {
      await onCreateBranch(trimmed);
      closeCreate();
    } catch (e) {
      setCreateError(errorMessage(e));
    }
  }

  return (
    <aside className="sidebar">
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
                    disabled={busy}
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
                      disabled={busy}
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
                <ul className="branch-list">
                  {data.head.detached && (
                    <li className="branch-row branch-row-detached" title={data.head.oid}>
                      <span className="branch-glyph">{'◎'}</span>
                      <span className="branch-name">
                        HEAD detached @ <span className="mono">{shortOid(data.head.oid)}</span>
                      </span>
                    </li>
                  )}
                  {data.local.map((branch) => (
                    <BranchRow
                      key={branch.name}
                      branch={branch}
                      busy={busy}
                      onCheckout={onCheckout}
                      onAskDelete={setPendingDelete}
                    />
                  ))}
                </ul>
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
              ) : (
                <ul className="branch-list">
                  {data.remote.map((r) => (
                    <li key={r.name} className="branch-row branch-row-readonly">
                      <span className="branch-glyph">{'☁'}</span>
                      <span className="branch-name branch-name-muted" title={r.name}>
                        {r.name}
                      </span>
                    </li>
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
              ) : (
                <ul className="branch-list">
                  {data.tags.map((tag) => (
                    <li key={tag} className="branch-row branch-row-readonly">
                      <span className="branch-glyph">{'#'}</span>
                      <span className="branch-name branch-name-muted" title={tag}>
                        {tag}
                      </span>
                    </li>
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
