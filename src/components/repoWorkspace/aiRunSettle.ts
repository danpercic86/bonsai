/**
 * P68f — the settle step of `useAiRuns`, extracted per the ~500-line rule.
 *
 * Turn the resolved batch into per-file state and route it by autonomy.
 *
 * THE SAFETY GATE (blocking requirement from the P68b review): a `proposedText`
 * here is a REVIEWABLE proposal, not a verified-clean merge — the single-path
 * stream returns the model's body verbatim (P13 parity), and bulk's markerful
 * check lives on the other side of the IPC boundary. So `hasUnresolvedMarkers`
 * is applied HERE, before anything can be staged, exactly as the deleted
 * `handleAiResolveConflict` did. Nothing markerful is ever presented as clean.
 */
import { settleBatch, type AiRunState } from './aiRunState';
import type { AiResolveBatch } from '../../ipc';
import type { AiRunsDeps } from './useAiRuns';

/** The hook internals the settle step reads/writes; every field is a stable ref
 *  or a stable callback, so `useAiRuns` builds this object once. */
export interface AiSettleCtx {
  runsRef: { readonly current: Record<string, AiRunState> };
  depsRef: { readonly current: AiRunsDeps };
  /** Audit §3.10: false once the workspace unmounted — settle goes log-only. */
  mounted: { readonly current: boolean };
  /** FOLD-IN 1: which diff slot the user was looking at when each run started. */
  slotAtStart: { readonly current: Map<string, string | null> };
  /** Immutable per-entry patch. Does not commit — settle decides when. */
  patch(key: string, next: Partial<AiRunState>): void;
  flush(): void;
}

export async function settleRun(
  ctx: AiSettleCtx,
  key: string,
  batch: AiResolveBatch,
): Promise<void> {
  const entry = ctx.runsRef.current[key];
  if (entry === undefined) return;
  const d = ctx.depsRef.current;
  const autonomy = d.aiConflictAutonomy;
  // `settleBatch` owns the arithmetic AND the markerful safety gate (see
  // `aiRunState.ts`): a body that still carries conflict markers is demoted to
  // `failed` for its row and can never reach `applyResolution`.
  const out = settleBatch(entry.paths, batch, autonomy);

  ctx.patch(key, {
    files: out.files,
    proposal: out.proposal,
    costUsd: batch.costUsd ?? entry.costUsd,
    status: out.status,
    error: out.error,
    endedAt: Date.now(),
  });
  ctx.flush();

  // Audit §3.10: the workspace can unmount (tab close) while the batch is in
  // flight — never stage into, toast over, or open a pane for a repo whose
  // tab is gone. The terminal state was recorded in the ref above; stop here.
  if (!ctx.mounted.current) return;

  for (const f of out.markerful) {
    if (f.error !== null) d.pushToast('error', f.error);
  }

  if (autonomy === 'autoResolve' && out.stageable.length > 0) {
    // P68f: stage every marker-free file, then refresh ONCE. Anything markerful was
    // already demoted to `failed` by `settleBatch` above, so it cannot get here —
    // that is the safety gate, and it runs BEFORE `stageable` is computed.
    const many = out.stageable.length > 1;
    let staged = 0;
    for (const f of out.stageable) {
      try {
        await d.applyResolution(
          f.path,
          f.proposal ?? '',
          // Bulk: stay silent per file and summarise once, instead of N toasts.
          many ? null : `Resolved ${f.path} with AI — review the staged result`,
          true,
        );
        staged += 1;
      } catch {
        // applyResolution already toasted; keep going for the other files.
      }
    }
    await d.refreshAll();
    if (many && staged > 0) {
      d.pushToast(
        'success',
        `Resolved ${staged} file${staged === 1 ? '' : 's'} with AI — review the staged results`,
      );
    }
  }

  // ONE center pane opens: the marker fallback under autoResolve, otherwise the
  // first ready proposal. A bulk run with several ready files opens nothing and
  // points at the activity dock instead.
  const toOpen = autonomy === 'autoResolve' ? out.markerful[0] : out.stageable[0];
  const text = toOpen?.proposal ?? null;
  if (toOpen === undefined || text === null) return;
  if (autonomy === 'proposeReview') {
    // FOLD-IN 1: the pane is only taken when the user is still looking at what
    // they were looking at when the run started.
    const stayed = ctx.slotAtStart.current.get(key) === (d.diffSlotKey?.() ?? null);
    if (out.stageable.length > 1) {
      d.pushToast(
        'success',
        `AI proposals ready for ${out.stageable.length} files — review them from the AI activity dock`,
      );
      return;
    }
    d.pushToast(
      'success',
      stayed
        ? `AI proposal ready for ${toOpen.path} — opened for review`
        : `AI proposal ready for ${toOpen.path} — review it from the AI activity dock`,
    );
    if (!stayed) return;
  }
  // The markerful fallback under `autoResolve` opens UNCONDITIONALLY: its row
  // shows `⚠` (retry), so this open is the only path to that body, and the whole
  // point is that the user must see what the model actually produced. Under BULK
  // (P68f) several files can be markerful at once; only `markerful[0]` is opened,
  // so N finishing files still take the centre pane AT MOST ONCE — the rest are
  // reachable from their queue rows and each already got its own error toast.
  //
  // P68e M1: record that the pane really was taken, BEFORE the await, so the dock
  // renders `Proposal is open in the center pane.` only in this branch — the
  // suppressed branch gets the sentence that points at the dock instead.
  ctx.patch(key, { openedInPane: true });
  ctx.flush();
  await d.openAiProposal(toOpen.path, text);
}
