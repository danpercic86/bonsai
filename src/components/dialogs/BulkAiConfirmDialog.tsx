/**
 * P68f — the confirm gate in front of "Resolve all with AI".
 *
 * Why a confirm at all, when the AI writes nothing (D4)? Two reasons the user cares
 * about, and both are spelled out in the body rather than implied:
 *   * it SPENDS — one CLI run over N files is the most expensive thing Bonsai can
 *     start, and `ai_max_budget_usd` ships as opt-in (OQ2), so nothing stops it but
 *     the user's own Cancel;
 *   * it touches N files at once, and under the `autoResolve` autonomy the marker-free
 *     results are STAGED without a further click (OQ3) — the first place P68 stages
 *     several files from one AI call.
 *
 * The markerful half of that sentence is not decoration: `settleBatch` demotes any
 * body that still carries conflict markers to `failed` BEFORE deciding what is
 * stageable, so those files fall back to review instead of being staged. Saying so
 * here is what makes the `autoResolve` branch honest.
 *
 * `confirmVariant="primary"`: this destroys nothing (the AI cannot write, staging is
 * reversible), so the danger styling is reserved for the operations that really are
 * irreversible.
 */
import { ConfirmDialog } from '../ConfirmDialog';
import type { BulkAiConfirmState } from '../repoWorkspace/useBulkAiResolve';

/** How many paths are listed before the "+N more" line (the `DestructiveDialogs`
 *  bulk-discard idiom). */
const LIST_MAX = 10;

export function BulkAiConfirmDialog({
  open,
  paths,
  autonomy,
  onConfirm,
  onCancel,
}: BulkAiConfirmState) {
  const count = paths.length;
  return (
    <ConfirmDialog
      open={open}
      title="Resolve all conflicts with AI"
      confirmLabel="Resolve all with AI"
      confirmVariant="primary"
      busy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      <div>
        Send {count} conflicted {count === 1 ? 'file' : 'files'} to Claude in{' '}
        <strong>one AI run</strong>, so it can reason across them?
      </div>
      <ul className="confirm-name-list">
        {paths.slice(0, LIST_MAX).map((p) => (
          <li key={p} className="mono">
            {p}
          </li>
        ))}
        {count > LIST_MAX && (
          <li className="dialog-body-note">+{count - LIST_MAX} more</li>
        )}
      </ul>
      <div className="dialog-body-note">
        {autonomy === 'autoResolve'
          ? 'Marker-free results are staged automatically (Settings → AI: “Resolve automatically”). Anything that still contains conflict markers is opened for review instead — never staged.'
          : 'Nothing is written to your files: each result is a proposal you review before it is staged.'}
      </div>
      <div className="dialog-body-note">
        This runs the Claude CLI once and uses your Claude quota. You can stop it at any
        time with Cancel all.
      </div>
    </ConfirmDialog>
  );
}
