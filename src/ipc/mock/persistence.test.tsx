/** T3.4 — persistence.ts: localStorage-backed recents / session / ui-settings.
 *  Round-trips plus the corrupt-storage matrix (garbage JSON, wrong shapes,
 *  partial objects, huge blobs, __proto__ pollution attempts) — every reader
 *  must degrade to defaults and never throw. jsdom (.tsx) for localStorage. */
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import {
  AUTO_FETCH_INTERVAL_MAX,
  AUTO_FETCH_INTERVAL_MIN,
  LANE_WIDTH_MAX,
  ROW_HEIGHT_MIN,
} from '../../settings/ranges';
import {
  DEFAULT_UI_SETTINGS,
  clampAutoFetch,
  clampGraphPrefs,
  clampPaneWidths,
  readRecents,
  readSession,
  readUiSettings,
  recordRecent,
  writeRecents,
  writeSession,
  writeUiSettings,
} from './persistence';
import type { UiSettings } from '../types';

const RECENTS_KEY = 'bonsai.mockRecents';
const SESSION_KEY = 'bonsai.mockSession';
const UI_KEY = 'bonsai.mockUiSettings';

beforeEach(() => window.localStorage.clear());
afterEach(() => window.localStorage.clear());

describe('recents', () => {
  it('missing storage → []', () => {
    expect(readRecents()).toEqual([]);
  });

  it('round-trips a valid list', () => {
    const list = [{ path: 'C:/repo', lastOpened: 123 }];
    writeRecents(list);
    expect(readRecents()).toEqual(list);
  });

  it('garbage JSON / non-array JSON → [] (no throw)', () => {
    window.localStorage.setItem(RECENTS_KEY, '{{{not json');
    expect(readRecents()).toEqual([]);
    window.localStorage.setItem(RECENTS_KEY, '{"path":"x"}');
    expect(readRecents()).toEqual([]);
    window.localStorage.setItem(RECENTS_KEY, '42');
    expect(readRecents()).toEqual([]);
  });

  it('filters malformed elements, keeps valid ones', () => {
    window.localStorage.setItem(
      RECENTS_KEY,
      JSON.stringify([
        { path: 'C:/good', lastOpened: 1 },
        { path: 42, lastOpened: 1 },
        { lastOpened: 2 },
        null,
        'str',
        { path: 'C:/also-good', lastOpened: '1' }, // lastOpened wrong type
      ]),
    );
    expect(readRecents()).toEqual([{ path: 'C:/good', lastOpened: 1 }]);
  });

  it('recordRecent upserts at front, dedupes case-insensitively, caps at 10', () => {
    for (let i = 0; i < 12; i++) recordRecent(`C:/repo-${i}`);
    let list = readRecents();
    expect(list).toHaveLength(10);
    expect(list[0].path).toBe('C:/repo-11');
    // Case-insensitive dedupe moves the entry to the front, no duplicate.
    recordRecent('c:/REPO-5');
    list = readRecents();
    expect(list).toHaveLength(10);
    expect(list[0].path).toBe('c:/REPO-5');
    expect(list.filter((r) => r.path.toLowerCase() === 'c:/repo-5')).toHaveLength(1);
  });
});

describe('session', () => {
  it('missing / garbage / wrong-shape → empty session', () => {
    expect(readSession()).toEqual({ openRepos: [], activeRepo: null });
    window.localStorage.setItem(SESSION_KEY, 'not-json{');
    expect(readSession()).toEqual({ openRepos: [], activeRepo: null });
    window.localStorage.setItem(SESSION_KEY, JSON.stringify({ openRepos: 'x', activeRepo: 7 }));
    expect(readSession()).toEqual({ openRepos: [], activeRepo: null });
  });

  it('round-trips and filters non-string openRepos elements', () => {
    writeSession({ openRepos: ['a', 'b'], activeRepo: 'b' });
    expect(readSession()).toEqual({ openRepos: ['a', 'b'], activeRepo: 'b' });
    window.localStorage.setItem(
      SESSION_KEY,
      JSON.stringify({ openRepos: ['a', 1, null, 'b'], activeRepo: 'b' }),
    );
    expect(readSession()).toEqual({ openRepos: ['a', 'b'], activeRepo: 'b' });
  });
});

describe('ui settings — corrupt-storage matrix', () => {
  it('missing → a fresh deep copy of the defaults', () => {
    const s = readUiSettings();
    expect(s).toEqual(DEFAULT_UI_SETTINGS);
    // Deep copy — mutating the result must not poison the module default.
    s.profiles.pop();
    expect(readUiSettings().profiles).toHaveLength(DEFAULT_UI_SETTINGS.profiles.length);
  });

  it('garbage JSON → defaults, no throw', () => {
    window.localStorage.setItem(UI_KEY, '<<<garbage>>>');
    expect(readUiSettings()).toEqual(DEFAULT_UI_SETTINGS);
  });

  it.each(['42', '"a string"', 'null', 'true', '[]'])(
    'valid JSON, wrong shape (%s) → defaults, no throw',
    (raw) => {
      window.localStorage.setItem(UI_KEY, raw);
      expect(readUiSettings()).toEqual(DEFAULT_UI_SETTINGS);
    },
  );

  it('partial object: present fields honored, missing fields default', () => {
    window.localStorage.setItem(
      UI_KEY,
      JSON.stringify({ theme: 'light', listView: 'flat', aiConsented: true }),
    );
    const s = readUiSettings();
    expect(s.theme).toBe('light');
    expect(s.listView).toBe('flat');
    expect(s.aiConsented).toBe(true);
    expect(s.paneWidths).toEqual(DEFAULT_UI_SETTINGS.paneWidths);
    expect(s.graph).toEqual(DEFAULT_UI_SETTINGS.graph);
    expect(s.profiles).toEqual(DEFAULT_UI_SETTINGS.profiles);
  });

  it('unknown enum values fall back (theme/listView/dateBasis/autonomy)', () => {
    window.localStorage.setItem(
      UI_KEY,
      JSON.stringify({
        theme: 'neon',
        listView: 'spiral',
        aiConflictAutonomy: 'yolo',
        graph: { dateBasis: 'lunar' },
      }),
    );
    const s = readUiSettings();
    expect(s.theme).toBe('dark');
    expect(s.listView).toBe('tree');
    expect(s.aiConflictAutonomy).toBe('proposeReview');
    expect(s.graph.dateBasis).toBe('author');
  });

  it('out-of-range numerics are clamped on read', () => {
    window.localStorage.setItem(
      UI_KEY,
      JSON.stringify({
        paneWidths: { sidebar: 5, rightPanel: 99_999 },
        autoFetch: { enabled: true, intervalMinutes: -3 },
        graph: { rowHeight: 1, laneWidth: 10_000 },
      }),
    );
    const s = readUiSettings();
    expect(s.paneWidths).toEqual({ sidebar: 180, rightPanel: 640 });
    expect(s.autoFetch).toEqual({ enabled: true, intervalMinutes: AUTO_FETCH_INTERVAL_MIN });
    expect(s.graph.rowHeight).toBe(ROW_HEIGHT_MIN);
    expect(s.graph.laneWidth).toBe(LANE_WIDTH_MAX);
  });

  it('__proto__ pollution attempt: no throw, no Object.prototype pollution', () => {
    window.localStorage.setItem(
      UI_KEY,
      '{"__proto__":{"polluted":true},"theme":"light","profiles":{"__proto__":[]}}',
    );
    const s = readUiSettings();
    expect(s.theme).toBe('light');
    expect(s.profiles).toEqual(DEFAULT_UI_SETTINGS.profiles); // non-array → defaults
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
    expect(Object.prototype).not.toHaveProperty('polluted');
  });

  it('huge blob: parses (or degrades) without throwing', () => {
    const huge = JSON.stringify({ theme: 'light', junk: 'x'.repeat(2_000_000) });
    window.localStorage.setItem(UI_KEY, huge);
    const s = readUiSettings();
    expect(s.theme).toBe('light');
    expect(s.autoFetch).toEqual(DEFAULT_UI_SETTINGS.autoFetch);
  });

  it('legacy graph object carrying dotRadius / missing new keys → per-field defaults', () => {
    window.localStorage.setItem(
      UI_KEY,
      JSON.stringify({ graph: { dotRadius: 4, rowHeight: 40 } }),
    );
    const g = readUiSettings().graph;
    expect(g.rowHeight).toBe(40);
    expect(g.avatarRadius).toBe(DEFAULT_UI_SETTINGS.graph.avatarRadius);
    expect(g.showSignatureBadge).toBe(true);
    expect(g.showPrBadge).toBe(false);
    expect(g.showCiStatus).toBe(false);
    expect(g).not.toHaveProperty('dotRadius');
  });

  it('profiles: legitimately-empty stays empty; all-corrupt falls back to seeds', () => {
    window.localStorage.setItem(UI_KEY, JSON.stringify({ profiles: [] }));
    expect(readUiSettings().profiles).toEqual([]);
    window.localStorage.setItem(UI_KEY, JSON.stringify({ profiles: [{ id: 1 }, null, 'x'] }));
    expect(readUiSettings().profiles).toEqual(DEFAULT_UI_SETTINGS.profiles);
  });

  it('full round-trip: write then read returns the same settings', () => {
    const custom: UiSettings = structuredClone(DEFAULT_UI_SETTINGS);
    custom.theme = 'light';
    custom.paneWidths = { sidebar: 300, rightPanel: 400 };
    custom.autoFetch = { enabled: true, intervalMinutes: 10 };
    custom.aiConsented = true;
    custom.onboardingSeen = true;
    custom.profiles = [
      { id: 'p1', label: 'L', userName: 'N', userEmail: 'e@x.dev', signingKey: null },
    ];
    writeUiSettings(custom);
    expect(readUiSettings()).toEqual(custom);
  });
});

describe('clamp helpers (pure)', () => {
  it('clampPaneWidths clamps both panes into range', () => {
    expect(clampPaneWidths({ sidebar: 0, rightPanel: 10_000 })).toEqual({
      sidebar: 180,
      rightPanel: 640,
    });
    expect(clampPaneWidths({ sidebar: 240, rightPanel: 380 })).toEqual({
      sidebar: 240,
      rightPanel: 380,
    });
  });

  it('clampAutoFetch clamps the interval, passes enabled through', () => {
    expect(clampAutoFetch({ enabled: true, intervalMinutes: 10_000 })).toEqual({
      enabled: true,
      intervalMinutes: AUTO_FETCH_INTERVAL_MAX,
    });
  });

  it('clampGraphPrefs clamps geometry, passes toggles through unclamped', () => {
    const g = clampGraphPrefs({
      ...DEFAULT_UI_SETTINGS.graph,
      laneWidth: 0,
      rowHeight: 9999,
      compact: true,
      showSha: false,
    });
    expect(g.laneWidth).toBeGreaterThan(0);
    expect(g.rowHeight).toBeLessThanOrEqual(9999);
    expect(g.compact).toBe(true);
    expect(g.showSha).toBe(false);
  });
});
