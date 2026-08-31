import { useCallback, useRef, useState } from 'react';
import { isAppError } from '../../utils/errors';
import { COMMIT_HOOK_CANCELED } from '../commitPushSignal';

/**
 * P59a: parks a commit attempt behind the HookOutputDialog when a git hook
 * BLOCKS it (`AppError.kind === 'hookRejected'`), then either re-runs it with
 * `skipHooks:true` ("Commit anyway") or cancels (leaving the message intact).
 * Non-hook errors pass straight through to the caller's own error path.
 *
 * Shared by every commit path (commit / amend / merge / commit&push) via the
 * injected {@link HookGate.runWithHookGate}, so one dialog + one retry closure
 * serves them all.
 */
export interface HookGate {
  /** The `hookRejected` message to show, or null when the dialog is closed. */
  pendingHook: string | null;
  /** True while a skip-hooks retry is in flight (drives the dialog busy state). */
  hookRetrying: boolean;
  /** Wrap a commit attempt. `attempt(skipHooks)` performs the mutation AND its
   *  success side-effects; running it through this gate turns a `hookRejected`
   *  into the dialog instead of surfacing raw. The returned promise resolves
   *  when the commit ultimately succeeds (directly, or after the retry), rejects
   *  with the ORIGINAL error for non-hook failures, or rejects with
   *  {@link COMMIT_HOOK_CANCELED} when the user dismisses the dialog. */
  runWithHookGate(
    attempt: (skipHooks: boolean) => Promise<void>,
    skipHooks: boolean,
  ): Promise<void>;
  /** "Commit anyway (skip hooks)": re-run the parked attempt with skipHooks:true. */
  onHookSkipRetry(): void;
  /** Dismiss the dialog; nothing was committed. */
  onHookCancel(): void;
}

export function useHookGate(
  /** First-time per-repo hook-execution disclosure, run BEFORE `attempt` so every
   *  hook-bearing op discloses once with zero per-call-site change. Resolves
   *  `false` when the user declines ⇒ the op cancels silently. */
  ensureHooksDisclosed: (skipHooks: boolean) => Promise<boolean>,
): HookGate {
  const [pendingHook, setPendingHook] = useState<string | null>(null);
  const [hookRetrying, setHookRetrying] = useState(false);
  const gateRef = useRef<{
    resolve: () => void;
    reject: (e: unknown) => void;
    retry: () => Promise<void>;
  } | null>(null);

  const runWithHookGate = useCallback(
    async (attempt: (skipHooks: boolean) => Promise<void>, skipHooks: boolean): Promise<void> => {
      // Disclose BEFORE any hook could run. A decline cancels the op via the
      // existing sentinel (silent cancel, no error banner). skipHooks bypasses
      // this (no hook runs ⇒ nothing to disclose).
      if (!(await ensureHooksDisclosed(skipHooks))) throw COMMIT_HOOK_CANCELED;
      try {
        await attempt(skipHooks);
      } catch (e) {
        if (isAppError(e) && e.kind === 'hookRejected') {
          // Park: defer resolution until the dialog is answered. The retry
          // re-runs the SAME attempt with skipHooks:true (≡ --no-verify),
          // preserving the message + amend/merge mode captured in the closure.
          await new Promise<void>((resolve, reject) => {
            gateRef.current = { resolve, reject, retry: () => attempt(true) };
            setPendingHook(e.message);
          });
          return;
        }
        throw e;
      }
    },
    [ensureHooksDisclosed],
  );

  const onHookSkipRetry = useCallback(() => {
    const gate = gateRef.current;
    if (gate === null) return;
    // Detach the gate BEFORE launching the async retry so a concurrent Cancel/Esc
    // no-ops (onHookCancel reads a now-null gateRef): the retry's own resolve/reject
    // settles the parked promise. Prevents a cancel-during-retry double-settle where
    // the fast skipHooks commit lands but the box treats it as canceled (stale message).
    gateRef.current = null;
    setHookRetrying(true);
    void (async () => {
      try {
        await gate.retry();
        gate.resolve();
      } catch (e) {
        // The retry itself failed with some non-hook error — surface it via the
        // original caller's error path.
        gate.reject(e);
      } finally {
        setPendingHook(null);
        setHookRetrying(false);
      }
    })();
  }, []);

  const onHookCancel = useCallback(() => {
    const gate = gateRef.current;
    gateRef.current = null;
    setPendingHook(null);
    // Nothing committed: reject with the sentinel so the commit box keeps the
    // typed message and shows no error banner.
    gate?.reject(COMMIT_HOOK_CANCELED);
  }, []);

  return { pendingHook, hookRetrying, runWithHookGate, onHookSkipRetry, onHookCancel };
}
