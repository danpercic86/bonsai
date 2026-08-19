// P69d: the per-profile card, extracted verbatim from SettingsProfilesSection so
// that section stays a lean container (state + IPC) under the file-size limit.
// Purely presentational: every mutation is lifted to the parent, which owns the
// profiles array, the Apply IPC and the 2.5 s "Applied" flash.
import type { IdentityProfile } from '../../ipc';

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
  /** The last Apply error for this profile, if any. */
  error?: string;
  onChange(patch: Partial<IdentityProfile>): void;
  onApply(): void;
  onDelete(): void;
}

export function IdentityProfileCard({
  profile: p,
  isActive,
  applying,
  applied,
  noRepo,
  error,
  onChange,
  onApply,
  onDelete,
}: IdentityProfileCardProps) {
  const labelHint = p.label.trim() === '' ? 'Name this profile' : null;
  const emailWarn =
    p.userEmail.trim() !== '' && !p.userEmail.includes('@')
      ? 'This does not look like an email address (missing @).'
      : null;

  return (
    <div className="settings-config-group settings-profile">
      <div className="settings-profile-head">
        <span className="settings-config-subtitle">{p.label.trim() || 'Untitled profile'}</span>
        {isActive && <span className="settings-profile-badge">Active on this repo</span>}
      </div>

      <div className="settings-control">
        <label className="settings-control-label" htmlFor={`profile-label-${p.id}`}>
          Label
        </label>
        <input
          id={`profile-label-${p.id}`}
          className="settings-number settings-config-field"
          type="text"
          value={p.label}
          placeholder="Work"
          onChange={(e) => onChange({ label: e.target.value })}
        />
        {labelHint !== null && <p className="settings-config-hint">{labelHint}</p>}
      </div>

      <div className="settings-control">
        <label className="settings-control-label" htmlFor={`profile-name-${p.id}`}>
          user.name
        </label>
        <input
          id={`profile-name-${p.id}`}
          className="settings-number settings-config-field"
          type="text"
          value={p.userName}
          placeholder="Ada Lovelace"
          onChange={(e) => onChange({ userName: e.target.value })}
        />
      </div>

      <div className="settings-control">
        <label className="settings-control-label" htmlFor={`profile-email-${p.id}`}>
          user.email
        </label>
        <input
          id={`profile-email-${p.id}`}
          className="settings-number settings-config-field"
          type="text"
          value={p.userEmail}
          placeholder="ada@example.com"
          onChange={(e) => onChange({ userEmail: e.target.value })}
        />
        {emailWarn !== null && (
          <p className="settings-config-hint settings-config-warn">{emailWarn}</p>
        )}
      </div>

      <div className="settings-control">
        <label className="settings-control-label" htmlFor={`profile-key-${p.id}`}>
          signing key
        </label>
        <input
          id={`profile-key-${p.id}`}
          className="settings-number settings-config-field"
          type="text"
          value={p.signingKey ?? ''}
          placeholder="(optional)"
          onChange={(e) => onChange({ signingKey: e.target.value === '' ? null : e.target.value })}
        />
        <p className="settings-config-hint">
          If left empty, an existing repo signing key is kept untouched.
        </p>
      </div>

      <div className="settings-profile-actions">
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          disabled={noRepo || applying}
          onClick={onApply}
        >
          {applying ? 'Applying…' : 'Apply to current repo'}
        </button>
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          disabled={applying}
          onClick={onDelete}
        >
          Delete
        </button>
        {applied && <span className="settings-profile-applied">Applied</span>}
      </div>

      {noRepo && <p className="settings-config-hint">Open a repository to apply a profile.</p>}
      {error !== undefined && <p className="settings-config-error">{error}</p>}
    </div>
  );
}
