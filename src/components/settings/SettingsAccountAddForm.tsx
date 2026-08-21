// P79 §3.5 — the add-a-token-for-a-host form. Provider kind (radiogroup, Azure
// disabled per OD-2) + host + masked token → forgeSetTokenForHost. A bordered
// card reusing `.settings-profile` chrome under `.settings-account-add`.
import { useState, type MouseEvent } from 'react';

import { ipc } from '../../ipc';
import type { ForgeKind } from '../../ipc';
import { errorMessage, isAppError } from '../../utils/errors';
import { CONNECT_HINTS } from '../ForgeConnect';
import { forgeKindLabel as kindLabel } from '../ForgeProviderBadge';

const AZURE_HINT =
  'Azure DevOps accounts must be added from an open Azure DevOps repository — its sign-in needs a repository to verify against.';

/** Selectable provider kinds, in display order. Azure is present but disabled. */
const KIND_OPTIONS: readonly ForgeKind[] = ['gitHub', 'gitLab', 'bitbucket', 'azureDevOps'];

const HOST_PLACEHOLDER: Partial<Record<ForgeKind, string>> = {
  gitHub: 'github.com',
  gitLab: 'gitlab.com',
  bitbucket: 'bitbucket.org',
};

export interface SettingsAccountAddFormProps {
  /** Collapse the form (Cancel or after a successful add). */
  onCancel(): void;
  /** A token was validated + stored for `host` (login for the success toast). */
  onSuccess(host: string, login: string): void;
  onOpenUrl(url: string): void;
}

function isPlatformClick(e: MouseEvent): boolean {
  return e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0;
}

/** Map a rejected add to friendly copy (§3.5). */
function addError(e: unknown, host: string): string {
  if (isAppError(e)) {
    switch (e.kind) {
      case 'authFailed':
        return "That token was rejected. Check it has the required scopes and hasn't expired.";
      case 'forgeUnsupported':
        return AZURE_HINT;
      case 'forgeRateLimited':
        return 'The forge is rate-limiting requests — try again in a few minutes.';
      case 'networkError':
        return `Couldn't reach ${host}. Check your connection.`;
      default:
        return e.message;
    }
  }
  return errorMessage(e);
}

export function SettingsAccountAddForm({
  onCancel,
  onSuccess,
  onOpenUrl,
}: SettingsAccountAddFormProps) {
  const [kind, setKind] = useState<ForgeKind>('gitHub');
  const [host, setHost] = useState('');
  const [token, setToken] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const hint = CONNECT_HINTS[kind] ?? CONNECT_HINTS.unknown;
  const isAzure = kind === 'azureDevOps';
  const canSubmit = !submitting && !isAzure && host.trim() !== '' && token.trim() !== '';
  const hintId = 'account-add-azure-hint';

  const submit = () => {
    if (!canSubmit) return;
    const normalizedHost = host.trim().toLowerCase();
    setSubmitting(true);
    setError(null);
    void ipc.forgeSetTokenForHost(normalizedHost, kind, token).then(
      (viewer) => {
        setSubmitting(false);
        onSuccess(normalizedHost, viewer.login);
      },
      (e: unknown) => {
        setSubmitting(false);
        setError(addError(e, normalizedHost));
      },
    );
  };

  return (
    <form
      className="settings-account-add"
      autoComplete="off"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <fieldset className="settings-account-kinds" role="radiogroup" aria-label="Provider">
        {KIND_OPTIONS.map((k) => {
          const disabled = k === 'azureDevOps';
          return (
            <label key={k} className="settings-account-kind">
              <input
                type="radio"
                name="account-add-kind"
                value={k}
                checked={kind === k}
                disabled={disabled}
                aria-disabled={disabled}
                aria-describedby={disabled ? hintId : undefined}
                onChange={() => setKind(k)}
              />
              <span>{kindLabel(k)}</span>
            </label>
          );
        })}
      </fieldset>
      {KIND_OPTIONS.includes('azureDevOps') && (
        <p className="settings-row-help" id={hintId}>
          {AZURE_HINT}
        </p>
      )}

      <label className="settings-config-field-label" htmlFor="account-add-host">
        Host
      </label>
      <input
        id="account-add-host"
        className="settings-config-field"
        type="text"
        autoComplete="off"
        placeholder={HOST_PLACEHOLDER[kind] ?? 'host'}
        value={host}
        disabled={submitting}
        onChange={(e) => setHost(e.target.value)}
      />

      <label className="settings-config-field-label" htmlFor="account-add-token">
        Personal access token
      </label>
      <input
        id="account-add-token"
        className="settings-config-field"
        type="password"
        autoComplete="off"
        autoCorrect="off"
        spellCheck={false}
        placeholder={hint.placeholder}
        value={token}
        disabled={submitting}
        onChange={(e) => setToken(e.target.value)}
      />
      <p className="settings-row-help">
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
                if (isPlatformClick(e)) return;
                e.preventDefault();
                onOpenUrl(hint.url);
              }}
            >
              Create a token
            </a>
          </>
        )}
      </p>
      <p className="settings-row-help">
        {
          'Stored in your OS keychain, never in a settings file. It is sent only as an authorization header and is never logged.'
        }
      </p>

      {error !== null && (
        <div className="error-banner error-banner-dismissible" role="alert">
          <span className="error-banner-text">{error}</span>
        </div>
      )}

      <div className="settings-account-form-actions">
        <button
          type="submit"
          className="btn-primary settings-toggle-btn"
          disabled={!canSubmit}
          title={isAzure ? AZURE_HINT : undefined}
        >
          {submitting ? 'Adding…' : 'Add account'}
        </button>
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          disabled={submitting}
          onClick={onCancel}
        >
          Cancel
        </button>
      </div>
    </form>
  );
}
