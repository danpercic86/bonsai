// Spec-007: replay-mode state machine — pure math, no DOM, no React (plan
// decision 2). Row 0 is the NEWEST commit, so the reveal cutoff `c` starts at
// `n` (nothing revealed) and animates DOWN to 0 (everything); reveal = [c, n).
//
// Clock mapping: one cumulative narrative-time array `T[k]` over rows walked
// oldest→newest (chronological index k ⇔ row n-1-k), built from committerTs
// deltas clamped to [MIN_STEP_MS, MAX_STEP_MS] narrative units (long real-world
// gaps compress, bursts stay visible), then normalized so the full run lasts
// `totalMs` at 1×. Playhead→cutoff is a binary search — no per-commit stalls.
//
// Conventions (recorded): `T[0] = 0` and the search uses `<=`, so playhead 0
// reveals the root commit — "Home = start of history" shows the first commit,
// and a single-commit repo reveals its row at t=0 (UI contract §5). The
// pre-play blank state (cutoff = n) exists only via `initialState` (§5
// "Entering": blank canvas until autoplay's first tick).
import type { GraphLayout } from '../../ipc';

export type ReplayStatus = 'paused' | 'playing' | 'finished';
export type ReplaySpeed = 1 | 2 | 4;

/** Narrative-unit clamp for one commit→commit gap (pre-normalization). */
export const MIN_STEP_MS = 40;
export const MAX_STEP_MS = 400;
/** Ideal 1× duration budget per commit, clamped to [MIN, MAX]_TOTAL_MS. */
export const MS_PER_COMMIT = 45;
export const MIN_TOTAL_MS = 10_000;
export const MAX_TOTAL_MS = 90_000;
export const REPLAY_SPEEDS: readonly ReplaySpeed[] = [1, 2, 4];

export interface ReplayModel {
  /** Total rows. */
  readonly n: number;
  /** T[k]: narrative ms at which chronological commit k (row n-1-k) reveals. */
  readonly cumTime: Float64Array;
  /** Normalized 1× duration. */
  readonly totalMs: number;
  /** False under reduced motion or when the layout is empty. */
  readonly canPlay: boolean;
}

export interface ReplayState {
  readonly status: ReplayStatus;
  /** Narrative clock ∈ [0, totalMs]. */
  readonly playheadMs: number;
  /** Derived row cutoff c ∈ [0, n]; reveal = [c, n). */
  readonly cutoff: number;
  readonly speed: ReplaySpeed;
  /** Auto-follow engaged (frontier pinned at viewport top while playing). */
  readonly follow: boolean;
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

export function buildReplayModel(layout: GraphLayout, reducedMotion: boolean): ReplayModel {
  const nodes = layout.nodes;
  const n = nodes.length;
  const cum = new Float64Array(n);
  let t = 0;
  for (let k = 1; k < n; k += 1) {
    // committerTs is seconds; the delta is clamped per gap, so units only shape
    // relative pacing — normalization below sets the absolute scale.
    const delta = (nodes[n - 1 - k].committerTs - nodes[n - k].committerTs) * 1000;
    t += clamp(delta, MIN_STEP_MS, MAX_STEP_MS);
    cum[k] = t;
  }
  const totalMs = clamp(n * MS_PER_COMMIT, MIN_TOTAL_MS, MAX_TOTAL_MS);
  const raw = n > 0 ? cum[n - 1] : 0;
  if (raw > 0) {
    const scale = totalMs / raw;
    for (let k = 0; k < n; k += 1) cum[k] *= scale;
    // fp guard: `raw * (totalMs / raw)` can land one ulp ABOVE totalMs, which
    // would leave cutoff at 1 forever (row 0 only reveals when T[n-1] <= totalMs).
    cum[n - 1] = totalMs;
  }
  return { n, cumTime: cum, totalMs, canPlay: !reducedMotion && n > 0 };
}

/** Cutoff = n − (count of T[k] <= ms), via binary search (upper bound). */
export function cutoffForPlayhead(m: ReplayModel, ms: number): number {
  const T = m.cumTime;
  let lo = 0;
  let hi = m.n; // first index with T[i] > ms
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (T[mid] <= ms) lo = mid + 1;
    else hi = mid;
  }
  return m.n - lo;
}

/** Narrative time at which cutoff `c` was reached (0 for the blank state). */
export function playheadForCutoff(m: ReplayModel, c: number): number {
  if (c >= m.n || m.n === 0) return 0;
  return m.cumTime[m.n - c - 1];
}

/** Blank pre-play state: nothing revealed, paused at the start (§5 Entering). */
export function initialState(m: ReplayModel): ReplayState {
  return { status: 'paused', playheadMs: 0, cutoff: m.n, speed: 1, follow: true };
}

/** Advance the narrative clock by `dtMs` wall-clock ms. Playing only. */
export function tick(m: ReplayModel, s: ReplayState, dtMs: number): ReplayState {
  if (s.status !== 'playing') return s;
  const p = Math.min(m.totalMs, s.playheadMs + dtMs * s.speed);
  return {
    ...s,
    playheadMs: p,
    cutoff: cutoffForPlayhead(m, p),
    status: p >= m.totalMs ? 'finished' : 'playing',
  };
}

/** Structural no-op when `!canPlay` (reduced motion / empty layout). From
 *  `finished` this is "Replay again": reset to the blank start and play. */
export function play(m: ReplayModel, s: ReplayState): ReplayState {
  if (!m.canPlay) return s;
  if (s.status === 'finished') {
    return { status: 'playing', playheadMs: 0, cutoff: m.n, speed: s.speed, follow: true };
  }
  return { ...s, status: 'playing', follow: true };
}

export function pause(s: ReplayState): ReplayState {
  return s.status === 'playing' ? { ...s, status: 'paused' } : s;
}

/** Scrub to a track fraction ∈ [0, 1]. Scrubbing while playing pauses (UI §5). */
export function scrubTo(m: ReplayModel, s: ReplayState, fraction: number): ReplayState {
  const p = clamp(fraction, 0, 1) * m.totalMs;
  return {
    ...s,
    playheadMs: p,
    cutoff: cutoffForPlayhead(m, p),
    status: m.n > 0 && p >= m.totalMs ? 'finished' : 'paused',
  };
}

/** Keyboard scrub: move the cutoff by `deltaRows` (positive = reveal more).
 *  Row-exact; the playhead snaps to the new cutoff's narrative time. Pauses. */
export function stepRows(m: ReplayModel, s: ReplayState, deltaRows: number): ReplayState {
  const c = clamp(s.cutoff - deltaRows, 0, m.n);
  return {
    ...s,
    cutoff: c,
    playheadMs: playheadForCutoff(m, c),
    status: m.n > 0 && c === 0 ? 'finished' : 'paused',
  };
}

/** ←/→ step size (UI contract §3.3): max(1, n/500) rows. */
export function keyStepRows(n: number): number {
  return Math.max(1, Math.round(n / 500));
}

export function setSpeed(s: ReplayState, sp: ReplaySpeed): ReplayState {
  return s.speed === sp ? s : { ...s, speed: sp };
}

export function setFollow(s: ReplayState, follow: boolean): ReplayState {
  return s.follow === follow ? s : { ...s, follow };
}
