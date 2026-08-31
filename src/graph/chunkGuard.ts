/** Audit 2026-08-18 §3.8: contain throws from a streamed-chunk apply function.
 *
 *  The stream assembler THROWS on a non-contiguous `startRow` — a correct
 *  invariant guard — but the chunk callback runs inside `Channel.onmessage`
 *  (src/ipc/tauri.ts). In real Tauri a throw there is an unhandled
 *  event-handler exception: it never reaches the `await ipc.streamGraph(...)`
 *  catch, so `setGraphError` never fires, every later batch re-throws, and the
 *  stream silently freezes while the loading flag clears as if it succeeded.
 *  (The mock calls the callback synchronously, which is why tests could not
 *  see the divergence.)
 *
 *  This wrapper catches the FIRST throw, reports it once, and poisons the
 *  handler so every subsequent chunk is dropped. Pure + unit-testable — no
 *  React, no IPC. */

export interface GuardedChunkHandler<C> {
  /** True once `apply` has thrown; all later chunks are dropped. */
  readonly poisoned: boolean;
  handle(chunk: C): void;
}

export function guardChunks<C>(
  apply: (chunk: C) => void,
  onError: (e: unknown) => void,
): GuardedChunkHandler<C> {
  let poisoned = false;
  return {
    get poisoned() {
      return poisoned;
    },
    handle(chunk: C): void {
      if (poisoned) return;
      try {
        apply(chunk);
      } catch (e) {
        poisoned = true;
        onError(e);
      }
    },
  };
}
