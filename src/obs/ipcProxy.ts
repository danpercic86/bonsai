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

/**
 * Newest span per `cmd` + repo, for the §4 `superseded` outcome.
 *
 * P122d — keyed by repo as well as `cmd`: with six tabs open, repo A's
 * `listBranches` resolving while repo B's was in flight is NOT a superseded
 * result (nothing discards it), yet a per-`cmd` key stamped it so. Replaying
 * session `s229d042a` with this key and the subscription exclusion below leaves
 * ~22 of its 300 `superseded` results (StrictMode double-mounts + forge
 * last-wins). Different ARGS for the same repo stay one key on purpose: that is
 * the classic last-wins supersession.
 */
const latestSpan = new Map<string, SpanId>();

type AnyFn = (...args: unknown[]) => unknown;

/**
 * P122c — one backend emit fans out to every mounted tab's listener (all repo
 * tabs stay mounted), and each delivery used to log its own `event` record: 2496
 * `onRepoChanged#0` records for 416 `repo-changed` emits (6.0×, six tabs), so
 * `event-storm` (§5, ≥20 per 1 s) fired on four backend events. The non-owning
 * listeners early-return on `repoId` (one refresh round per emit, 414 `watcher`
 * refreshes for 416 emits), so the fan-out is delivery, not work, and the rule's
 * count was the misleading part.
 *
 * The fix counts per backend emit, at the producer: Tauri's emit function (and
 * the mock's `Set` loop) hands the SAME payload object to every listener
 * synchronously in one turn, so deliveries of one slot sharing a payload object
 * within a turn — each to a distinct listener — are ONE emit and become one
 * record whose `listeners` is the true fan-out. A repeat delivery to the same
 * listener, a different payload object (channel chunks are always distinct) or a
 * primitive payload starts a new record. If a path ever dispatched
 * asynchronously this degrades to one record per delivery — the old behaviour.
 *
 * Costs, accepted: the record is written at the end of the turn, so records the
 * callbacks log synchronously precede it by `seq`; and `causedBy` is the FIRST
 * listener's subscribing trace.
 */
interface PendingDelivery {
  payload: object;
  listeners: Set<AnyFn>;
  causedBy: string | undefined;
}
const pendingDeliveries = new Map<string, PendingDelivery>();
let deliveryFlushQueued = false;

function logDelivery(name: string, listeners: number, causedBy: string | undefined): void {
  logRecord({
    kind: 'event',
    name,
    delivered: true,
    listeners,
    ...(causedBy ? { causedBy } : {}),
  });
}

function flushDeliveries(): void {
  deliveryFlushQueued = false;
  for (const [name, p] of pendingDeliveries) logDelivery(name, p.listeners.size, p.causedBy);
  pendingDeliveries.clear();
}

function noteDelivery(
  name: string,
  listener: AnyFn,
  payload: unknown,
  causedBy: string | undefined,
): void {
  if (typeof payload !== 'object' || payload === null) {
    logDelivery(name, 1, causedBy);
    return;
  }
  const pending = pendingDeliveries.get(name);
  if (pending !== undefined && pending.payload === payload && !pending.listeners.has(listener)) {
    pending.listeners.add(listener);
    return;
  }
  if (pending !== undefined) logDelivery(name, pending.listeners.size, pending.causedBy);
  pendingDeliveries.set(name, { payload, listeners: new Set([listener]), causedBy });
  if (!deliveryFlushQueued) {
    deliveryFlushQueued = true;
    queueMicrotask(flushDeliveries);
  }
}

/**
 * P122d — the repo half of the supersession key. `repoIdArg` only knows the
 * commands `rawArgPolicy.json` names (most reads — `listBranches`, `getStatus` —
 * have no row), so fall back to the first string argument, which is `repoId` on
 * every repo-scoped read. Unlike the `repo` field this never reaches a record,
 * so a wrong guess costs only the accuracy of an info-level stamp — nothing is
 * suppressed or attributed by it.
 */
function supersedeScope(repo: string | undefined, args: readonly unknown[]): string {
  if (repo !== undefined) return repo;
  const first = args.find((a) => typeof a === 'string');
  return typeof first === 'string' ? first : '';
}

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

/** §4 — callbacks emit an `event` record on DELIVERY, under the subscribing trace
 *  (P122c: one record per backend emit, see {@link noteDelivery}). */
function wrapCallback(fn: AnyFn, cmd: string, index: number, trace: string | undefined): AnyFn {
  const bound = bindTrace(fn as (...a: never[]) => unknown) as AnyFn;
  const name = `${cmd}#${index}`;
  const wrapped = (...cbArgs: unknown[]): unknown => {
    noteDelivery(name, wrapped, cbArgs[0], trace);
    return bound(...cbArgs);
  };
  return wrapped;
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

    // P122d — a subscription (any callback argument: `onRepoChanged(cb)`,
    // `streamGraph(…, onChunk)`) resolving is never "superseded": each tab's
    // listener is live and nothing discards it (39 of the 300 in `s229d042a`).
    const supersedable = !args.some((a) => typeof a === 'function');
    const supersedeKey = `${cmd}\0${supersedeScope(repo, args)}`;
    if (supersedable) latestSpan.set(supersedeKey, span);
    const started = performance.now();
    const finish = (outcome: IpcOutcome, result: unknown, err?: unknown): void => {
      const latest = latestSpan.get(supersedeKey);
      const superseded = supersedable && outcome === 'ok' && latest !== span;
      // Only the newest call deletes its key, so the map holds in-flight keys
      // only; an older call finding the key gone or replaced was superseded.
      if (supersedable && latest === span) latestSpan.delete(supersedeKey);
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

/** TEST ONLY — clears the supersession map and any pending delivery between cases. */
export function resetIpcProxyForTests(): void {
  latestSpan.clear();
  pendingDeliveries.clear();
  deliveryFlushQueued = false;
}
