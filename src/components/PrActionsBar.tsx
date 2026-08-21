import type { ForgeKind, MergeMethod, PrState } from '../ipc';

// P83 — presentational PR actions footer bar (UI contract §1). Renders while the
// PR is open: a primary Merge (opens the merge dialog) + a quieter danger-tinted
// Close/Decline/Abandon (opens a ConfirmDialog). No IPC; all state is props.

/** Per-forge label for the close action's button (UI contract §1). */
export function closeActionLabel(kind: ForgeKind): string {
  switch (kind) {
    case 'bitbucket':
      return 'Decline';
    case 'azureDevOps':
      return 'Abandon';
    default:
      return 'Close';
  }
}

/** Past-tense close verb for success/error toasts ("Closed"/"Declined"/…). */
export function closeActionPast(kind: ForgeKind): string {
  switch (kind) {
    case 'bitbucket':
      return 'Declined';
    case 'azureDevOps':
      return 'Abandoned';
    default:
      return 'Closed';
  }
}

/** Busy gerund for the close confirm button ("Closing…"/"Declining…"/…). */
export function closeActionGerund(kind: ForgeKind): string {
  switch (kind) {
    case 'bitbucket':
      return 'Declining';
    case 'azureDevOps':
      return 'Abandoning';
    default:
      return 'Closing';
  }
}

export interface PrActionsBarProps {
  state: PrState;
  kind: ForgeKind;
  /** null ⇒ still computing; false ⇒ conflicts; true ⇒ mergeable. */
  mergeable: boolean | null;
  supportedMethods: MergeMethod[];
  busy: boolean;
  onMerge(): void;
  onClose(): void;
}

export function PrActionsBar({
  state,
  kind,
  mergeable,
  supportedMethods,
  busy,
  onMerge,
  onClose,
}: PrActionsBarProps) {
  // Only an OPEN PR has actions.
  if (state !== 'open') return null;

  // Defensive: an unsupported forge exposes no methods ⇒ hide Merge entirely.
  const mergeSupported = supportedMethods.length > 0;
  const mergeDisabled = busy || mergeable !== true;
  const mergeTitle =
    mergeable === null
      ? 'Bonsai is still checking whether this can be merged.'
      : mergeable === false
        ? "This pull request has conflicts and can't be merged."
        : undefined;

  return (
    <div className="pr-actions-bar">
      <button
        type="button"
        className="btn-secondary btn-secondary-danger"
        disabled={busy}
        onClick={onClose}
      >
        {closeActionLabel(kind)}
      </button>
      {mergeSupported && (
        <button
          type="button"
          className="btn-primary"
          disabled={mergeDisabled}
          title={mergeTitle}
          onClick={onMerge}
        >
          Merge…
        </button>
      )}
    </div>
  );
}
