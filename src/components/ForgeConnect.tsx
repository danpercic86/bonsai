import { useState } from 'react';

// P62c: paste-a-PAT affordance shown when the host has no stored token. The
// token is submitted to the container (forgeSetToken → OS keychain); it is
// NEVER prefilled, autofilled, echoed, or persisted in the DOM beyond this
// controlled input. Password type keeps it masked on screen.

export interface ForgeConnectProps {
  host: string;
  owner: string;
  repo: string;
  submitting: boolean;
  error: string | null;
  onSubmit(token: string): void;
}

export function ForgeConnect({ host, owner, repo, submitting, error, onSubmit }: ForgeConnectProps) {
  const [token, setToken] = useState('');
  const canSubmit = !submitting && token.trim() !== '';

  return (
    <form
      className="forge-connect"
      autoComplete="off"
      onSubmit={(e) => {
        e.preventDefault();
        if (canSubmit) onSubmit(token);
      }}
    >
      <h3 className="forge-connect-heading">{`Connect to ${host}`}</h3>
      <p className="forge-connect-sub">
        {`Paste a personal access token to view and open pull requests for `}
        <span className="mono">{`${owner}/${repo}`}</span>
        {'.'}
      </p>
      <label className="pr-field">
        <span className="pr-field-label">Personal access token</span>
        <input
          className="pr-input"
          type="password"
          autoComplete="off"
          autoCorrect="off"
          spellCheck={false}
          placeholder="ghp_…"
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

      <button type="submit" className="btn-primary forge-connect-submit" disabled={!canSubmit}>
        {submitting ? 'Connecting…' : 'Connect'}
      </button>
    </form>
  );
}
