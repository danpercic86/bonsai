// P79 §3.3–3.4 — one connected (or previously-connected) forge account, with its
// inline "Change token" form. Reuses `.settings-profile` chrome under
// `.settings-account`. The Remove confirm is owned by the parent section (§3.6);
// this card only requests it.
import { useEffect, useRef, useState, type MouseEvent } from 'react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { ContextMenu, type ContextMenuItem } from '../ContextMenu';
import { ForgeAvatar } from '../ForgeAvatar';
import { CONNECT_HINTS } from '../ForgeConnect';
import { ForgeProviderBadge } from '../ForgeProviderBadge';

export interface SettingsAccountCardProps {
  account: ForgeAccount;
  /** Refetch the list after a successful token replace. */
  onChanged(): void;
  /** Ask the parent to open the Remove confirm for this host. */
  onRequestRemove(): void;
  /** IPC-routed "Create a token" link (a bare target=_blank is a no-op natively). */
  onOpenUrl(url: string): void;
}

/** A modified/auxiliary click must reach the platform untouched (open-in-new-tab
 *  in the browser harness). `true` ⇒ do NOT intercept. */
function isPlatformClick(e: MouseEvent): boolean {
  return e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0;
}

export function SettingsAccountCard({
  account,
  onChanged,
  onRequestRemove,
  onOpenUrl,
}: SettingsAccountCardProps) {
  const { host, kind, login, avatarUrl, connected } = account;
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [editing, setEditing] = useState(false);
  const [token, setToken] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const kebabRef = useRef<HTMLButtonElement>(null);
  const tokenRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) tokenRef.current?.focus();
  }, [editing]);

  const titleId = `account-card-${host}-title`;
  const title = login ?? host;
  const hint = CONNECT_HINTS[kind] ?? CONNECT_HINTS.unknown;

  const openMenu = () => {
    const r = kebabRef.current?.getBoundingClientRect();
    setMenu(r === undefined ? { x: 0, y: 0 } : { x: r.right, y: r.bottom });
  };

  const startEdit = () => {
    setToken('');
    setError(null);
    setEditing(true);
  };

  const cancelEdit = () => {
    setEditing(false);
    setToken('');
    setError(null);
    kebabRef.current?.focus();
  };

  const submit = () => {
    if (submitting || token.trim() === '') return;
    setSubmitting(true);
    setError(null);
    void ipc.forgeSetTokenForHost(host, kind, token).then(
      () => {
        setSubmitting(false);
        setEditing(false);
        setToken('');
        onChanged();
      },
      (e: unknown) => {
        setSubmitting(false);
        setError(errorMessage(e));
      },
    );
  };

  const changeLabel = connected ? 'Change token' : 'Add token';
  const items: ContextMenuItem[] = [
    { label: changeLabel, onSelect: startEdit },
    { label: 'Remove account', tone: 'danger', onSelect: onRequestRemove },
  ];

  return (
    <div className="settings-account" role="group" aria-labelledby={titleId}>
      <div className="settings-account-head">
        <ForgeAvatar avatarUrl={avatarUrl} login={login} />
        <span className="settings-account-login" id={titleId} title={title}>
          {title}
        </span>
        <ForgeProviderBadge kind={kind} />
        <span
          className={`settings-account-state ${connected ? 'is-connected' : 'is-disconnected'}`}
        >
          <span className="settings-account-dot" aria-hidden="true">
            {connected ? '●' : '○'}
          </span>
          {connected ? 'Connected' : 'Token missing'}
        </span>
        <button
          ref={kebabRef}
          type="button"
          className="btn-icon settings-account-kebab"
          aria-label={`Actions for ${title}`}
          aria-haspopup="menu"
          aria-expanded={menu !== null}
          onClick={openMenu}
        >
          {'⋯'}
        </button>
      </div>

      {login !== null && (
        <p className="settings-account-host" title={host}>
          {host}
        </p>
      )}

      {!connected && (
        <p className="settings-row-help">
          {
            'The token for this host is no longer in your keychain. Add a new one or remove this entry.'
          }
        </p>
      )}

      {editing && (
        <div className="settings-account-form">
          <label className="settings-config-field-label" htmlFor={`account-token-${host}`}>
            Personal access token
          </label>
          <input
            ref={tokenRef}
            id={`account-token-${host}`}
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
              type="button"
              className="btn-primary settings-toggle-btn"
              disabled={submitting || token.trim() === ''}
              onClick={submit}
            >
              {connected
                ? submitting
                  ? 'Replacing…'
                  : 'Replace token'
                : submitting
                  ? 'Adding…'
                  : 'Add token'}
            </button>
            <button
              type="button"
              className="btn-secondary settings-toggle-btn"
              disabled={submitting}
              onClick={cancelEdit}
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {menu !== null && (
        <ContextMenu x={menu.x} y={menu.y} items={items} onClose={() => setMenu(null)} />
      )}
    </div>
  );
}
