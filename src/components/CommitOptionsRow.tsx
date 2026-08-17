import type { SigningStatus } from '../ipc';

export interface CommitOptionsRowProps {
  /** P58c: render the Sign checkbox at all (false in merge mode / unknown status). */
  showSign: boolean;
  signChecked: boolean;
  onChangeSign(next: boolean): void;
  signingStatus: SigningStatus | null | undefined;
  /** 'SSH' | 'GPG' — precomputed by CommitBox. */
  signFormatLabel: string;
  onOpenIdentitySettings?: () => void;
  skipHooks: boolean;
  onChangeSkipHooks(next: boolean): void;
  /** `submitting !== null || blocked` — both checkboxes. */
  disabled: boolean;
}

/** P67 §5.2: the P58c "Sign commit" and P59a "Skip hooks" toggles merged into ONE
 *  wrapping flex row (was two stacked rows, ~46 px → ~22 px). Sign is
 *  `showSign`-gated, Skip hooks is unconditional (git runs the commit hooks on
 *  plain commits, amends and merge commits alike). Both hints are wrapping flex
 *  items rendered only while their own box is checked. Class names and visible
 *  strings are preserved verbatim from CommitBox so the existing role+name test
 *  queries keep passing. */
export function CommitOptionsRow({
  showSign,
  signChecked,
  onChangeSign,
  signingStatus,
  signFormatLabel,
  onOpenIdentitySettings,
  skipHooks,
  onChangeSkipHooks,
  disabled,
}: CommitOptionsRowProps) {
  return (
    <div className="commit-options-row">
      {showSign && (
        <label className="commit-sign-toggle">
          <input
            type="checkbox"
            checked={signChecked}
            disabled={disabled}
            onChange={(e) => onChangeSign(e.target.checked)}
          />
          <span>Sign commit</span>
        </label>
      )}
      <label className="commit-skip-toggle">
        <input
          type="checkbox"
          checked={skipHooks}
          disabled={disabled}
          onChange={(e) => onChangeSkipHooks(e.target.checked)}
        />
        <span>Skip hooks</span>
      </label>
      {showSign &&
        signChecked &&
        (signingStatus?.hasKey ? (
          <span className="commit-sign-hint">Commits will be signed ({signFormatLabel})</span>
        ) : (
          <span className="commit-sign-warn" role="note">
            No signing key set — set user.signingkey
            {onOpenIdentitySettings !== undefined && (
              <button
                type="button"
                className="commit-sign-fix"
                onClick={() => onOpenIdentitySettings()}
              >
                Set key…
              </button>
            )}
          </span>
        ))}
      {skipHooks && (
        <span className="commit-skip-hint" role="note">
          Git hooks (pre-commit, commit-msg) won’t run for this commit
        </span>
      )}
    </div>
  );
}
