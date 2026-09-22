/**
 * P117 §2.2 — lifting the canonical `repoId` out of an outgoing IPC call, so an
 * `ipc.call` record can be attributed to a repo and the `mutations` timeline the
 * anomaly detector keeps becomes repo-aware (§2.4).
 *
 * ## Why this needs a table at all
 *
 * The contract says "lift `args.repoId` when the invoke args object has a string
 * `repoId`". There is no such object here: every `IpcApi` method is
 * **positional** (`streamGraph(repoId, filter, onChunk)`), and only
 * `src/ipc/tauri/invoke.ts` ever sees a keyed payload map — by which point the
 * observability proxy has already emitted its record. So the parameter NAME has
 * to come from a table: primarily `rawArgPolicy.json`, which exists to name
 * positional arguments and already names `repoId` at position 0 in 100 of its
 * 119 rows.
 *
 * ## Why a second, local table (P117 review fix 1)
 *
 * 10 commands that `is_mutation_cmd` (`src-tauri/src/obs/anomaly.rs`) recognises
 * as mutations have **no** `rawArgPolicy.json` row at all — `fetch`, `pull`,
 * `push`, `rebaseContinue|Skip|Abort`, `cherrypickContinue|Abort`,
 * `revertContinue|Abort`. Unattributed, each of them suppresses `cache-collapse`
 * in EVERY open repo for 10 s and `redundant-refresh` for 1 s (§2.4: an
 * unattributed mutation intervenes everywhere) — so a user-pressed fetch in repo
 * A blinds the detector for repos B…E.
 *
 * Adding policy rows would fix attribution but would ALSO start emitting
 * `args: {repoId}` for fetch/pull/push in **raw** mode: a real, if benign,
 * privacy widening for a fix that is supposed to buy signal, not exposure. So
 * the position comes from [`REPO_PARAM_FALLBACK`] below — consulted only when
 * the policy yields nothing, never overriding it, and read by nothing that
 * writes to disk.
 *
 * ## Why not "the first string argument"
 *
 * Because a WRONG lift is worse than no lift. An `ipc.call` whose `repo` names
 * the wrong repo makes a real mutation stop suppressing anything, i.e. it turns
 * a suppression into a FALSE POSITIVE. An absent `repo`, by contrast, lands the
 * mutation in the unattributed bucket, where §2.4 has it suppress everywhere —
 * a missed anomaly at worst. Unknown ⇒ omit is the safe direction, so that is
 * what this does, for both tables.
 *
 * ## Mode independence
 *
 * This reads the policy table only as a NAME map. It is not gated on `raw`
 * mode and it never copies a value into `args`: `repo` is a base-record field
 * present in both modes (which is the whole reason §2.2 put it there), and the
 * Rust writer is what redacts it.
 */
import { RAW_ARG_POLICY } from './rawArgPolicy';

/** Untyped view for `cmd`-keyed lookup — the proxy only ever has a `string`. */
const POLICY_LOOKUP: Readonly<Record<string, readonly (string | null)[] | undefined>> =
  RAW_ARG_POLICY;

/** The parameter name that carries the canonical repo id. */
const REPO_PARAM = 'repoId';

/**
 * Review fix 1 — `repoId` POSITIONS for recognised mutations that
 * `rawArgPolicy.json` does not list, so they attribute without widening
 * raw-mode `args` (see the module note).
 *
 * A `Map`, not an object literal, so a `cmd` like `constructor` or `toString`
 * cannot reach `Object.prototype` and hand back a non-number. Positions are
 * pinned against the real `IpcApi` declarations by
 * `rawArgPolicy.test.ts` ("every recognised mutation attributes"), so a
 * signature change that inserts a parameter before `repoId` fails there rather
 * than silently lifting the wrong argument.
 */
const REPO_PARAM_FALLBACK: ReadonlyMap<string, number> = new Map([
  ['fetch', 0],
  ['pull', 0],
  ['push', 0],
  ['rebaseContinue', 0],
  ['rebaseSkip', 0],
  ['rebaseAbort', 0],
  ['cherrypickContinue', 0],
  ['cherrypickAbort', 0],
  ['revertContinue', 0],
  ['revertAbort', 0],
]);

/**
 * The argument index holding the repo id, or `undefined` if unknown.
 *
 * `Array.isArray`, not a truthiness test: `POLICY_LOOKUP` is a plain object, so
 * `cmd === 'constructor'` resolves through `Object.prototype` to a function and
 * `row.indexOf(...)` would THROW inside the IPC proxy. Not reachable from the
 * proxy (its `cmd` is a real `IpcApi` property name), but the proxy wraps every
 * user-facing call and must not be able to throw over a lookup.
 */
function repoParamPosition(cmd: string): number | undefined {
  const row = POLICY_LOOKUP[cmd];
  const at = Array.isArray(row) ? row.indexOf(REPO_PARAM) : -1;
  return at >= 0 ? at : REPO_PARAM_FALLBACK.get(cmd);
}

/**
 * The raw `repoId` this call is about, or `undefined` when the command is not
 * repo-scoped, is absent from both name tables, or passed something that is not
 * a non-empty string there.
 */
export function repoIdArg(cmd: string, args: readonly unknown[]): string | undefined {
  const at = repoParamPosition(cmd);
  if (at === undefined) return undefined;
  const value = args[at];
  return typeof value === 'string' && value.length > 0 ? value : undefined;
}
