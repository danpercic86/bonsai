/**
 * P69 §4.3 — the anti-drift guard, pure-data half (no DOM).
 *
 * What this protects: search can only find a row that is in the catalog, and the
 * catalog can only be trusted if it matches what is actually rendered. This file
 * pins everything checkable WITHOUT rendering. The DOM half (per-category
 * set-equality over `data-setting-id`) lives in `settingsCatalog.coverage.test.tsx`
 * and owns the `MIGRATED`/`PENDING` partition (Amendment A, AM-5) — one source of
 * truth per check, and headroom kept in this file.
 */
import { describe, expect, it } from 'vitest';
import type { UiSettings } from '../../ipc/types';
import { DEFAULT_UI_SETTINGS, cloneDefaultUiSettings } from '../../settings/defaults';
import {
  SETTINGS_CATEGORIES,
  SETTINGS_INDEX,
  findSettingsRow,
  formatDefaultLabel,
  searchSettings,
} from './settingsCatalog';
import { REQUIREMENT_PREDICATES, type SettingsAvailability } from './settingsAvailability';
import type { SettingsCategoryId, SettingsIndexEntry, SettingsRowRequirement } from './types';

const CATEGORY_IDS: readonly SettingsCategoryId[] = [
  'general',
  'appearance',
  'graph',
  'ai',
  'identities',
  'accounts',
  'git-config',
  'about',
];

/** Values that differ from the defaults in every field any `reset` touches. */
const MUTATED: UiSettings = {
  ...cloneDefaultUiSettings(),
  primaryCommitAction: 'commitPush',
  autoFetch: { enabled: true, intervalMinutes: 42 },
  healthRefresh: { enabled: true, intervalMinutes: 7 },
  graph: {
    avatarRadius: 6,
    rowHeight: 24,
    laneWidth: 28,
    showSha: false,
    showAuthor: true,
    showDate: false,
    dateBasis: 'committer',
    showAheadBehind: false,
    compact: true,
    showSignatureBadge: false,
    showPrBadge: true,
    showCiStatus: true,
  },
  terminalCommand: 'wt.exe {path}',
  editorCommand: 'code {path}',
  aiEnabled: false,
  aiConflictAutonomy: 'autoResolve',
  aiConflictTools: 'none',
  aiStreamLog: false,
  aiIncludePartialMessages: true,
  aiMaxTurns: 9,
  aiBulkMaxBytes: 100_000,
  autoCheckUpdates: true,
};

const withReset = SETTINGS_INDEX.filter((e) => e.reset !== undefined);
/** The one leaf a reset is expected to move, as `key` or `key.field`. Declared,
 *  not derived — deriving it from the patch would only make the guard agree with
 *  whatever the patch happens to do. */
const RESET_LEAVES: Readonly<Record<string, string>> = {
  'general.auto-fetch': 'autoFetch.enabled',
  'general.fetch-interval': 'autoFetch.intervalMinutes',
  'general.auto-refresh': 'healthRefresh.enabled',
  'general.refresh-interval': 'healthRefresh.intervalMinutes',
  'general.terminal-command': 'terminalCommand',
  'general.editor-command': 'editorCommand',
  'general.primary-commit-action': 'primaryCommitAction',
  'graph.node-size': 'graph.avatarRadius',
  'graph.row-height': 'graph.rowHeight',
  'graph.lane-width': 'graph.laneWidth',
  'graph.compact-rows': 'graph.compact',
  'graph.short-sha': 'graph.showSha',
  'graph.author-name': 'graph.showAuthor',
  'graph.date': 'graph.showDate',
  'graph.date-basis': 'graph.dateBasis',
  'graph.ahead-behind': 'graph.showAheadBehind',
  'graph.signature-badge': 'graph.showSignatureBadge',
  'graph.pr-badges': 'graph.showPrBadge',
  'graph.ci-status': 'graph.showCiStatus',
  'ai.conflict-resolution': 'aiConflictAutonomy',
  'ai.repository-access': 'aiConflictTools',
  'ai.stream-output': 'aiStreamLog',
  'ai.stream-partial': 'aiIncludePartialMessages',
  'ai.max-turns': 'aiMaxTurns',
  'ai.bulk-batch-size': 'aiBulkMaxBytes',
  'about.auto-check-updates': 'autoCheckUpdates',
};

/**
 * The six ↺ labels that are FORMATTED rather than the raw value (five distinct
 * strings), each pinned together with the raw default it claims to name. Changing
 * either side alone is copy drift the user reads in "Reset to default (…)".
 */
const FORMATTED_DEFAULT_LABELS: Readonly<Record<string, readonly [string, unknown]>> = {
  'general.terminal-command': ['auto-detect', ''],
  'general.editor-command': ['auto-detect', ''],
  'general.primary-commit-action': ['Commit', 'commit'],
  'graph.date-basis': ['Author', 'author'],
  'ai.conflict-resolution': ['Propose & review', 'proposeReview'],
  'ai.repository-access': ['Read-only', 'readOnly'],
  'ai.bulk-batch-size': ['400 KB', 400_000],
};

function leafOf(entry: SettingsIndexEntry): string {
  const leaf = RESET_LEAVES[entry.id];
  if (leaf === undefined) throw new Error(`no expected leaf declared for ${entry.id}`);
  return leaf;
}

/** Every leaf that really differs between `before` and `after`, as `key`/`key.field`. */
function changedLeaves(before: UiSettings, after: UiSettings, key: keyof UiSettings): string[] {
  const b: unknown = before[key];
  const a: unknown = after[key];
  if (typeof b !== 'object' || b === null || typeof a !== 'object' || a === null) {
    return a === b ? [] : [key];
  }
  const bo = b as Record<string, unknown>;
  const ao = a as Record<string, unknown>;
  expect(Object.keys(ao).sort(), `${key} gained or lost fields`).toEqual(Object.keys(bo).sort());
  return Object.keys(bo)
    .filter((f) => ao[f] !== bo[f])
    .map((f) => `${key}.${f}`);
}

/** Reads a declared leaf out of a settings object. */
function leafValue(values: UiSettings, leaf: string): unknown {
  const [key, field] = leaf.split('.');
  const top = values[key as keyof UiSettings];
  if (field === undefined) return top;
  return (top as unknown as Record<string, unknown>)[field];
}

function applyPatch(values: UiSettings, patch: object): UiSettings {
  return { ...values, ...patch };
}

describe('catalog identity and shape', () => {
  it('has unique, well-formed ids naming a real category', () => {
    const ids = SETTINGS_INDEX.map((e) => e.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const entry of SETTINGS_INDEX) {
      const dot = entry.id.indexOf('.');
      expect(dot, `${entry.id} must be \`category.slug\``).toBeGreaterThan(0);
      expect(entry.id.slice(0, dot)).toBe(entry.category);
      expect(entry.id.slice(dot + 1)).toMatch(/^[a-z0-9]+(?:-[a-z0-9]+)*$/);
      expect(CATEGORY_IDS).toContain(entry.category);
    }
  });

  it('lists every category exactly once, in rail order, and nothing else', () => {
    expect(SETTINGS_CATEGORIES.map((c) => c.id)).toEqual(CATEGORY_IDS);
    for (const c of SETTINGS_CATEGORIES) {
      expect(c.label.length).toBeGreaterThan(0);
      expect(c.subtitle.length).toBeGreaterThan(0);
    }
    // Only Git config carries the repo pill; the two hairlines fence it (UI §1.1).
    expect(SETTINGS_CATEGORIES.filter((c) => c.pill !== undefined).map((c) => c.id)).toEqual([
      'git-config',
    ]);
    expect(SETTINGS_CATEGORIES.filter((c) => c.dividerBefore === true).map((c) => c.id)).toEqual([
      'git-config',
      'about',
    ]);
    // Every category actually has rows — an empty pane is a dead rail item.
    for (const c of SETTINGS_CATEGORIES) {
      expect(SETTINGS_INDEX.filter((e) => e.category === c.id).length).toBeGreaterThan(0);
    }
  });

  it('has non-empty labels, groups and help, unique per category (the two-Interval guard)', () => {
    const seen = new Set<string>();
    for (const entry of SETTINGS_INDEX) {
      expect(entry.label.trim()).not.toBe('');
      expect(entry.group.trim()).not.toBe('');
      expect(entry.help?.trim() ?? 'x').not.toBe('');
      const key = `${entry.category}::${entry.label}`;
      expect(seen.has(key), `duplicate label in ${entry.category}: ${entry.label}`).toBe(false);
      seen.add(key);
    }
  });

  it('has clean keywords: lowercase, single-spaced, no duplicates, nothing shorter than 2 chars', () => {
    for (const entry of SETTINGS_INDEX) {
      if (entry.keywords === undefined) continue;
      expect(entry.keywords).toBe(entry.keywords.toLowerCase());
      expect(entry.keywords).not.toMatch(/\s\s|^\s|\s$/);
      const terms = entry.keywords.split(' ');
      expect(new Set(terms).size, `duplicate keyword in ${entry.id}`).toBe(terms.length);
      for (const term of terms) {
        expect(term.length, `${entry.id}: "${term}"`).toBeGreaterThanOrEqual(2);
        expect(term).toMatch(/^[a-z0-9][a-z0-9-]*$/);
      }
    }
  });

  it('resolves ids through findSettingsRow and nothing else', () => {
    expect(findSettingsRow('graph.row-height')?.label).toBe('Row height');
    expect(findSettingsRow('graph.rowheight')).toBeUndefined();
    for (const entry of SETTINGS_INDEX) expect(findSettingsRow(entry.id)).toBe(entry);
  });
});

/** Everything renderable: a repo is open, AI is on, MCP is running, profiles exist. */
const ALL: SettingsAvailability = {
  repoPath: '/repo',
  aiEnabled: true,
  aiConsented: true,
  mcpStatus: { enabled: true },
  profiles: ['p1'],
};

/** Nothing conditional renders: no repo, AI off, MCP stopped, no profiles. */
const NONE: SettingsAvailability = {
  repoPath: null,
  aiEnabled: false,
  aiConsented: false,
  mcpStatus: { enabled: false },
  profiles: [],
};

describe('search', () => {
  it('returns nothing for an empty or whitespace query', () => {
    expect(searchSettings('', ALL)).toEqual([]);
    expect(searchSettings('   ', ALL)).toEqual([]);
  });

  it('ANDs the terms and is case-insensitive', () => {
    const ids = searchSettings('graph row', ALL).map((e) => e.id);
    expect(ids).toContain('graph.row-height');
    expect(ids).not.toContain('general.fetch-interval');
    expect(searchSettings('GRAPH Row', ALL).map((e) => e.id)).toEqual(ids);
    // A term that matches nothing kills the whole result set.
    expect(searchSettings('graph row zzzz', ALL)).toEqual([]);
  });

  it('matches on keywords and help, not just the label', () => {
    expect(searchSettings('husky', ALL).map((e) => e.id)).toEqual(['git-config.run-hooks']);
    expect(searchSettings('colour', ALL).map((e) => e.id)).toEqual(['appearance.theme']);
    expect(searchSettings('upstream', ALL).map((e) => e.id)).toContain('graph.ahead-behind');
  });

  it('finds every row by its own label', () => {
    for (const entry of SETTINGS_INDEX) {
      expect(searchSettings(entry.label, ALL).map((e) => e.id)).toContain(entry.id);
    }
  });
});

describe('search — availability (P69k review A3)', () => {
  it('drops a row whose `requires` fails, so no result block can render empty', () => {
    // `husky` is only in `git-config.run-hooks`, which requires a repo.
    expect(searchSettings('husky', NONE)).toEqual([]);
    // `bearer` is only in the MCP token row, which requires a running server.
    expect(searchSettings('bearer', ALL).map((e) => e.id)).toEqual(['ai.mcp-token']);
    expect(searchSettings('bearer', NONE)).toEqual([]);
    // `nickname` is only in the per-profile label row.
    expect(searchSettings('nickname', ALL).map((e) => e.id)).toEqual(['identities.profile-label']);
    expect(searchSettings('nickname', NONE)).toEqual([]);
  });

  it('never drops an unconditional row', () => {
    const unconditional = SETTINGS_INDEX.filter((e) => e.requires === undefined);
    expect(unconditional.length).toBeGreaterThan(0);
    for (const entry of unconditional) {
      expect(searchSettings(entry.label, NONE).map((e) => e.id), entry.id).toContain(entry.id);
    }
  });

  it('implements every member of the requirement union', () => {
    const requirements: readonly SettingsRowRequirement[] = [
      'repo',
      'aiActive',
      'mcpRunning',
      'mcpStopped',
      'profile',
    ];
    for (const requirement of requirements) {
      expect(typeof REQUIREMENT_PREDICATES[requirement], requirement).toBe('function');
    }
    expect(Object.keys(REQUIREMENT_PREDICATES).sort()).toEqual([...requirements].sort());
    // The two MCP predicates are exhaustive and mutually exclusive by construction.
    for (const availability of [ALL, NONE, { ...ALL, mcpStatus: null }]) {
      expect(REQUIREMENT_PREDICATES.mcpRunning(availability)).toBe(
        !REQUIREMENT_PREDICATES.mcpStopped(availability),
      );
    }
  });
});

describe('reset descriptors (§3.4)', () => {
  it('agree with the defaults module and patch only real keys', () => {
    expect(withReset.length).toBeGreaterThan(0);
    const defaultKeys = new Set(Object.keys(DEFAULT_UI_SETTINGS));
    for (const entry of withReset) {
      const reset = entry.reset;
      if (reset === undefined) throw new Error('unreachable');
      expect(formatDefaultLabel(entry).trim(), entry.id).not.toBe('');
      expect(reset.isDefault(DEFAULT_UI_SETTINGS, DEFAULT_UI_SETTINGS), entry.id).toBe(true);
      const patch = reset.patch(DEFAULT_UI_SETTINGS, DEFAULT_UI_SETTINGS);
      const keys = Object.keys(patch);
      expect(keys.length, entry.id).toBe(1);
      expect(defaultKeys.has(keys[0]), `${entry.id} patches unknown key ${keys[0]}`).toBe(true);
      // Applied to the defaults it is a no-op — nothing else moves.
      expect(applyPatch(DEFAULT_UI_SETTINGS, patch)).toEqual(DEFAULT_UI_SETTINGS);
    }
  });

  it('each detect their own off-default value and restore exactly it', () => {
    const signatures = new Set<string>();
    for (const entry of withReset) {
      const reset = entry.reset;
      if (reset === undefined) throw new Error('unreachable');
      expect(reset.isDefault(MUTATED, DEFAULT_UI_SETTINGS), `${entry.id} reads no changed field`).toBe(
        false,
      );
      const patch = reset.patch(MUTATED, DEFAULT_UI_SETTINGS);
      // Two rows resetting the SAME field is the copy-paste bug this catches.
      const signature = JSON.stringify(patch);
      expect(signatures.has(signature), `${entry.id} duplicates another row's reset`).toBe(false);
      signatures.add(signature);
      const restored = applyPatch(MUTATED, patch);
      expect(reset.isDefault(restored, DEFAULT_UI_SETTINGS), entry.id).toBe(true);
      // Every OTHER top-level key keeps its mutated value.
      const key = Object.keys(patch)[0] as keyof UiSettings;
      for (const other of Object.keys(MUTATED) as (keyof UiSettings)[]) {
        if (other === key) continue;
        expect({ [other]: restored[other] }).toEqual({ [other]: MUTATED[other] });
      }
      // ...and exactly ONE LEAF moved. This is the guard for the destructive bug
      // `reset.ts` warns about: spreading `d[parent]` instead of `c[parent]` would
      // reset all twelve graph prefs while isDefault, the no-op check and the
      // top-level loop above all still passed.
      expect(changedLeaves(MUTATED, restored, key), `${entry.id} moved the wrong leaves`).toEqual([
        leafOf(entry),
      ]);
    }
  });

  it('name the actual default in `defaultLabel` — the ↺ title is user-visible copy', () => {
    for (const entry of withReset) {
      const leaf = leafOf(entry);
      const value = leafValue(DEFAULT_UI_SETTINGS, leaf);
      const label = formatDefaultLabel(entry);
      const formatted = FORMATTED_DEFAULT_LABELS[entry.id];
      if (formatted !== undefined) {
        expect(label, entry.id).toBe(formatted[0]);
        expect({ [leaf]: value }, `${entry.id}: label "${label}" names`).toEqual({
          [leaf]: formatted[1],
        });
      } else if (typeof value === 'boolean') {
        expect(label, entry.id).toBe(value ? 'On' : 'Off');
      } else if (typeof value === 'number') {
        expect(label, entry.id).toBe(String(value));
      } else {
        throw new Error(`${entry.id}: default needs a FORMATTED_DEFAULT_LABELS entry`);
      }
    }
  });

  it('omits the ↺ everywhere §3.4 says it must be absent', () => {
    const noReset = SETTINGS_INDEX.filter((e) => e.reset === undefined).map((e) => e.id);
    // The three P68 mode sentinels and their switches, plus the AI master switch:
    // `resetRow` patches `{aiEnabled}` straight through `onChange`, bypassing the
    // consent-aware `setAiEnabled`, so a ↺ would turn AI on without the consent
    // dialog. Pinned here so it cannot be re-added with the suite green (P69j-1).
    for (const id of [
      'ai.enabled',
      'ai.idle-timeout-enabled',
      'ai.idle-timeout-secs',
      'ai.hard-cap-enabled',
      'ai.hard-cap-secs',
      'ai.budget-enabled',
      'ai.budget-usd',
    ]) {
      expect(noReset, id).toContain(id);
    }
    // Buttons, read-only rows, identity fields and Git config keys.
    for (const entry of SETTINGS_INDEX) {
      if (
        entry.control === 'button' ||
        entry.control === 'readonly' ||
        // AM-2: an aggregate block has no single value to restore.
        entry.control === 'group'
      ) {
        expect(entry.reset, entry.id).toBeUndefined();
      }
      if (entry.category === 'git-config' || entry.category === 'identities') {
        expect(entry.reset, entry.id).toBeUndefined();
      }
    }
  });
});

/**
 * Amendment A (AM-5): the DOM half now lives in
 * `settingsCatalog.coverage.test.tsx`, which owns the `MIGRATED`/`PENDING`
 * partition and the tripwire. What stays here is the pure-data half — the
 * per-category row set — so there is exactly one source of truth per check.
 */
describe('expected row sets, per category', () => {
  for (const category of SETTINGS_CATEGORIES) {
    const entries = SETTINGS_INDEX.filter((e) => e.category === category.id);

    it(`${category.id}: has a coherent expected row set`, () => {
      const ids = entries.map((e) => e.id);
      expect(new Set(ids).size).toBe(ids.length);
      // Rows allowed to be missing from the minimal fixture are EXACTLY the ones
      // the catalog declares conditional; everything else must always render.
      const conditional = entries.filter((e) => e.requires !== undefined).map((e) => e.id);
      const always = entries.filter((e) => e.requires === undefined).map((e) => e.id);
      // `git-config` is the one wholly-gated pane: with no repo open it renders
      // `SettingsEmpty` instead of any row (UI §1.2). Every other category must
      // put something on screen unconditionally.
      if (category.id !== 'git-config') {
        expect(always.length, `${category.id} renders nothing unconditionally`).toBeGreaterThan(0);
      } else {
        expect(conditional.length).toBe(ids.length);
      }
    });
  }
});

describe('entry control kinds', () => {
  it('are all drawn from the declared union (guards a typo in a new row)', () => {
    const kinds = new Set<SettingsIndexEntry['control']>([
      'switch',
      'segmented',
      'radiogroup',
      'numberSlider',
      'text',
      'button',
      'readonly',
      'group',
    ]);
    for (const entry of SETTINGS_INDEX) expect(kinds.has(entry.control), entry.id).toBe(true);
  });

  it('only gate rows on a requirement that pane really has', () => {
    const allowed: Record<SettingsCategoryId, readonly string[]> = {
      general: [],
      appearance: [],
      graph: [],
      ai: ['aiActive', 'mcpRunning', 'mcpStopped'],
      identities: ['profile'],
      accounts: [],
      'git-config': ['repo'],
      about: [],
    };
    for (const entry of SETTINGS_INDEX) {
      if (entry.requires === undefined) continue;
      expect(allowed[entry.category], `${entry.id} requires ${entry.requires}`).toContain(
        entry.requires,
      );
    }
  });
});
