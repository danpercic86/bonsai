/**
 * P87b — the git-activity stream in the mock IPC layer.
 *
 * `subscribeGitActivity` / `emitGitActivity` are the subscribe + fan-out seam
 * (mirroring the events bus / `GitActivityHub`). `runMockActivity(category, target, fn)`
 * wraps a push/commit/fetch handler body: it emits `started` → the category's
 * phase/line/hookDone/progress script → runs `fn` → `finished` (success from
 * resolve, failure from throw). A shared per-run sequencer gives a monotonic `seq`
 * and a real `elapsedMs`, exactly like `aiStream.ts`.
 *
 * Query seams (mirror `?aiSlow`/`?aiFail`), so every event kind + terminal state
 * is reachable in a plain browser:
 *   ?prePushHook  — a passing `pre-push` hook (runningHook phase + lines +
 *                   hookDone{success:true}) before the Network phase.
 *   ?prePushFail  — the failing `pre-push` hook: emit the verbatim
 *                   MOCK_PRE_PUSH_OUTPUT + hookDone{success:false} + a failed row,
 *                   AND throw the same `hookRejected` HookOutputDialog consumes.
 *   ?pushSlow     — a long Network phase (indeterminate bar, live elapsed).
 *   ?fetchSlow    — ramping structured `progress` ticks → the determinate bar +
 *                   `N / M objects` readout.
 *   ?fetchNoCount — a Network phase with NO progress → indeterminate fallback.
 *   ?gitFlood     — ~700 output lines (one exactly 2000 chars) → the 500-line cap,
 *                   `linesDropped`, the `⋯ trimmed` + `truncated` chips.
 *
 * FU-1 run-target seams (§3.10) — the `target` carried on `started`:
 *   ?fetchAll      — fetch with NO target → the frontend-derived `Fetch all
 *                    remotes`. Already the default; named so the case is
 *                    addressable.
 *   ?gitNoTarget   — forces `target: null` for EVERY category → the
 *                    no-placeholder rule (the row is the bare noun).
 *   ?gitLongTarget — a >=90-char ref on push/force-push → the 22ch ellipsis with
 *                    the leaf intact, recoverable from `title`.
 *   ?gitBidiTarget — a ref carrying U+202E on push/force-push, fed through
 *                    `mockActivityTarget` → the emitted string must be exactly
 *                    `origin/main`.
 *
 * P119: every other category runs the plain script (`runPlain`), which also
 * carries `outcome` / `targetCount` and, on failure, the error-message line
 * before `finished` (every script does, except for `hookRejected`).
 *   ?gitSlowLocal  — an 800 ms pause inside every plain run (a visible row).
 */
import { MOCK_PRE_PUSH_OUTPUT } from './hooksGate';
import { delay, query } from './repoState';
import type {
  AppError,
  GitActivityCategory,
  GitActivityEvent,
  GitActivityKind,
  GitPhaseKind,
  GitRunOutcome,
  GitTransferProgress,
} from '../types';

/** Every live `gitActivitySubscribe` callback. A reload re-subscribes; the mock
 *  keeps them all (the real backend prunes on send failure — harmless here). */
const subscribers: Array<(e: GitActivityEvent) => void> = [];

/** Register a long-lived git-activity listener (the mock's `git_activity_subscribe`). */
export function subscribeGitActivity(onEvent: (e: GitActivityEvent) => void): void {
  subscribers.push(onEvent);
}

/** Fan one event out to every subscriber. A no-op when nobody is listening
 *  (mirrors `GitActivityHub::emit`). */
export function emitGitActivity(event: GitActivityEvent): void {
  for (const cb of subscribers) cb(event);
}

/** True while ≥1 subscriber is attached (mirrors `GitActivityHub::is_active`). */
export function gitActivityActive(): boolean {
  return subscribers.length > 0;
}

// ---------------------------------------------------------------- seams

const PRE_PUSH_HOOK = query('prePushHook') !== null;
const PRE_PUSH_FAIL = query('prePushFail') !== null;
const PUSH_SLOW = query('pushSlow') !== null;
const FETCH_SLOW = query('fetchSlow') !== null;
const FETCH_NO_COUNT = query('fetchNoCount') !== null;
const GIT_FLOOD = query('gitFlood') !== null;
const FETCH_ALL = query('fetchAll') !== null;
const GIT_NO_TARGET = query('gitNoTarget') !== null;
const GIT_LONG_TARGET = query('gitLongTarget') !== null;
const GIT_BIDI_TARGET = query('gitBidiTarget') !== null;
const GIT_SLOW_LOCAL = query('gitSlowLocal') !== null;

/** MIRRORS `bonsai_core::git::activity::MAX_ACTIVITY_LINE_CHARS`. */
const MAX_ACTIVITY_LINE_CHARS = 2000;

/** MIRRORS `bonsai_core::git::activity::MAX_ACTIVITY_TARGET_CHARS`. */
const MAX_ACTIVITY_TARGET_CHARS = 255;

/** `?gitLongTarget` — a ref long enough to overflow the 22ch box several times
 *  over, whose LEAF is the part that names the thing (P111 R3). */
export const MOCK_LONG_TARGET =
  'origin/feature/very-long-experimental-branch/with-many-nested-path-segments/retry-budget-tuning';

/** `?gitBidiTarget` — an RTL override spliced into a ref (written as the escape,
 *  never the literal char). `mockActivityTarget` must reduce it to exactly
 *  `origin/main`. */
export const MOCK_BIDI_TARGET = 'origin/ma\u{202e}in';

/**
 * MIRRORS `ActivityTarget::new` (P87b-FU1-run-target §2). The mock is a second
 * backend: its fixtures cross the same boundary, so they get the same funnel.
 * Strips C0/C1 controls + the bidi overrides/isolates (U+200E/200F,
 * U+202A-202E, U+2066-2069) + the zero-width chars (U+200B-200D, U+FEFF),
 * trims, then caps at 255 CHARS with a trailing `…`.
 *
 * Written as a code-point filter rather than a regex both because that is the
 * exact shape of Rust's `strip_control_chars` and because a control-char class
 * in a regex literal is a lint error. `[...s]` iterates code points, matching
 * Rust's `chars()`.
 */
export function mockActivityTarget(raw: string | null): string | null {
  if (raw === null) return null;
  const clean = [...raw]
    .filter((ch) => {
      const cp = ch.codePointAt(0) ?? 0;
      if (cp <= 0x1f || (cp >= 0x7f && cp <= 0x9f)) return false; // C0 / C1
      if (cp >= 0x200b && cp <= 0x200f) return false; // ZWSP/ZWNJ/ZWJ + LRM/RLM
      if (cp >= 0x202a && cp <= 0x202e) return false; // bidi embeddings/overrides
      if (cp >= 0x2066 && cp <= 0x2069) return false; // bidi isolates
      return cp !== 0xfeff; // BOM
    })
    .join('')
    .trim();
  if (clean === '') return null;
  const chars = [...clean];
  if (chars.length <= MAX_ACTIVITY_TARGET_CHARS) return clean;
  return `${chars.slice(0, MAX_ACTIVITY_TARGET_CHARS - 1).join('')}…`;
}

/** §3.10 — the query-seam override for a run's target. The call site's fixture
 *  is the default; a seam replaces it. */
function seamTarget(category: GitActivityCategory, target: string | null): string | null {
  if (GIT_NO_TARGET) return null;
  // Scoped like ?gitLongTarget: a fetch run's target is always null in the real
  // backend (fetch-all has no single ref, run-target F-3), so overriding EVERY
  // category here would put a fetch-with-target state on screen that the app
  // cannot produce.
  if (GIT_BIDI_TARGET && (category === 'push' || category === 'forcePush')) {
    return MOCK_BIDI_TARGET;
  }
  if (GIT_LONG_TARGET && (category === 'push' || category === 'forcePush')) {
    return MOCK_LONG_TARGET;
  }
  // Redundant by construction (fetch-all is the only fetch entry point, so the
  // call site already passes null) — the seam exists so the case has a name.
  if (category === 'fetch' && FETCH_ALL) return null;
  return target;
}

/** Passing pre-push output (a "refusal" body is MOCK_PRE_PUSH_OUTPUT, used only on
 *  the fail path so the dialog body stays verbatim). */
const MOCK_PRE_PUSH_OK = [
  'Running pre-push checks…',
  'gitleaks................................................................Passed',
  'detect-secrets..........................................................Passed',
];

let counter = 0;

/** One monotonic `seq` per run, a real `elapsedMs`, and the fixed event shape. */
class GitSequencer {
  private seq = 0;
  private readonly startedAt = Date.now();
  constructor(readonly id: string) {}

  private emit(kind: GitActivityKind, extra: Partial<GitActivityEvent> = {}): void {
    emitGitActivity({
      id: this.id,
      seq: this.seq++,
      kind,
      elapsedMs: Date.now() - this.startedAt,
      ...extra,
    });
  }

  /** `target` rides on `started` ONLY, through the sanitizer mirror, and the key
   *  is DROPPED when null so the wire shape matches serde's
   *  `skip_serializing_if = "Option::is_none"`. */
  start(category: GitActivityCategory, target: string | null, count?: number): void {
    // P119 §2.7 mirror of `RunSubject::many`: ≥2 items → a count and NO target.
    if (count !== undefined && count >= 2) {
      this.emit('started', { category, phase: { kind: 'preparing' }, targetCount: count });
      return;
    }
    const clean = mockActivityTarget(target);
    this.emit('started', {
      category,
      phase: { kind: 'preparing' },
      ...(clean !== null ? { target: clean } : {}),
    });
  }
  phase(kind: GitPhaseKind, hook?: string): void {
    this.emit('phase', { phase: hook !== undefined ? { kind, hook } : { kind } });
  }
  stdout(line: string): void {
    this.emit('stdoutLine', { line });
  }
  stderr(line: string): void {
    this.emit('stderrLine', { line });
  }
  hookDone(hook: string, code: number, success: boolean): void {
    this.emit('hookDone', { hook, code, success });
  }
  progress(p: GitTransferProgress): void {
    this.emit('progress', { progress: p });
  }
  /** `outcome` rides on a SUCCESSFUL `finished` only; the key is dropped when
   *  null (mirror of `ActivityEmitter::finish`, P119 §2.6). */
  finished(code: number | undefined, success: boolean, outcome: GitRunOutcome | null = null): void {
    this.emit('finished', {
      ...(code !== undefined ? { code } : {}),
      success,
      ...(success && outcome !== null ? { outcome } : {}),
    });
  }
  /** P119 §2.8 mirror: a failed run's last line is the user-facing error
   *  message, then `finished`. Skipped for `hookRejected` (its hook output is
   *  already on the stream). */
  failed(e: unknown): void {
    if (isAppError(e) && e.kind !== 'hookRejected') this.stderr(e.message);
    this.finished(activityExitCode(), false);
  }
}

function nextId(): string {
  counter += 1;
  return `git-mock-${counter}`;
}

/** Best-effort AppError → exit code (mock: every failure is exit 1). */
function activityExitCode(): number {
  return 1;
}

// ---------------------------------------------------------------- scripts

async function runPush<T>(
  s: GitSequencer,
  category: 'push' | 'forcePush',
  target: string | null,
  fn: () => Promise<T>,
): Promise<T> {
  s.start(category, target);

  // ?prePushFail — the failing hook: verbatim output + failed row + the same
  // rejection HookOutputDialog consumes (both surfaces, from one seam).
  if (PRE_PUSH_FAIL) {
    s.phase('runningHook', 'pre-push');
    await delay(250);
    for (const line of MOCK_PRE_PUSH_OUTPUT.split('\n')) s.stderr(line);
    s.hookDone('pre-push', 1, false);
    s.finished(1, false);
    const rejection: AppError = { kind: 'hookRejected', message: MOCK_PRE_PUSH_OUTPUT };
    throw rejection;
  }

  try {
    if (PRE_PUSH_HOOK) {
      s.phase('runningHook', 'pre-push');
      await delay(PUSH_SLOW ? 400 : 250);
      for (const line of MOCK_PRE_PUSH_OK) s.stdout(line);
      s.hookDone('pre-push', 0, true);
    }
    s.phase('network');
    if (GIT_FLOOD) emitFlood(s);
    if (PUSH_SLOW) await delay(1500);
    const result = await fn();
    s.finished(0, true);
    return result;
  } catch (e) {
    s.failed(e);
    throw e;
  }
}

async function runFetch<T>(
  s: GitSequencer,
  category: 'fetch' | 'pull',
  target: string | null,
  fn: () => Promise<T>,
): Promise<T> {
  s.start(category, target);
  try {
    s.phase('network');
    if (FETCH_NO_COUNT) {
      // Indeterminate: no progress events at all.
      await delay(FETCH_SLOW ? 1800 : 200);
    } else if (FETCH_SLOW) {
      await emitProgressRamp(s);
    }
    if (category === 'pull') s.phase('finalizing');
    const result = await fn();
    s.finished(0, true);
    return result;
  } catch (e) {
    s.failed(e);
    throw e;
  }
}

async function runCommit<T>(
  s: GitSequencer,
  category: 'commit' | 'amend' | 'mergeCommit',
  target: string | null,
  fn: () => Promise<T>,
): Promise<T> {
  s.start(category, target);
  s.phase('runningHook', 'pre-commit');
  await delay(120);
  try {
    const result = await fn();
    // Passing commit: record the three commit-family hooks (§4.2) and the write.
    s.hookDone('pre-commit', 0, true);
    s.phase('finalizing');
    s.hookDone('commit-msg', 0, true);
    s.hookDone('post-commit', 0, true);
    s.finished(0, true);
    return result;
  } catch (e) {
    if (isAppError(e) && e.kind === 'hookRejected') {
      for (const line of e.message.split('\n')) s.stderr(line);
      s.hookDone('pre-commit', 1, false);
    }
    s.failed(e);
    throw e;
  }
}

/** ~12 structured `progress` ticks ramping 0→total (§14.11) → the determinate
 *  bar + `N / M objects` readout. */
async function emitProgressRamp(s: GitSequencer): Promise<void> {
  const total = 50_000;
  const ticks = 12;
  for (let i = 1; i <= ticks; i += 1) {
    await delay(150);
    const received = Math.round((total * i) / ticks);
    s.progress({
      receivedObjects: received,
      totalObjects: total,
      indexedObjects: received,
      receivedBytes: received * 80,
    });
  }
}

/** ~700 output lines (one exactly 2000 chars) → the 500-line cap + chips. */
function emitFlood(s: GitSequencer): void {
  for (let i = 1; i <= 700; i += 1) s.stdout(`remote: counting objects ${i}/700`);
  s.stdout(`${'x'.repeat(MAX_ACTIVITY_LINE_CHARS - 1)}…`);
}

function isAppError(e: unknown): e is AppError {
  return typeof e === 'object' && e !== null && 'kind' in e && 'message' in e;
}

/** P119 §5.1 — the categories whose real command emits `phase(Network)` right
 *  after `started` (§1 `Net` column). */
const MOCK_NETWORK_CATEGORIES: ReadonlySet<GitActivityCategory> = new Set<GitActivityCategory>([
  'pushTag',
  'deleteRemoteTag',
  'forceRefreshTag',
  'submoduleAdd',
  'submoduleUpdate',
  'cloneRepo',
]);

/** Per-run options for `runMockActivity` (P119 §5.1). */
export interface MockActivityOpts<T> {
  /** Mirror of the command's §2.6 classifier: the SUCCESS result → `outcome`. */
  classify?: (result: T) => GitRunOutcome | null;
  /** Item count of a multi-item op; `>= 2` → `targetCount` and no `target`. */
  count?: number;
}

/** P119 — every category without a hook/transfer script of its own:
 *  `started` → `phase(network)` for the network rows (+ the determinate clone
 *  ramp under `?fetchSlow`) → `?gitSlowLocal` pause → `fn` → `finished` with
 *  the classified outcome, or the failure line + a failed `finished`. */
async function runPlain<T>(
  s: GitSequencer,
  category: GitActivityCategory,
  target: string | null,
  fn: () => Promise<T>,
  opts: MockActivityOpts<T>,
): Promise<T> {
  s.start(category, target, opts.count);
  try {
    if (MOCK_NETWORK_CATEGORIES.has(category)) {
      s.phase('network');
      if (category === 'cloneRepo' && FETCH_SLOW) await emitProgressRamp(s);
    }
    if (GIT_SLOW_LOCAL) await delay(800);
    const result = await fn();
    s.finished(0, true, opts.classify?.(result) ?? null);
    return result;
  } catch (e) {
    s.failed(e);
    throw e;
  }
}

/**
 * Wrap a handler body in the git-activity stream. A no-op passthrough when nobody
 * is listening (mirrors the hub). Dispatch: the push family → `runPush`,
 * fetch/pull → `runFetch`, EXPLICITLY commit/amend/mergeCommit → `runCommit`
 * (their hook script), and every other category → `runPlain` — so a new
 * category can never be dressed up as a commit.
 */
export function runMockActivity<T>(
  category: GitActivityCategory,
  target: string | null,
  fn: () => Promise<T>,
  opts: MockActivityOpts<T> = {},
): Promise<T> {
  if (!gitActivityActive()) return fn();
  const s = new GitSequencer(nextId());
  const seamed = seamTarget(category, target);
  switch (category) {
    case 'push':
    case 'forcePush':
      return runPush(s, category, seamed, fn);
    case 'fetch':
    case 'pull':
      return runFetch(s, category, seamed, fn);
    case 'commit':
    case 'amend':
    case 'mergeCommit':
      return runCommit(s, category, seamed, fn);
    default:
      return runPlain(s, category, seamed, fn, opts);
  }
}
