/**
 * P80 §2c — the single conditional note line below the commit toolbar. Extracted
 * from `CommitBox.tsx` (file-size discipline). Behaviour preserved verbatim.
 * Priority: amend-pushed > sign no-key > sign will-sign > skip hooks.
 */
import type { SigningStatus } from '../ipc';

export interface CommitNoteProps {
  showAmendPushWarning: boolean;
  showSign: boolean;
  signChecked: boolean;
  signingStatus?: SigningStatus | null;
  signFormatLabel: string;
  skipHooks: boolean;
  onOpenIdentitySettings?: () => void;
}

export function CommitNote({
  showAmendPushWarning,
  showSign,
  signChecked,
  signingStatus,
  signFormatLabel,
  skipHooks,
  onOpenIdentitySettings,
}: CommitNoteProps) {
  if (showAmendPushWarning) {
    return (
      <div className="commit-note" role="note">
        <span className="commit-note-glyph" aria-hidden="true">
          ⚠
        </span>
        <span>This commit is already pushed — amending rewrites published history.</span>
      </div>
    );
  }
  if (showSign && signChecked && !(signingStatus?.hasKey ?? false)) {
    return (
      <div className="commit-note" role="note">
        <span className="commit-note-glyph" aria-hidden="true">
          ⚠
        </span>
        <span>
          No signing key set — commits won’t be signed.
          {onOpenIdentitySettings !== undefined && (
            <button
              type="button"
              className="commit-sign-fix"
              title="Set user.signingkey in Git config"
              onClick={() => onOpenIdentitySettings()}
            >
              Set key…
            </button>
          )}
        </span>
      </div>
    );
  }
  if (showSign && signChecked && (signingStatus?.hasKey ?? false)) {
    return (
      <div className="commit-note" role="note">
        <span className="commit-note-glyph commit-note-glyph-ok" aria-hidden="true">
          ✓
        </span>
        <span>Commits will be signed ({signFormatLabel}).</span>
      </div>
    );
  }
  if (skipHooks) {
    return (
      <div className="commit-note" role="note">
        <span className="commit-note-glyph" aria-hidden="true">
          ⚠
        </span>
        <span>Git hooks won’t run for this commit.</span>
      </div>
    );
  }
  return null;
}
