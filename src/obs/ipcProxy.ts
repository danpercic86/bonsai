/**
 * P91 §4 — the frontend choke point.
 *
 * `instrumentIpc(api)` wraps the ONE object every IPC call goes through, applied
 * in `src/ipc/index.ts` around the resolved api. Because that resolution picks
 * either the real Tauri api or the mock api, mock-IPC parity is satisfied **by
 * construction**: there is no second place a call can escape through, and no
 * future change can forget to instrument one side.
 *
 * ## Zero cost when Dev mode is off (§11, §12 row 2)
 *
 * The Proxy is always installed, but its `get` trap re-reads `obsEnabled()` on
 * every access and returns the ORIGINAL method when off. So
 * `ipc.foo === ipc.foo` holds, no wrapper is allocated, no trace is minted — and
 * toggling Dev mode still takes effect with no reload (§10), which a
 * "wrap-once-at-boot" design could not deliver.
 */
import { obsCaptureIpc, obsEnabled, obsRedaction, onObsConfigChange } from './enabled';
import { attachSink, type LogSink } from './batcher';
import { logRecord } from './log';
import {
  argsShape,
  hashArgs,
  objectShape,
  redactionReady,
} from './redact';
import { buildRawArgs } from './rawArgPolicy';
import { repoIdArg } from './repoArg';
import { bindTrace, currentTrace, newSpan, withIpcSpan } from './trace';
import type { ArgShape, IpcCallPayload, IpcOutcome, SpanId } from './types';

/**
 * §2.3 — commands excluded from instrumentation because logging them would log
 * the act of logging. This is the SECOND guard; the first is structural (the
 * batcher holds the raw api, not the proxy).
 */
export const OBS_EXCLUDED_METHODS: ReadonlySet<string> = new Set([
  'logAppend',
  'logSessionInfo',
  'logRevealDir',
  'logExportSession',
  'logsDeleteAll',
  'metricsSnapshot',
  'metricsReset',
  'debugPerfCounters',
]);

/** Per-`cmd` newest span, for the §4 `superseded` outcome. */
const latestSpan = new Map<string, SpanId>();

type AnyFn = (...args: unknown[]) => unknown;

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/**
 * Prepares the outgoing argument list.
 *
 * It does NOT inject `__trace`/`__span`: the `IpcApi` methods are positional and
 * each builds its own invoke payload internally, so injection belongs to the
 * transport layer (`src/ipc/tauri/invoke.ts`), which is the only place that sees
 * the payload map §2.2 talks about. Injecting here would either nest the keys
 * inside a domain object (and, on the mock path, persist them) or append an
 * argument no target function reads.
 *
 * Callback arguments ARE handled here: they are re-bound to the calling trace so
 * a delivery long after subscription still attributes to the trace that
 * subscribed, and each delivery emits an `event` record (§4).
 */
function prepareArgs(args: readonly unknown[], cmd: string, trace: string | undefined): unknown[] {
  if (!args.some((a) => typeof a === 'function')) return args as unknown[];
  return args.map((arg, i) =>
    typeof arg === 'function' ? wrapCallback(arg as AnyFn, cmd, i, trace) : arg,
  );
}

/** §4 — callbacks emit an `event` record on DELIVERY, under the subscribing trace. */
function wrapCallback(fn: AnyFn, cmd: string, index: number, trace: string | undefined): AnyFn {
  const bound = bindTrace(fn as (...a: never[]) => unknown) as AnyFn;
  return (...cbArgs: unknown[]) => {
    logRecord({
      kind: 'event',
      name: `${cmd}#${index}`,
      delivered: true,
      listeners: 1,
      ...(trace ? { causedBy: trace } : {}),
    });
    return bound(...cbArgs);
  };
}

function errCodeOf(err: unknown): string | undefined {
  if (isPlainObject(err) && typeof err.kind === 'string') return err.kind;
  if (err instanceof Error) return err.name;
  return undefined;
}

/**
 * A26 — the raw-mode `args` fragment, or `{}` when the allow-list yields
 * nothing. `argsOmitted` is emitted only when positive so a fully-included call
 * stays byte-identical to before.
 */
function rawArgs(cmd: string, args: readonly unknown[]): Partial<IpcCallPayload> {
  const { args: kept, omitted } = buildRawArgs(cmd, args);
  return {
    ...(kept ? { args: kept } : {}),
    ...(omitted > 0 ? { argsOmitted: omitted } : {}),
  };
}

function wrapMethod(target: object, cmd: string, fn: AnyFn): AnyFn {
  return function instrumented(this: unknown, ...args: unknown[]): unknown {
    const span = newSpan();
    const trace = currentTrace()?.trace;
    // Hashed BEFORE injection: if `__trace`/`__span` entered the digest, every
    // call would hash uniquely and `dup-ipc` (§5) could never fire.
    const argsHash = hashArgs(args) ?? '';
    const shape: ArgShape = argsShape(args);
    const raw = obsRedaction() === 'raw';
    // P117 §2.2/§2.4 — the repo this call is about, on the record BASE so it is
    // present in strict mode too (`args` is raw-only, so without this an
    // `ipc.call` carries no repo in the shipping configuration and a mutation in
    // repo A would go on suppressing findings in repo B). RAW and unredacted:
    // the detector matches it against the Rust span's raw `repoId`.
    const repo = repoIdArg(cmd, args);
    logRecord({
      kind: 'ipc.call',
      cmd,
      argsHash,
      argsShape: shape,
      span,
      ...(trace ? { trace } : {}),
      ...(repo !== undefined ? { repo } : {}),
      // §7.1 + A26: argument VALUES only ever appear in `raw` mode, and even
      // there only the allow-listed IDENTIFIER scalars of `rawArgPolicy.json`,
      // keyed by parameter NAME. Free text (commit messages, search strings) and
      // credentials are outside both modes; an unlisted command is DENY, so it
      // behaves exactly like strict — hash + shape and nothing else.
      ...(raw ? rawArgs(cmd, args) : {}),
    });

    latestSpan.set(cmd, span);
    const started = performance.now();
    const finish = (outcome: IpcOutcome, result: unknown, err?: unknown): void => {
      const superseded = outcome === 'ok' && latestSpan.get(cmd) !== span;
      logRecord({
        kind: 'ipc.result',
        cmd,
        argsHash,
        ms: Math.round((performance.now() - started) * 100) / 100,
        outcome: superseded ? 'superseded' : outcome,
        span,
        ...(trace ? { trace } : {}),
        ...(err !== undefined ? { errCode: errCodeOf(err) } : {}),
        ...(outcome === 'ok' ? { resultShape: objectShape(result) } : {}),
      });
    };

    let returned: unknown;
    try {
      // `withIpcSpan` publishes the span for the transport layer, which injects
      // `__trace`/`__span` into the invoke payload (§2.2). The wrapper calls
      // `invoke` synchronously inside this `apply`, so the ambient is exact.
      returned = withIpcSpan(span, () => fn.apply(target, prepareArgs(args, cmd, trace)));
    } catch (err) {
      // A synchronous throw is still an outcome the reviewer must see.
      finish('err', undefined, err);
      throw err;
    }
    if (
      typeof returned === 'object' &&
      returned !== null &&
      typeof (returned as PromiseLike<unknown>).then === 'function'
    ) {
      return (returned as Promise<unknown>).then(
        (value) => {
          finish('ok', value);
          return value;
        },
        (err: unknown) => {
          finish('err', undefined, err);
          throw err;
        },
      );
    }
    finish('ok', returned);
    return returned;
  };
}

/**
 * Wraps the api. `api` must be the resolved (real or mock) implementation; the
 * RAW object is also handed to the batcher, which is what keeps the sink's own
 * calls outside the instrumentation for good.
 */
export function instrumentIpc<T extends object>(api: T): T {
  attachSink(api as unknown as LogSink);
  // Memoized per method, so an enabled proxy still hands out STABLE identities
  // (React dependency arrays compare by reference).
  const wrapped = new Map<string, AnyFn>();
  // A caller may hold a method reference taken while enabled. Dropping the memo
  // on every config change means the next access re-enters the gate, so a
  // stale wrapper cannot keep hashing args after Dev mode goes off (§11).
  onObsConfigChange(() => wrapped.clear());
  return new Proxy(api, {
    get(target, prop, receiver) {
      const value = Reflect.get(target, prop, receiver);
      if (typeof prop !== 'string' || typeof value !== 'function') return value;
      if (OBS_EXCLUDED_METHODS.has(prop)) return value;
      // The full gate, re-read per access: off, IPC capture off, or no session
      // salt yet ⇒ the original method identity, unchanged and unallocated.
      if (!obsEnabled() || !obsCaptureIpc() || !redactionReady()) return value;
      const cached = wrapped.get(prop);
      if (cached) return cached;
      const fresh = wrapMethod(target, prop, value as AnyFn);
      wrapped.set(prop, fresh);
      return fresh;
    },
  });
}

/** TEST ONLY — clears the supersession map between cases. */
export function resetIpcProxyForTests(): void {
  latestSpan.clear();
}
