/** T3.4 — scheduler.ts (job-status seeds, backoff math, synthetic job runs +
 *  event dispatch ordering, timer arming) and events.ts (mcpStatusOf shapes). */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  applyMockJobTimers,
  completeMockJobRun,
  jobStatuses,
  mockEffectiveIntervalMs,
  seedJobStatuses,
} from './scheduler';
import { jobStatusListeners, mcpStatusOf, mockMcp, repoChangedListeners } from './events';
import { createRepoState, repos } from './repoState';
import { DEFAULT_UI_SETTINGS, writeUiSettings } from './persistence';
import type { JobStatusChangedPayload, RepoChangedPayload, UiSettings } from '../types';

const REPO_ID = '/mock/bonsai-fixture';

beforeEach(() => {
  vi.useFakeTimers();
  window.localStorage.clear();
  repos.set(REPO_ID, createRepoState(REPO_ID));
});

afterEach(() => {
  // Disarm any armed interval timers, then drop state.
  applyMockJobTimers(structuredClone(DEFAULT_UI_SETTINGS));
  vi.useRealTimers();
  repos.delete(REPO_ID);
  jobStatuses.clear();
  jobStatusListeners.clear();
  repoChangedListeners.clear();
  window.localStorage.clear();
});

describe('seedJobStatuses', () => {
  it('seeds autoFetch (success 2 min ago) + healthRefresh (never run), idempotently', () => {
    const list = seedJobStatuses(REPO_ID);
    expect(list.map((s) => s.job)).toEqual(['autoFetch', 'healthRefresh']);
    expect(list[0]).toMatchObject({ lastOutcome: 'success', consecutiveFailures: 0 });
    expect(list[1]).toMatchObject({ lastRunMs: null, lastOutcome: null });
    expect(seedJobStatuses(REPO_ID)).toBe(list); // same array, not re-seeded
  });
});

describe('mockEffectiveIntervalMs (backoff table mirrors scheduler.rs)', () => {
  it.each([
    [0, 1000],
    [1, 1000],
    [2, 1000],
    [3, 2000],
    [4, 4000],
    [5, 8000],
    [6, 8000], // capped at 8×
    [10, 8000],
  ])('failures=%i → %i ms (base 1000)', (failures, expected) => {
    expect(mockEffectiveIntervalMs(1000, failures)).toBe(expected);
  });
});

describe('completeMockJobRun', () => {
  it('success: updates the entry, dispatches job-status then repo-changed', () => {
    const events: string[] = [];
    jobStatusListeners.add((p: JobStatusChangedPayload) => {
      events.push(`job:${p.outcome}:${p.updatedRefs ?? 0}`);
    });
    repoChangedListeners.add((p: RepoChangedPayload) => events.push(`repo:${p.reason}`));
    completeMockJobRun(REPO_ID, 'autoFetch');
    expect(events).toEqual(['job:success:2', 'repo:fs']); // ordering matters
    const entry = seedJobStatuses(REPO_ID).find((s) => s.job === 'autoFetch');
    expect(entry).toMatchObject({
      lastOutcome: 'success',
      consecutiveFailures: 0,
      inBackoff: false,
      lastError: null,
      nextRunMs: null, // autoFetch disabled in defaults
    });
  });

  it('failure shim escalates to backoff at 3 and suppresses repo-changed', () => {
    window.localStorage.setItem('bonsaiMockJobFail', '1');
    const payloads: JobStatusChangedPayload[] = [];
    let repoChanges = 0;
    jobStatusListeners.add((p) => payloads.push(p));
    repoChangedListeners.add(() => (repoChanges += 1));
    completeMockJobRun(REPO_ID, 'autoFetch');
    completeMockJobRun(REPO_ID, 'autoFetch');
    completeMockJobRun(REPO_ID, 'autoFetch');
    expect(payloads.map((p) => p.consecutiveFailures)).toEqual([1, 2, 3]);
    expect(payloads.map((p) => p.inBackoff)).toEqual([false, false, true]);
    expect(payloads.map((p) => p.enteredBackoff)).toEqual([false, false, true]);
    expect(payloads.every((p) => p.outcome === 'failed')).toBe(true);
    expect(repoChanges).toBe(0);
    // A success resets the failure counter.
    window.localStorage.removeItem('bonsaiMockJobFail');
    completeMockJobRun(REPO_ID, 'autoFetch');
    const entry = seedJobStatuses(REPO_ID).find((s) => s.job === 'autoFetch');
    expect(entry).toMatchObject({ consecutiveFailures: 0, inBackoff: false });
  });

  it('healthRefresh success emits repo-changed; unknown repoId is a no-op', () => {
    let repoChanges = 0;
    repoChangedListeners.add(() => (repoChanges += 1));
    completeMockJobRun(REPO_ID, 'healthRefresh');
    expect(repoChanges).toBe(1);
    completeMockJobRun('/never/opened', 'healthRefresh');
    expect(repoChanges).toBe(1); // unchanged
  });
});

describe('applyMockJobTimers', () => {
  it('arms enabled jobs on the minutes-as-seconds shim; disabling clears them', () => {
    const runs: string[] = [];
    jobStatusListeners.add((p) => runs.push(p.job));
    const enabled: UiSettings = structuredClone(DEFAULT_UI_SETTINGS);
    enabled.autoFetch = { enabled: true, intervalMinutes: 2 };
    writeUiSettings(enabled); // completeMockJobRun reads settings from storage
    applyMockJobTimers(enabled);
    vi.advanceTimersByTime(2000);
    expect(runs).toEqual(['autoFetch']);
    vi.advanceTimersByTime(2000);
    expect(runs).toEqual(['autoFetch', 'autoFetch']);
    // Disable → timer cleared, no further runs.
    applyMockJobTimers(structuredClone(DEFAULT_UI_SETTINGS));
    vi.advanceTimersByTime(10_000);
    expect(runs).toHaveLength(2);
  });
});

describe('mcpStatusOf (events.ts)', () => {
  it('disabled: nulled endpoint fields; enabled: url/port/token + write tool count', () => {
    mockMcp.enabled = false;
    mockMcp.allowWrite = false;
    expect(mcpStatusOf()).toEqual({
      enabled: false,
      allowWrite: false,
      port: null,
      url: null,
      token: null,
      toolCount: 14,
    });
    mockMcp.enabled = true;
    mockMcp.allowWrite = true;
    try {
      expect(mcpStatusOf()).toMatchObject({
        enabled: true,
        allowWrite: true,
        port: 8765,
        url: 'http://127.0.0.1:8765/mcp',
        token: 'mock-token-abc123',
        toolCount: 34,
      });
    } finally {
      mockMcp.enabled = false;
      mockMcp.allowWrite = false;
    }
  });
});
