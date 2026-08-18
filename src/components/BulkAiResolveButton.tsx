/**
 * P68f — the ONE "Resolve all with AI" / "Cancel all" button, rendered by BOTH entry
 * points (OQ4, settled as "keep both"):
 *
 *   * the conflicts-section header in `StatusConflictsSection.tsx` — the discoverable
 *     place, right above the per-file rows;
 *   * the merge banner's actions row in `OpBanner.tsx` — where the user actually looks
 *     during a merge ("Merging <branch> · N conflict(s) remaining · Commit merge ·
 *     Abort").
 *
 * One component rather than two copies of the markup: the enable/disable conditions
 * and the copy are part of the contract (§9 P68f), and two hand-written buttons would
 * drift the moment one of them is touched. All of the state lives in
 * `useBulkAiResolve`; this file owns nothing but the two visual variants.
 */
import type { BulkAiControl } from './repoWorkspace/useBulkAiResolve';

/** `section` = the status-panel section-header idiom (`.section-action`);
 *  `banner` = the op-banner actions-row idiom (`.op-banner-btn`). */
export type BulkAiVariant = 'section' | 'banner';

function classFor(variant: BulkAiVariant, active: boolean): string {
  if (variant === 'section') {
    return 'section-action section-action-ai section-action-ai-bulk';
  }
  // A live run's stop control is the destructive-looking one in the banner, matching
  // Abort beside it; starting a run is secondary next to "Commit merge" (primary).
  return active ? 'btn-danger op-banner-btn' : 'btn-secondary op-banner-btn';
}

export function BulkAiResolveButton({
  control,
  variant,
  busy = false,
}: {
  control: BulkAiControl;
  variant: BulkAiVariant;
  /** The host section's own "a mutation is in flight" state. It blocks STARTING a run
   *  (same as every other button in that row) but never blocks CANCELLING one — a
   *  refresh must not be able to trap a live run. */
  busy?: boolean;
}) {
  // §9 P68f: offered ONLY with ≥2 AI-eligible conflicts (or while the run it started
  // is still live). A single eligible conflict already has its row button.
  if (!control.shown) return null;
  return (
    <button
      type="button"
      className={classFor(variant, control.active)}
      data-state={control.active ? 'cancel' : 'idle'}
      disabled={control.disabled || (!control.active && busy)}
      title={control.title}
      aria-label={control.ariaLabel}
      onClick={control.onClick}
    >
      {control.label}
    </button>
  );
}
