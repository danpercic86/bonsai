import type { Dispatch, SetStateAction } from 'react';
import type { BranchesSnapshot, RemoteBranchInfo, RemoteInfo } from '../../ipc';
import type { RevealTarget } from '../../graph/reveal';
import type { TreeNode } from '../../utils/pathTree';
import { Tree } from '../Tree';
import { ListFilterInput } from '../ListFilterInput';
import { SectionHeader } from './SectionHeader';
import { ConfiguredRemoteRow, RemoteRow } from './rows';

export interface RemotesSectionProps {
  data: BranchesSnapshot;
  remotes: RemoteInfo[];
  remotesCollapsed: boolean;
  setRemotesCollapsed: Dispatch<SetStateAction<boolean>>;
  actionsDisabled: boolean;
  treeMode: boolean;
  onAddRemote(): void;
  onContextMenu(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
    clientX: number,
    clientY: number,
  ): void;
  onRemoteContextMenu(name: string, clientX: number, clientY: number): void;
  onReveal?: (t: RevealTarget) => void;
  showRemoteFilter: boolean;
  remoteFilter: string;
  setRemoteFilter: Dispatch<SetStateAction<string>>;
  remoteFiltering: boolean;
  remotesFiltered: RemoteInfo[];
  remoteFlatFiltered: RemoteBranchInfo[];
  remoteTreeFiltered: TreeNode<RemoteBranchInfo>[];
  remoteNoMatch: boolean;
}

export function RemotesSection({
  data,
  remotes,
  remotesCollapsed,
  setRemotesCollapsed,
  actionsDisabled,
  treeMode,
  onAddRemote,
  onContextMenu,
  onRemoteContextMenu,
  onReveal,
  showRemoteFilter,
  remoteFilter,
  setRemoteFilter,
  remoteFiltering,
  remotesFiltered,
  remoteFlatFiltered,
  remoteTreeFiltered,
  remoteNoMatch,
}: RemotesSectionProps) {
  return (
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
                    onReveal={onReveal}
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
                    onReveal={onReveal}
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
  );
}
