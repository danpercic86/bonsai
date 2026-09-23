import { memo } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import type { RemoteBranchInfo, RemoteInfo } from '../../ipc';
import type { RevealTarget } from '../../graph/reveal';
import type { TreeNode } from '../../utils/pathTree';
import { Tree } from '../Tree';
import { ListFilterInput } from '../ListFilterInput';
import { SectionHeader } from './SectionHeader';
import { SidebarActionButton } from './SidebarActionButton';
import { ConfiguredRemoteRow, RemoteRow } from './rows';
import { useRenderCount } from '../../obs/react';

export interface RemotesSectionProps {
  /** NO `data: BranchesSnapshot` PROP (P118b) — the section read it exactly once,
   *  for the "no remotes" empty state, and the whole-snapshot coupling made any
   *  LOCAL-branch change re-render it. A boolean instead. */
  hasRemoteRefs: boolean;
  remotes: RemoteInfo[];
  remotesCollapsed: boolean;
  setRemotesCollapsed: Dispatch<SetStateAction<boolean>>;
  /** NO `actionsDisabled` PROP (P118) — see BranchesSection; the "+" button
   *  reads `SidebarBusyContext` itself. */
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
  remotesFiltered: readonly RemoteInfo[];
  remoteFlatFiltered: readonly RemoteBranchInfo[];
  remoteTreeFiltered: readonly TreeNode<RemoteBranchInfo>[];
  remoteNoMatch: boolean;
}

function RemotesSectionImpl({
  hasRemoteRefs,
  remotes,
  remotesCollapsed,
  setRemotesCollapsed,
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
  useRenderCount('RemotesSection', undefined, 'aggregate'); // §9.2
  return (
    <section className="sidebar-section">
      <SectionHeader
        label="Remotes"
        collapsed={remotesCollapsed}
        onToggle={() => setRemotesCollapsed((c) => !c)}
        extra={
          <SidebarActionButton
            label="Add remote"
            onClick={() => {
              setRemotesCollapsed(false);
              onAddRemote();
            }}
          >
            {'+'}
          </SidebarActionButton>
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
          {remotes.length === 0 && !hasRemoteRefs && (
            <p className="branch-muted">No remotes</p>
          )}
        </>
      )}
    </section>
  );
}

/** Render-storm fix — see BranchesSection for why a plain `memo` suffices. */
export const RemotesSection = memo(RemotesSectionImpl);
