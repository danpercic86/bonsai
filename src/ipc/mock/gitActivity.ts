/**
 * P87b — the git-activity stream in the mock IPC layer.
 *
 * `subscribeGitActivity` / `emitGitActivity` are the subscribe + fan-out seam
 * (mirroring the events bus / `GitActivityHub`). `runMockActivity(category, fn)`
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
 */
import { MOCK_PRE_PUSH_OUTPUT } from './hooksGate';
import { delay, query } from './repoState';
import type {
  AppError,
  GitActivityCategory,
  GitActivityEvent,
  GitActivityKind,
  GitPhaseKind,
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

/** MIRRORS `bonsai_core::git::activity::MAX_ACTIVITY_LINE_CHARS`. */
const MAX_ACTIVITY_LINE_CHARS = 2000;

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

  start(category: GitActivityCategory): void {
    this.emit('started', { category, phase: { kind: 'preparing' } });
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
  finished(code: number | undefined, success: boolean): void {
    this.emit('finished', code !== undefined ? { code, success } : { success });
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
  fn: () => Promise<T>,
): Promise<T> {
  s.start(category);

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
    s.finished(activityExitCode(), false);
    throw e;
  }
}

async function runFetch<T>(
  s: GitSequencer,
  category: 'fetch' | 'pull',
  fn: () => Promise<T>,
): Promise<T> {
  s.start(category);
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
    s.finished(activityExitCode(), false);
    throw e;
  }
}

async function runCommit<T>(
  s: GitSequencer,
  category: 'commit' | 'amend' | 'mergeCommit',
  fn: () => Promise<T>,
): Promise<T> {
  s.start(category);
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
    s.finished(activityExitCode(), false);
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

/**
 * Wrap a handler body in the git-activity stream. A no-op passthrough when nobody
 * is listening (mirrors the hub). Branches on the category family.
 */
export function runMockActivity<T>(
  category: GitActivityCategory,
  fn: () => Promise<T>,
): Promise<T> {
  if (!gitActivityActive()) return fn();
  const s = new GitSequencer(nextId());
  if (category === 'push' || category === 'forcePush') return runPush(s, category, fn);
  if (category === 'fetch' || category === 'pull') return runFetch(s, category, fn);
  return runCommit(s, category, fn);
}
