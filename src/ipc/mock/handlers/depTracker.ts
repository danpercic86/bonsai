// P125 §8 / E5 — the mock dependency tracker.
//
// Mode: `?deps=off|loading|populated|uptodate|stale|offline|error|empty|mirror|large|unverified|netprofile`,
// read at CALL time (so the harness and tests can switch it without a reload).
// With no `?deps`, the mode follows the mock settings' `depTracker.enabled`
// (off ⇒ `off`, on ⇒ `populated`).
//
// `depsCheckNow` emits started → 3×progress → finished and then serves a fresh
// report; disabling the tracker in Settings emits `cleared` (the Rust
// `apply_config` true→false path), enabling it starts a check for the active repo
// (the false→true path).
import type { AppError, IpcApi, UiSettings, Unsubscribe } from '../../types';
import type {
  DepRunningCheck,
  DepsChangedKind,
  DepsChangedPayload,
  DepTrackerSettings,
  DepTrackerView,
} from '../../types/depTracker';
import { buildDepView, DEP_VIEW_POPULATED, isDepMockMode } from '../../fixtures/depTracker';
import type { DepMockMode } from '../../fixtures/depTracker';
import { depsChangedListeners, mockMcp } from '../events';
import { readUiSettings } from '../persistence';
import { delay, query as urlParam, repos, requireRepo } from '../repoState';

/** Verbatim `AppError::Other` text from `deps/exec.rs::start_run` (§6.4). */
export const DEPS_DISABLED_MESSAGE = 'dependency tracker is disabled';

/** Work units the mock check "plans" (deps.dev lookups + OSV batches [UI-b]). */
export const MOCK_CHECK_TOTAL = 57;
/** Gap between the mock check's events; the backend caps Progress at 4/s. */
export const MOCK_CHECK_STEP_MS = 300;

interface RepoDepState {
  running: DepRunningCheck | null;
  /** Set once a mock check finished: the view then serves a fresh report. */
  completedAtMs: number | null;
  timers: ReturnType<typeof setTimeout>[];
}

const depState = new Map<string, RepoDepState>();
/** Per-process `u64` counter starting at 1 (E6). */
let nextCheckId = 1;

function stateOf(repoId: string): RepoDepState {
  let s = depState.get(repoId);
  if (s === undefined) {
    s = { running: null, completedAtMs: null, timers: [] };
    depState.set(repoId, s);
  }
  return s;
}

/** The effective harness mode right now. */
export function depMockMode(): DepMockMode {
  const q = urlParam('deps');
  if (isDepMockMode(q)) return q;
  return readUiSettings().depTracker.enabled ? 'populated' : 'off';
}

/**
 * `?deps=<mode>` PINS the harness, so it also pins the setting App reads at
 * launch: the hook does zero IPC while `depTracker.enabled` is false (the
 * cold-open ratchet), and a pinned `populated` / `stale` / … mode must still
 * render. A pinned `off` reads as disabled. No pin ⇒ the stored value.
 */
export function pinDepTrackerSetting(s: UiSettings): UiSettings {
  const q = urlParam('deps');
  if (!isDepMockMode(q)) return s;
  return { ...s, depTracker: { ...s.depTracker, enabled: q !== 'off' } };
}

function emit(
  repoId: string,
  kind: DepsChangedKind,
  checkId: number | null,
  done: number | null = null,
  total: number | null = null,
): void {
  const payload: DepsChangedPayload = { repoId, checkId, kind, done, total };
  for (const cb of [...depsChangedListeners]) cb(payload);
}

function viewFor(repoId: string, nowMs: number): DepTrackerView {
  const mode = depMockMode();
  // A fresh copy per call, like a real IPC response: a consumer mutating it
  // can never corrupt the shared fixture tables.
  const view = structuredClone(buildDepView(mode, repoId, nowMs));
  if (mode === 'off') return view;
  const s = depState.get(repoId);
  if (s === undefined) return view;
  if (s.completedAtMs !== null) {
    // After a mock check: the mode's report (or the populated one when the mode
    // had none), freshly stamped; the failure and the stale flag are cleared.
    const base = view.report ?? structuredClone(DEP_VIEW_POPULATED.report);
    if (base !== null) {
      view.report = { ...base, repoId, checkedAtMs: s.completedAtMs };
      view.nextCheckMs = s.completedAtMs + 12 * 3_600_000;
    }
    view.lastError = null;
    view.manifestsChanged = false;
    view.running = null;
  }
  if (s.running !== null) view.running = { ...s.running };
  return view;
}

function startMockCheck(repoId: string): number {
  const s = stateOf(repoId);
  if (s.running !== null) return s.running.checkId;
  const checkId = nextCheckId;
  nextCheckId += 1;
  // Nothing planned yet at `started` (total 0 ⇒ the UI reads "Checking…");
  // the first progress event carries the real total.
  s.running = { checkId, startedAtMs: Date.now(), done: 0, total: 0 };
  emit(repoId, 'started', checkId);
  const steps = [19, 38, MOCK_CHECK_TOTAL];
  steps.forEach((done, i) => {
    s.timers.push(
      setTimeout(() => {
        if (s.running?.checkId !== checkId) return;
        s.running = { ...s.running, done, total: MOCK_CHECK_TOTAL };
        emit(repoId, 'progress', checkId, done, MOCK_CHECK_TOTAL);
      }, MOCK_CHECK_STEP_MS * (i + 1)),
    );
  });
  s.timers.push(
    setTimeout(() => {
      if (s.running?.checkId !== checkId) return;
      s.running = null;
      s.completedAtMs = Date.now();
      s.timers = [];
      emit(repoId, 'finished', checkId);
    }, MOCK_CHECK_STEP_MS * (steps.length + 1)),
  );
  return checkId;
}

/**
 * Called by the mock `setUiSettings` after persisting — the stand-in for
 * `deps::apply_config`. true→false: cancel everything, forget every result and
 * emit `cleared` per open repo. false→true: start a check for the active repo.
 */
export function applyMockDepTrackerConfig(prev: DepTrackerSettings, next: DepTrackerSettings): void {
  if (prev.enabled && !next.enabled) {
    for (const s of depState.values()) for (const t of s.timers) clearTimeout(t);
    depState.clear();
    for (const repoId of repos.keys()) emit(repoId, 'cleared', null);
    return;
  }
  if (!prev.enabled && next.enabled) {
    const active = mockMcp.activeRepo;
    if (active !== null && repos.has(active) && depMockMode() !== 'off') startMockCheck(active);
  }
}

/** Test seam: drop all mock tracker state (timers included). */
export function resetMockDepTrackerForTests(): void {
  for (const s of depState.values()) for (const t of s.timers) clearTimeout(t);
  depState.clear();
  nextCheckId = 1;
}

export const depTrackerHandlers = {
  async getDepTracker(repoId: string): Promise<DepTrackerView> {
    await delay(80);
    requireRepo(repoId);
    return viewFor(repoId, Date.now());
  },

  async depsCheckNow(repoId: string): Promise<number> {
    await delay(50);
    requireRepo(repoId);
    if (depMockMode() === 'off') {
      const err: AppError = { kind: 'other', message: DEPS_DISABLED_MESSAGE };
      throw err;
    }
    return startMockCheck(repoId);
  },

  async onDepsChanged(cb: (p: DepsChangedPayload) => void): Promise<Unsubscribe> {
    depsChangedListeners.add(cb);
    return () => {
      depsChangedListeners.delete(cb);
    };
  },
} satisfies Partial<IpcApi>;
