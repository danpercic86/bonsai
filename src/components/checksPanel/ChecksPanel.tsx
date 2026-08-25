import { useState } from 'react';
import { ipc } from '../../ipc';
import type { CommitStatus } from '../../ipc';
import { usePushToast } from '../../ToastContext';
import { errorMessage } from '../../utils/errors';
import { SkeletonRows } from '../CommitPanel';
import { ForgeAccountHeader } from '../ForgeAccountHeader';
import { ForgeConnect, type ConnectMode } from '../ForgeConnect';
import { useForgeAccountCache } from '../prPanel/useForgeAccountCache';
import type { ChecksTarget } from './checksTarget';
import { ChecksHeader } from './ChecksHeader';
import { ChecksList } from './ChecksList';
import {
  ChecksError,
  ChecksIdle,
  ChecksNoChecks,
  ChecksNoForge,
  ChecksNoUpstream,
  ChecksWaiting,
} from './ChecksEmptyState';
import { ChecksAnnouncer } from './ChecksAnnouncer';
import { useBranchChecks } from './useBranchChecks';

// P90: right-pane Checks CONTAINER. Mounted only while rightPaneTab === 'checks'.
// Resolves the sidebar-selected branch tip → forgeCommitStatuses, switches on the
// ChecksState machine, and composes the presentational children. Reuses the PR
// panel's forge connect / account-header / openUrl story verbatim.

export interface ChecksPanelProps {
  repoId: string;
  /** The branch resolved from the last sidebar reveal (or HEAD); null ⇒ idle. */
  target: ChecksTarget | null;
  /** Bumped on fetch/pull to force a silent refetch. */
  refreshSeq: number;
  /** True only while this tab is the active right-pane tab. */
  active: boolean;
  /** Reveal a commit oid in the graph (tip-sha affordance). */
  onRevealCommit?(oid: string): void;
  /** §4.4: push the checks target (defined only when it is the current branch). */
  onPush?(): void;
  /** Open Settings → Accounts (header kebab "Manage accounts…"). */
  onManageAccounts?(): void;
}

export function ChecksPanel({
  repoId,
  target,
  refreshSeq,
  active,
  onRevealCommit,
  onPush,
  onManageAccounts,
}: ChecksPanelProps) {
  const pushToast = usePushToast();
  const { state, ctx, lastUpdated, failedRefreshAt, refreshing, refresh, reconnect } =
    useBranchChecks({
      repoId,
      target,
      refreshSeq,
      active,
    });

  const account = useForgeAccountCache(repoId, reconnect);

  const [connecting, setConnecting] = useState(false);
  const [connectError, setConnectError] = useState<string | null>(null);
  const [connectMode, setConnectMode] = useState<ConnectMode>('connect');
  // Header-initiated connect (change/add) overlays the loaded view.
  const [forcedConnect, setForcedConnect] = useState(false);

  function handleConnect(token: string) {
    setConnecting(true);
    setConnectError(null);
    void ipc.forgeSetToken(repoId, token).then(
      () => {
        setConnecting(false);
        setForcedConnect(false);
        setConnectMode('connect');
        if (account.accounts !== null) account.refetchAccounts();
        reconnect();
      },
      (e: unknown) => {
        setConnecting(false);
        setConnectError(errorMessage(e));
        pushToast('error', `Could not connect: ${errorMessage(e)}`);
      },
    );
  }

  function openUrl(url: string) {
    void ipc
      .openUrl(url)
      .catch((e: unknown) => pushToast('error', `Could not open the check page: ${errorMessage(e)}`));
  }

  const showHeader =
    ctx?.viewer != null && (state.kind === 'loaded' || state.kind === 'noChecks');

  const inConnect = state.kind === 'connect' || forcedConnect;

  return (
    <div className="checks-panel" role="tabpanel" aria-label="Checks">
      <ChecksAnnouncer state={state} refreshing={refreshing} />
      {showHeader && ctx?.viewer != null && (
        <ForgeAccountHeader
          viewer={ctx.viewer}
          host={ctx.host}
          kind={ctx.provider}
          accountSource={ctx.accountSource}
          resolvedAccountId={ctx.resolvedAccountId}
          accounts={account.accounts === null ? null : account.accounts.filter((a) => a.host === ctx.host)}
          accountsError={account.accountsError}
          busy={account.accountsBusy}
          onOpenMenu={account.handleOpenAccountMenu}
          onSelectAccount={account.handleSelectAccount}
          onUseHostDefault={account.handleUseHostDefault}
          onAddAnother={() => {
            setConnectError(null);
            setConnectMode('add');
            setForcedConnect(true);
          }}
          onChangeToken={() => {
            setConnectError(null);
            setConnectMode('change');
            setForcedConnect(true);
          }}
          onResetToDefault={account.handleResetToDefault}
          onManageAccounts={() => onManageAccounts?.()}
        />
      )}
      {inConnect ? (
        <ForgeConnect
          provider={ctx?.provider ?? 'unknown'}
          host={ctx?.host ?? 'the forge'}
          owner={ctx?.owner ?? ''}
          repo={ctx?.repo ?? ''}
          submitting={connecting}
          error={connectError}
          mode={connectMode}
          login={ctx?.viewer?.login ?? null}
          onSubmit={handleConnect}
          onCancel={
            forcedConnect
              ? () => {
                  setForcedConnect(false);
                  setConnectMode('connect');
                  setConnectError(null);
                }
              : undefined
          }
          onOpenUrl={openUrl}
        />
      ) : (
        renderBody()
      )}
    </div>
  );

  function header(target: ChecksTarget, status: CommitStatus | null) {
    return (
      <ChecksHeader
        target={target}
        status={status}
        refreshing={refreshing}
        lastUpdated={lastUpdated}
        failedRefreshAt={failedRefreshAt}
        onRefresh={refresh}
        onRevealCommit={onRevealCommit}
      />
    );
  }

  function renderBody() {
    switch (state.kind) {
      case 'idle':
        return <ChecksIdle />;
      case 'loading':
        return (
          <>
            {header(state.target, null)}
            <div className="checks-loading">
              <SkeletonRows />
            </div>
          </>
        );
      case 'noForge':
        return <ChecksNoForge host={ctx?.host ?? null} />;
      case 'noChecks':
        return (
          <>
            {header(state.target, null)}
            {state.reason === 'no-upstream' ? (
              <ChecksNoUpstream target={state.target} onPush={onPush} />
            ) : state.reason === 'waiting' ? (
              <ChecksWaiting target={state.target} />
            ) : (
              <ChecksNoChecks target={state.target} />
            )}
          </>
        );
      case 'error':
        // Stale-while-error (§4.10): keep the last-good header + rows under the
        // banner instead of blanking the whole panel.
        if (state.stale !== null) {
          return (
            <>
              {header(state.target, state.stale)}
              <ChecksError message={state.message} onRetry={refresh} />
              <ChecksList status={state.stale} onOpen={openUrl} />
            </>
          );
        }
        return <ChecksError message={state.message} onRetry={refresh} />;
      case 'loaded':
        return (
          <>
            {header(state.target, state.status)}
            <ChecksList status={state.status} onOpen={openUrl} />
          </>
        );
      default:
        return null;
    }
  }
}
