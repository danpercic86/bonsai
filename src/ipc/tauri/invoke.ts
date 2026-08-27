/**
 * P91 §2.2 — the single Tauri transport choke point.
 *
 * Every `src/ipc/tauri/*.ts` module imports `invoke` from HERE, never from
 * `@tauri-apps/api/core` (a test in `invoke.test.ts` enforces that, because one
 * missed file is a silently untraced command). This is the only layer that sees
 * the actual invoke PAYLOAD MAP, which is where §2.2 requires the reserved
 * `__trace`/`__span` keys to sit: Tauri deserializes command parameters
 * key-by-key from that map and ignores unknown keys, so no command signature
 * changes — and increment 3's dispatch shim reads them off `invoke.message`.
 *
 * Deliberately NOT done in `src/obs/ipcProxy.ts`: the `IpcApi` methods are
 * fixed-arity and positional, and each builds its own payload internally, so an
 * injection one layer up either lands nested inside a domain object (polluting
 * persisted state) or is dropped as an unread extra argument. The proxy keeps
 * timing, pairing, `argsHash`/`argsShape` and record emission; transport keeps
 * transport.
 */
import { invoke as tauriInvoke, type InvokeArgs, type InvokeOptions } from '@tauri-apps/api/core';
import { obsCaptureIpc, obsEnabled } from '../../obs/enabled';
import { currentIpcSpan, currentTrace, SPAN_ARG_KEY, TRACE_ARG_KEY } from '../../obs/trace';

/**
 * `invoke` with trace stamping. When Dev mode is off this is one boolean read
 * and a straight delegation — no copy, no allocation (§11).
 */
export function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  return tauriInvoke<T>(cmd, stamp(args), options);
}

function stamp(args?: InvokeArgs): InvokeArgs | undefined {
  if (!obsEnabled() || !obsCaptureIpc()) return args;
  const trace = currentTrace()?.trace;
  // No ambient trace ⇒ nothing honest to stamp. §2.5: never guess a trace.
  if (trace === undefined) return args;
  const span = currentIpcSpan();
  const meta = { [TRACE_ARG_KEY]: trace, ...(span ? { [SPAN_ARG_KEY]: span } : {}) };
  if (args === undefined) return meta;
  // A binary payload (ArrayBuffer / typed array) has no key space to inject
  // into; it is forwarded untouched rather than corrupted.
  if (!isPlainRecord(args)) return args;
  // A COPY: these objects are frequently React state, and a stray key on the
  // original could corrupt it.
  return { ...args, ...meta };
}

function isPlainRecord(args: InvokeArgs): args is Record<string, unknown> {
  if (typeof args !== 'object' || args === null) return false;
  if (args instanceof ArrayBuffer || ArrayBuffer.isView(args)) return false;
  return !Array.isArray(args);
}
