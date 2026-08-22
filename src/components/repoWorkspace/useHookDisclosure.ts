import { useCallback, useRef, useState } from 'react';
import { ipc } from '../../ipc';

/**
 * First-time per-repo git-hook execution disclosure.
 *
 * `bonsai.runHooks` defaults true, so committing / merging / pushing in a
 * pre-existing repo silently runs whatever lives in `.git/hooks`. This hook owns
 * a block-until-acknowledged disclosure that fires the FIRST time Bonsai is about
 * to run any hook in a repo, wired into the single {@link useHookGate} choke
 * point so all four hook-bearing ops get it with zero per-call-site change.
 *
 * Once per repo: a durable per-repo ack (backend) suppresses it across restarts;
 * an in-memory session cache suppresses the second (and later) op this session —
 * which is what makes commit&push a SINGLE prompt (push's gate sees the cache the
 * commit gate set).
 */
export interface HookDisclosure {
  /** Drives the ConfirmDialog open state. */
  pendingHookDisclosure: boolean;
  /** Gate entry point: resolves `true` ⇒ proceed, `false` ⇒ the caller cancels
   *  (throws the hook-cancel sentinel). `skipHooks` short-circuits to `true`
   *  (no hook runs ⇒ nothing to disclose). */
  ensureHooksDisclosed(skipHooks: boolean): Promise<boolean>;
  /** "Run hooks": persist the ack, cache it, and resolve the pending gate `true`. */
  onHookDiscloseConfirm(): void;
  /** Cancel / Esc: resolve the pending gate `false` (the op is canceled). */
  onHookDiscloseCancel(): void;
}

export function useHookDisclosure(repoId: string): HookDisclosure {
  const [pendingHookDisclosure, setPendingHookDisclosure] = useState(false);
  // In-memory, per-hook-instance (one per open tab): once we've cleared the
  // disclosure this session, never ask the backend again.
  const disclosedThisSession = useRef(false);
  const gateRef = useRef<{ resolve: (proceed: boolean) => void } | null>(null);

  const ensureHooksDisclosed = useCallback(
    async (skipHooks: boolean): Promise<boolean> => {
      if (skipHooks) return true; // skip-hooks runs no hook ⇒ nothing to disclose
      if (disclosedThisSession.current) return true;
      const d = await ipc.getRepoHooksDisclosure(repoId);
      if (!d.hasHooks || d.acknowledged) {
        disclosedThisSession.current = true;
        return true;
      }
      return new Promise<boolean>((resolve) => {
        gateRef.current = { resolve };
        setPendingHookDisclosure(true);
      });
    },
    [repoId],
  );

  const onHookDiscloseConfirm = useCallback(() => {
    const gate = gateRef.current;
    gateRef.current = null;
    setPendingHookDisclosure(false);
    if (gate === null) return;
    void (async () => {
      try {
        await ipc.ackRepoHooks(repoId);
      } catch {
        // Best-effort persistence: the user confirmed, so proceed regardless —
        // a failed settings write only means the disclosure may reappear next
        // session, never a blocked commit.
      }
      disclosedThisSession.current = true;
      gate.resolve(true);
    })();
  }, [repoId]);

  const onHookDiscloseCancel = useCallback(() => {
    const gate = gateRef.current;
    gateRef.current = null;
    setPendingHookDisclosure(false);
    gate?.resolve(false);
  }, []);

  return {
    pendingHookDisclosure,
    ensureHooksDisclosed,
    onHookDiscloseConfirm,
    onHookDiscloseCancel,
  };
}
