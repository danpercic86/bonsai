// P69i — the per-profile card in Settings → Identities.
//
// Amendment A (AM-1) governs its shape: the card itself is NOT a catalog row —
// it is a `role="group"` named by its own title, carrying `data-profile-id` — and
// each of the six repeated rows inside stamps BOTH `data-setting-id` and
// `data-profile-id`. AM-6 then requires every repeated control's accessible name
// to be a CONSTANT equal to the catalog label: per-profile disambiguation comes
// from the enclosing group's name, never from the control's.
//
// Purely presentational: every mutation is lifted to the parent, which owns the
// profiles array, the Apply IPC and the 2.5 s "Applied" flash.

import { useEffect, useRef, useState } from 'react';

import type { IdentityProfile } from '../../ipc';
import { settingsRowHelpId } from './settingsCatalog';
import { SettingsRow } from './SettingsRow';
import { useSettingsRowVisible } from './SettingsSearchContext';

const LABEL = 'identities.profile-label';
const NAME = 'identities.profile-name';
const EMAIL = 'identities.profile-email';
const KEY = 'identities.profile-signing-key';

export interface IdentityProfileCardProps {
  profile: IdentityProfile;
  /** This profile matches the repo's EFFECTIVE identity (P69 §5.1 / UI D6). */
  isActive: boolean;
  /** An Apply for THIS profile is in flight. */
  applying: boolean;
  /** This profile was applied in the last 2.5 s. */
  applied: boolean;
  /** No repo open → Apply is disabled and the note explains why. */
  noRepo: boolean;
  /** P69i: take focus on mount — this card is the draft the header menu just
   *  saved, and Label is the one field it could not fill in (UI §4.3 item 2). */
  autoFocusLabel?: boolean;
  /** The last Apply error for this profile, if any. */
  error?: string;
  onChange(patch: Partial<IdentityProfile>): void;
  onApply(): void;
  onDelete(): void;
}

/**
 * A stamped cell for a catalog row whose control is a bare button.
 *
 * The two action buttons sit side by side in one action area (UI §4.6), not on
 * two full-width `SettingsRow` grids — but the coverage guard still needs each to
 * be a stamped row with a `[data-setting-control]` descendant, so the stamp is
 * applied here instead of borrowing a row layout that does not fit.
 *
 * RESOLVED IN P69k: there is no help slot here, so the catalog `help` of
 * `identities.apply` / `identities.delete` was never rendered — search would have
 * matched text that is not on screen. Both `help` values were DROPPED and their
 * vocabulary folded into `keywords` instead (the `catalog/ai.ts` precedent),
 * rather than growing a help line under two side-by-side buttons that already
 * say what they do.
 *
 * It is also a stamped row, so it self-filters inside a search result block for
 * the same reason `SettingsRow` does.
 */
function ProfileActionCell({
  id,
  profileId,
  children,
}: {
  id: string;
  profileId: string;
  children: React.ReactNode;
}) {
  const visible = useSettingsRowVisible(id);
  if (!visible) return null;
  return (
    <div className="settings-profile-action" data-setting-id={id} data-profile-id={profileId}>
      <span data-setting-control="">{children}</span>
    </div>
  );
}

export function IdentityProfileCard({
  profile: p,
  isActive,
  applying,
  applied,
  noRepo,
  autoFocusLabel,
  error,
  onChange,
  onApply,
  onDelete,
}: IdentityProfileCardProps) {
  // UI §4.6: delete is an inline TWO-STEP, not a modal over a modal. Esc cancels;
  // so does moving focus out of the card, so a half-armed delete cannot be left
  // lying around on a card the user has walked away from.
  const [confirming, setConfirming] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const labelRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (autoFocusLabel === true) labelRef.current?.focus();
  }, [autoFocusLabel]);
  useEffect(() => {
    if (confirming) cancelRef.current?.focus();
  }, [confirming]);

  const titleId = `identity-card-${p.id}-title`;
  const cardTitle = p.label.trim() || 'Untitled profile';
  const emailWarn =
    p.userEmail.trim() !== '' && !p.userEmail.includes('@')
      ? 'That doesn’t look like an email address — it’s missing an “@”.'
      : null;

  return (
    <div
      className="settings-profile"
      role="group"
      aria-labelledby={titleId}
      data-profile-id={p.id}
      onBlur={(e) => {
        if (confirming && !e.currentTarget.contains(e.relatedTarget)) setConfirming(false);
      }}
      onKeyDown={(e) => {
        if (e.key === 'Escape' && confirming) {
          e.stopPropagation();
          setConfirming(false);
        }
      }}
    >
      <div className="settings-profile-head">
        <span className="settings-config-subtitle" id={titleId} title={cardTitle}>
          {cardTitle}
        </span>
        {isActive && <span className="settings-profile-badge">{'in use'}</span>}
      </div>

      <SettingsRow id={LABEL} profileId={p.id} controlId={`profile-label-${p.id}`} stacked>
        <input
          ref={labelRef}
          id={`profile-label-${p.id}`}
          className="settings-config-field"
          type="text"
          value={p.label}
          placeholder="Work"
          aria-describedby={settingsRowHelpId(LABEL, p.id)}
          onChange={(e) => onChange({ label: e.target.value })}
        />
      </SettingsRow>

      <SettingsRow id={NAME} profileId={p.id} controlId={`profile-name-${p.id}`} stacked>
        <input
          id={`profile-name-${p.id}`}
          className="settings-config-field"
          type="text"
          value={p.userName}
          placeholder="Ada Lovelace"
          aria-describedby={settingsRowHelpId(NAME, p.id)}
          onChange={(e) => onChange({ userName: e.target.value })}
        />
      </SettingsRow>

      <SettingsRow
        id={EMAIL}
        profileId={p.id}
        controlId={`profile-email-${p.id}`}
        stacked
        hint={
          emailWarn === null ? undefined : (
            <p className="settings-row-help settings-config-warn">{emailWarn}</p>
          )
        }
      >
        <input
          id={`profile-email-${p.id}`}
          className="settings-config-field"
          type="text"
          value={p.userEmail}
          placeholder="ada@example.com"
          aria-describedby={settingsRowHelpId(EMAIL, p.id)}
          onChange={(e) => onChange({ userEmail: e.target.value })}
        />
      </SettingsRow>

      <SettingsRow id={KEY} profileId={p.id} controlId={`profile-key-${p.id}`} stacked>
        <input
          id={`profile-key-${p.id}`}
          className="settings-config-field"
          type="text"
          value={p.signingKey ?? ''}
          placeholder="(optional)"
          aria-describedby={settingsRowHelpId(KEY, p.id)}
          onChange={(e) => onChange({ signingKey: e.target.value === '' ? null : e.target.value })}
        />
      </SettingsRow>

      <div className="settings-profile-actions">
        <ProfileActionCell id="identities.apply" profileId={p.id}>
          <button
            type="button"
            className="btn-secondary settings-toggle-btn"
            disabled={noRepo || applying}
            onClick={onApply}
          >
            {applying ? 'Applying…' : 'Use in this repository'}
          </button>
        </ProfileActionCell>
        <ProfileActionCell id="identities.delete" profileId={p.id}>
          {confirming ? (
            <span className="settings-profile-confirm">
              <span className="settings-profile-confirm-q">{`Delete “${cardTitle}”?`}</span>
              <button
                type="button"
                className="btn-secondary settings-toggle-btn settings-profile-danger"
                onClick={() => {
                  setConfirming(false);
                  onDelete();
                }}
              >
                {'Delete'}
              </button>
              <button
                ref={cancelRef}
                type="button"
                className="btn-secondary settings-toggle-btn"
                onClick={() => setConfirming(false)}
              >
                {'Cancel'}
              </button>
            </span>
          ) : (
            <button
              type="button"
              className="btn-secondary settings-toggle-btn settings-profile-danger"
              disabled={applying}
              onClick={() => setConfirming(true)}
            >
              {'Delete'}
            </button>
          )}
        </ProfileActionCell>
        {applied && <span className="settings-profile-applied">{'Applied'}</span>}
      </div>
      {confirming && (
        <p className="settings-row-help">
          {'This only removes the saved identity. Your repository’s Git config is not changed.'}
        </p>
      )}

      {noRepo && (
        <p className="settings-row-help">{'Open a repository to use an identity in it.'}</p>
      )}
      {error !== undefined && <p className="settings-config-error">{error}</p>}
    </div>
  );
}
