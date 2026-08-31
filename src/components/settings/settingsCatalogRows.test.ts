/**
 * P69 §4.3 rule 7 — UI §1.3's 60-row coverage table, pinned literally.
 *
 * Split out of `settingsCatalog.test.ts` purely for size: this file is the row
 * bookkeeping, that one is the catalog's own invariants. The DOM half of the
 * guard is a third file (`settingsCatalog.coverage.test.tsx`) added by the
 * increment that re-skins the first category.
 */
import { describe, expect, it } from 'vitest';
import { SETTINGS_INDEX, findSettingsRow } from './settingsCatalog';
import type { SettingsRowId } from './types';

/**
 * UI §1.3's coverage table, pinned literally: row number → the catalog entries
 * that carry it. Deleting a row from the catalog fails here even if it is also
 * deleted from the UI, which is the whole point.
 *
 * A row maps to SEVERAL ids when the table's "control after" column is a pair
 * (a switch plus its number) or a card of fields (#34).
 */
const COVERAGE: Readonly<Record<number, readonly SettingsRowId[]>> = {
  1: ['about.welcome-tour'],
  2: ['about.version'],
  3: ['about.check-updates'],
  4: ['about.auto-check-updates'],
  5: ['general.auto-fetch'],
  6: ['general.auto-fetch'],
  7: ['general.fetch-interval'],
  8: ['general.auto-refresh'],
  9: ['general.refresh-interval'],
  10: ['graph.node-size'],
  11: ['graph.row-height'],
  12: ['graph.lane-width'],
  13: ['graph.short-sha'],
  14: ['graph.author-name'],
  15: ['graph.date'],
  16: ['graph.date-basis'],
  17: ['graph.ahead-behind'],
  18: ['graph.signature-badge'],
  19: ['graph.compact-rows'],
  20: ['graph.pr-badges'],
  21: ['graph.ci-status'],
  22: ['appearance.theme'],
  23: ['appearance.file-lists'],
  24: ['appearance.panel-density'],
  25: ['appearance.panel-density'],
  26: ['git-config.run-hooks'],
  27: ['git-config.scope'],
  28: ['git-config.user-name'],
  29: ['git-config.user-email'],
  30: ['git-config.behaviour'],
  31: ['git-config.custom-keys'],
  32: ['general.terminal-command'],
  33: ['general.editor-command'],
  34: [
    'identities.profile-label',
    'identities.profile-name',
    'identities.profile-email',
    'identities.profile-signing-key',
    'identities.profile-color',
  ],
  35: ['identities.apply'],
  36: ['identities.delete'],
  37: ['identities.add'],
  38: ['identities.apply'],
  39: ['identities.add'],
  40: ['identities.profile-email', 'identities.apply'],
  41: ['ai.enabled'],
  42: ['ai.conflict-resolution'],
  43: ['ai.enabled'],
  44: ['ai.repository-access'],
  45: ['ai.stream-output'],
  46: ['ai.stream-partial'],
  47: ['ai.idle-timeout-enabled', 'ai.idle-timeout-secs'],
  48: ['ai.hard-cap-enabled', 'ai.hard-cap-secs'],
  49: ['ai.max-turns'],
  50: ['ai.budget-enabled', 'ai.budget-usd'],
  51: ['ai.bulk-batch-size'],
  52: ['ai.repository-access'],
  53: ['ai.mcp-enabled'],
  54: ['ai.mcp-allow-write'],
  55: ['ai.mcp-enabled'],
  56: ['ai.mcp-server-url'],
  57: ['ai.mcp-token'],
  58: ['ai.mcp-register-global'],
  59: ['ai.mcp-register-repo'],
  60: ['accounts.add'],
  // P80 §2b D1 — General → Committing: primary commit action segmented control.
  61: ['general.primary-commit-action'],
};

/**
 * Coverage rows that are NOT controls of their own — a section description, a
 * cross-reference note, a status line, a badge, an empty state, an inline
 * warning. UI §1.3 folds each into another row's help or chrome, so they map onto
 * an id another row already owns. Everything NOT listed here must own its ids
 * exclusively; that is what makes the "60 rows are covered" check structural
 * rather than a count that two duplicates could satisfy.
 */
const DISSOLVED_ROWS: ReadonlySet<number> = new Set([5, 25, 38, 39, 40, 43, 52, 55]);

describe('UI §1.3 coverage — all 61 rows, structurally', () => {
  it('maps exactly rows 1..61', () => {
    const rows = Object.keys(COVERAGE)
      .map(Number)
      .sort((a, b) => a - b);
    expect(rows).toEqual(Array.from({ length: 61 }, (_, i) => i + 1));
  });

  it('names only real entries, and every entry is claimed by some row', () => {
    const claimed = new Set<SettingsRowId>();
    for (const [row, ids] of Object.entries(COVERAGE)) {
      expect(ids.length, `row ${row} claims nothing`).toBeGreaterThan(0);
      for (const id of ids) {
        expect(findSettingsRow(id), `row ${row} → unknown id ${id}`).toBeDefined();
        claimed.add(id);
      }
    }
    // Both directions: no uncovered row, and no phantom entry nobody asked for.
    expect([...claimed].sort()).toEqual(SETTINGS_INDEX.map((e) => e.id).sort());
  });

  it('gives every real control its OWN entry — only dissolved rows may share', () => {
    const owner = new Map<SettingsRowId, number>();
    for (const [row, ids] of Object.entries(COVERAGE)) {
      if (DISSOLVED_ROWS.has(Number(row))) continue;
      for (const id of ids) {
        expect(owner.has(id), `${id} is claimed by rows ${owner.get(id)} and ${row}`).toBe(false);
        owner.set(id, Number(row));
      }
    }
    // 61 rows − 8 dissolved, expanded by the pair/card rows.
    expect(owner.size).toBe(SETTINGS_INDEX.length);
    // A dissolved row must fold INTO a row that really exists.
    for (const row of DISSOLVED_ROWS) {
      for (const id of COVERAGE[row]) expect(owner.has(id), `row ${row} → ${id}`).toBe(true);
    }
  });
});

/**
 * Amendment A (AM-1) — the repeated-row invariants, pure data.
 *
 * They live here rather than in `settingsCatalog.test.ts` because that file is
 * near the size limit and this is row bookkeeping, not catalog shape.
 */
describe('repeated rows (AM-1)', () => {
  it("repeats:'perProfile' ⟺ requires:'profile', over the whole index", () => {
    for (const entry of SETTINGS_INDEX) {
      expect(entry.repeats === 'perProfile', entry.id).toBe(entry.requires === 'profile');
    }
  });

  it('only the identities pane may repeat a row', () => {
    for (const entry of SETTINGS_INDEX) {
      if (entry.repeats === undefined) continue;
      expect(entry.category, entry.id).toBe('identities');
    }
  });

  it('a repeated row never carries a reset descriptor', () => {
    // Asserted against `repeats` directly (not via the identities-wide ban) so a
    // future repeated row in another category inherits the rule.
    for (const entry of SETTINGS_INDEX) {
      if (entry.repeats === undefined) continue;
      expect(entry.reset, entry.id).toBeUndefined();
    }
  });
});
