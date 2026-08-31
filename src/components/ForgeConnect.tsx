import { useState, type MouseEvent } from 'react';
import type { ForgeKind } from '../ipc';

// P79 (§2): the connect form serves three flows. Only the heading, sub-line, an
// optional banner and the submit label differ — the token field, the
// per-provider scopes hint, the keychain note, the error banner and the submit
// handler are identical across all three.
export type ConnectMode = 'connect' | 'change' | 'reauth' | 'add';

// P62c: paste-a-PAT affordance shown when the host has no stored token. The
// token is submitted to the container (forgeSetToken → OS keychain); it is
// NEVER prefilled, autofilled, echoed, or persisted in the DOM beyond this
// controlled input. Password type keeps it masked on screen.
//
// P64c (contract §3e): the copy is per-provider — a short line naming the token
// type/scopes each forge needs plus a "create a token" help link. Presentational
// only; still a password input, keychain note, no prefill.

export interface ForgeConnectProps {
  provider: ForgeKind;
  host: string;
  owner: string;
  repo: string;
  submitting: boolean;
  error: string | null;
  onSubmit(token: string): void;
  /** P79: which flow this is — selects the heading/sub/submit copy + the reauth
   *  banner. Defaults to 'connect' so existing call sites are unchanged. */
  mode?: ConnectMode;
  /** P79: last-known login, for the change/reauth copy; falls back to `host`. */
  login?: string | null;
  /** P79: when provided, a Cancel button is shown beside submit (change mode).
   *  Omitted ⇒ no Cancel (first connect / reauth have no back path). */
  onCancel?(): void;
  /** P72: route "Create a token" through the openUrl IPC — a bare
   *  `target="_blank"` is a silent no-op in the native webview. REQUIRED (not
   *  optional) so a future call site cannot regress to a dead link. This
   *  component stays presentational: no ipc, no error handling. */
  onOpenUrl(url: string): void;
}

/** A modified/auxiliary click (ctrl/cmd/shift/alt, middle button) must reach the
 *  platform untouched, so open-in-new-tab keeps working in the browser harness;
 *  in the Tauri webview it is a no-op. `true` ⇒ do NOT intercept. */
function isPlatformClick(e: MouseEvent): boolean {
  return e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0;
}

/** Per-provider connect guidance. A full `Record<ForgeKind, …>` so adding a new
 *  `ForgeKind` (e.g. Azure DevOps in P64d) is a compile error until its hint is
 *  supplied. `unknown` never reaches this panel (the container shows the
 *  unsupported state instead) but carries a sensible generic fallback. */
interface ConnectHint {
  /** One line naming the required token type/scopes. */
  scopes: string;
  /** "Create a token" destination; empty ⇒ no link rendered. */
  url: string;
  /** Masked-input placeholder hinting the token's shape. */
  placeholder: string;
}

export const CONNECT_HINTS: Record<ForgeKind, ConnectHint> = {
  gitHub: {
    scopes:
      'Use a fine-grained token with Pull requests (read/write) and Contents (read) permissions — Metadata is added automatically — or a classic token with the "repo" scope.',
    url: 'https://github.com/settings/personal-access-tokens/new',
    placeholder: 'github_pat_…',
  },
  gitLab: {
    scopes:
      'Use a personal access token with the "api" scope. Read-only scopes such as "read_api" or "read_repository" are not enough to create merge requests.',
    url: 'https://gitlab.com/-/user_settings/personal_access_tokens',
    placeholder: 'glpat-…',
  },
  bitbucket: {
    scopes:
      'Use a repository or workspace access token with Pull requests (read and write). App passwords still work but Atlassian is retiring them during 2026, so prefer an access token.',
    url: 'https://support.atlassian.com/bitbucket-cloud/docs/create-a-repository-access-token/',
    placeholder: 'access token',
  },
  azureDevOps: {
    scopes: 'Use an Azure DevOps personal access token with Code (Read & Write).',
    url: 'https://learn.microsoft.com/azure/devops/organizations/accounts/use-personal-access-tokens-to-authenticate',
    placeholder: 'Azure DevOps PAT',
  },
  unknown: {
    scopes: 'Use a personal access token with read and write access to pull requests.',
    url: '',
    placeholder: 'token',
  },
};

export function ForgeConnect({
  provider,
  host,
  owner,
  repo,
  submitting,
  error,
  onSubmit,
  mode = 'connect',
  login,
  onCancel,
  onOpenUrl,
}: ForgeConnectProps) {
  const [token, setToken] = useState('');
  const canSubmit = !submitting && token.trim() !== '';
  const hint = CONNECT_HINTS[provider] ?? CONNECT_HINTS.unknown;
  const who = login ?? host;

  const heading =
    mode === 'change'
      ? `Replace token for ${who}`
      : mode === 'reauth'
        ? `Reconnect to ${host}`
        : mode === 'add'
          ? `Add another account for ${host}`
          : `Connect to ${host}`;
  const submitIdle =
    mode === 'change'
      ? 'Replace token'
      : mode === 'reauth'
        ? 'Reconnect'
        : mode === 'add'
          ? 'Add account'
          : 'Connect';
  const submitBusy =
    mode === 'change'
      ? 'Replacing…'
      : mode === 'reauth'
        ? 'Reconnecting…'
        : mode === 'add'
          ? 'Adding…'
          : 'Connecting…';

  return (
    <form
      className="forge-connect"
      autoComplete="off"
      onSubmit={(e) => {
        e.preventDefault();
        if (canSubmit) onSubmit(token);
      }}
    >
      {mode === 'reauth' && (
        <div className="forge-reauth-banner" role="status" aria-live="polite">
          <span className="forge-reauth-icon" aria-hidden="true">
            {'⚠'}
          </span>
          <span>
            {'Your saved token for '}
            <span className="mono">{host}</span>
            {
              ' expired or was revoked. Reconnect to keep viewing pull requests — your token stays saved until you replace it.'
            }
          </span>
        </div>
      )}
      <h3 className="forge-connect-heading">{heading}</h3>
      <p className="forge-connect-sub">
        {mode === 'change' ? (
          <>
            {'Paste a new token to replace the one saved for '}
            <span className="mono">{host}</span>
            {'. The current token keeps working until the new one is validated.'}
          </>
        ) : mode === 'reauth' ? (
          <>
            {'Paste a new token to reconnect. Your access to '}
            <span className="mono">{`${owner}/${repo}`}</span>
            {' is paused until then.'}
          </>
        ) : mode === 'add' ? (
          <>{'Paste a token for a different account. This repository will use the new account.'}</>
        ) : (
          <>
            {`Paste a personal access token to view and open pull requests for `}
            <span className="mono">{`${owner}/${repo}`}</span>
            {'.'}
          </>
        )}
      </p>
      <p className="forge-connect-hint">
        {hint.scopes}
        {hint.url !== '' && (
          <>
            {' '}
            <a
              className="forge-connect-link"
              href={hint.url}
              target="_blank"
              rel="noreferrer noopener"
              onClick={(e) => {
                if (isPlatformClick(e)) return; // let ctrl/middle-click through
                e.preventDefault();
                onOpenUrl(hint.url);
              }}
            >
              Create a token
            </a>
          </>
        )}
      </p>
      <label className="pr-field">
        <span className="pr-field-label">Personal access token</span>
        <input
          className="pr-input"
          type="password"
          autoComplete="off"
          autoCorrect="off"
          spellCheck={false}
          placeholder={hint.placeholder}
          value={token}
          disabled={submitting}
          onChange={(e) => setToken(e.target.value)}
        />
      </label>
      <p className="forge-connect-note">
        Stored in your OS keychain, never in a settings file. It is sent only as an
        authorization header and is never logged.
      </p>

      {error !== null && (
        <div className="error-banner error-banner-dismissible pr-error" role="alert">
          <span className="error-banner-text">{error}</span>
        </div>
      )}

      <div className="forge-connect-actions">
        <button type="submit" className="btn-primary forge-connect-submit" disabled={!canSubmit}>
          {submitting ? submitBusy : submitIdle}
        </button>
        {onCancel !== undefined && (
          <button
            type="button"
            className="btn-secondary"
            disabled={submitting}
            onClick={onCancel}
          >
            Cancel
          </button>
        )}
      </div>
    </form>
  );
}
