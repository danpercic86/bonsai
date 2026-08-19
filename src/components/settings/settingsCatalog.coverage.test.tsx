/**
 * P69 §4.3 as amended by `docs/contracts/P69-settings-shell-amendment-A.md` —
 * the DOM↔catalog anti-drift guard.
 *
 * It renders each MIGRATED category's pane against two fixtures and asserts
 * set-equality between what the catalog says the pane contains and what the DOM
 * actually stamps with `data-setting-id`, in BOTH directions. That is what stops
 * search from offering a dead result, a row from being renamed in one place only,
 * or a control from being born unsearchable.
 *
 * A category joins `MIGRATED` in the same increment that re-skins it. `PENDING`
 * must be `[]` before search ships (AM-5); the two lists and the tripwire are
 * deleted then.
 */
import { describe, expect, it, vi, afterEach } from 'vitest';
import { act, cleanup, render, screen, within } from '@testing-library/react';

import { SettingsPanel } from '../SettingsPanel';
import { CATEGORY_PAGES } from './categories';
import { MAXIMAL, MINIMAL, FIXTURE_CONFIG_VIEW, FIXTURE_PROFILES } from './coverageFixtures';
import { SETTINGS_CATEGORIES, SETTINGS_INDEX, findSettingsRow } from './settingsCatalog';
import { DEFAULT_UI_SETTINGS } from '../../settings/defaults';
import { mockIpc } from '../../ipc/mock';
import { resetEffectiveIdentityForTests } from '../../hooks/useEffectiveIdentity';
import type {
  SettingsCategoryId,
  SettingsControlKind,
  SettingsIndexEntry,
  SettingsRowRepeat,
  SettingsRowRequirement,
} from './types';
import type { UiSettings } from '../../ipc/types';

// ---------------------------------------------------------------- AM-5 partition

/** Re-skinned and guarded. */
const MIGRATED: readonly SettingsCategoryId[] = ['general', 'appearance', 'about', 'git-config'];

/** Still on legacy interiors — reachable from the rail, not yet catalog-shaped. */
const PENDING: readonly SettingsCategoryId[] = [
  'graph', // P69j
  'ai', // P69j
  'identities', // P69i
];

// ------------------------------------------------------------- AM-4a predicates

type Fixture = typeof MAXIMAL;

const REQUIREMENT_HOLDS: Record<SettingsRowRequirement, (fx: Fixture) => boolean> = {
  repo: (fx) => fx.repoPath !== null,
  aiActive: (fx) => fx.aiEnabled && fx.aiConsented,
  mcpRunning: (fx) => fx.mcpStatus?.enabled === true,
  mcpStopped: (fx) => fx.mcpStatus?.enabled !== true,
  profile: (fx) => fx.profiles.length > 0,
};

const REPEAT_INSTANCES: Record<SettingsRowRepeat, (fx: Fixture) => readonly string[]> = {
  perProfile: (fx) => fx.profiles.map((p) => p.id),
};

/**
 * §4.3's control-kind → ARIA role map.
 *
 * `readonly` and `group` are handled inline. The two exclusive-choice kinds map to
 * a `radiogroup` named by the row label, whose RADIOS are named by their own
 * option text — the row label is the group's name, never a radio's.
 */
const ROLE_FOR: Record<Exclude<SettingsControlKind, 'readonly' | 'group'>, string> = {
  switch: 'checkbox',
  segmented: 'radiogroup',
  radiogroup: 'radiogroup',
  numberSlider: 'spinbutton',
  text: 'textbox',
  button: 'button',
};

/** The `UiSettings` view a reset descriptor compares against, per fixture. */
function valuesOf(fx: Fixture): UiSettings {
  return { ...DEFAULT_UI_SETTINGS, ...fx, ...fx.aiRun };
}

/**
 * AM-4a: accessible name of a `'group'` row. Resolved by hand — Testing Library's
 * `within(el).getByRole('group', …)` searches DESCENDANTS and would never see the
 * row element itself.
 */
function accName(el: HTMLElement): string | null {
  const direct = el.getAttribute('aria-label');
  if (direct !== null && direct.trim() !== '') return direct.trim();
  const ids = (el.getAttribute('aria-labelledby') ?? '').split(/\s+/).filter((i) => i !== '');
  if (ids.length === 0) return null;
  const text = ids
    .map((i) => document.getElementById(i)?.textContent?.trim() ?? '')
    .filter((t) => t !== '')
    .join(' ');
  return text === '' ? null : text;
}

/** Flush the pane's mount IPC (Git config reads a `ConfigView` before it can
 *  render a row). A macrotask turn drains the whole promise chain, so the guard
 *  never asserts against a half-rendered pane. */
async function settle(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => {
      setTimeout(resolve, 0);
    });
  });
}

function renderPane(category: SettingsCategoryId, fx: Fixture): HTMLElement {
  render(
    <SettingsPanel
      open
      initialCategory={category}
      onClose={vi.fn()}
      requestSeq={0}
      onChange={vi.fn()}
      onToggleTheme={vi.fn()}
      onToggleListView={vi.fn()}
      onRequestEnableAi={vi.fn()}
      onSetMcpEnabled={vi.fn()}
      onRequestEnableMcp={vi.fn()}
      onSetMcpAllowWrite={vi.fn()}
      onRequestEnableMcpWrite={vi.fn()}
      onRegisterMcp={vi.fn(async () => {})}
      onShowOnboarding={vi.fn()}
      onOpenRepository={vi.fn()}
      onCheckUpdate={vi.fn()}
      onOpenUpdateDialog={vi.fn()}
      {...fx}
    />,
  );
  return screen.getByRole('tabpanel');
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  resetEffectiveIdentityForTests();
});

describe('MIGRATED / PENDING partition (AM-5)', () => {
  it('covers all seven categories exactly once', () => {
    const all = SETTINGS_CATEGORIES.map((c) => c.id).sort();
    expect([...MIGRATED, ...PENDING].sort()).toEqual(all);
    expect(MIGRATED.filter((id) => PENDING.includes(id))).toEqual([]);
  });

  it('every category still has a page component (tripwire)', () => {
    // A migrated id with no renderer, or a renderer quietly deleted, must FAIL
    // rather than silently skip the strongest check in P69.
    for (const id of [...MIGRATED, ...PENDING]) {
      expect(CATEGORY_PAGES[id], `${id} has no page component`).toBeDefined();
    }
  });
});

for (const [name, fx] of [
  ['maximal', MAXIMAL],
  ['minimal', MINIMAL],
] as const) {
  describe(`DOM↔catalog guard — ${name} fixture`, () => {
    for (const category of SETTINGS_CATEGORIES) {
      const c = category.id;
      if (!MIGRATED.includes(c)) {
        it.skip(`${c}: pending migration`, () => {});
        continue;
      }

      it(`${c}: every stamped row is catalogued, named, and gated correctly`, async () => {
        // The maximal fixture opens a repo; the Git-config surface it can reach
        // must never hit real IPC (and P69h needs the curated/custom keys here).
        vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
        const pane = renderPane(c, fx);
        await settle();

        const entries = SETTINGS_INDEX.filter((e) => e.category === c);
        const expected = entries.filter(
          (e) => e.requires === undefined || REQUIREMENT_HOLDS[e.requires](fx),
        );

        const stamped = [...pane.querySelectorAll<HTMLElement>('[data-setting-id]')];

        // (1) No nesting — a stamped row inside a stamped row breaks set-equality
        // and would make a search result edit the wrong control.
        expect(
          pane.querySelectorAll('[data-setting-id] [data-setting-id]').length,
          `settings drift [${c}]: a stamped row is nested inside another stamped row.`,
        ).toBe(0);

        // (2) Instance bookkeeping.
        const byId = new Map<string, HTMLElement[]>();
        for (const el of stamped) {
          const id = el.dataset.settingId ?? '';
          byId.set(id, [...(byId.get(id) ?? []), el]);
        }

        for (const [id, els] of byId) {
          const entry = findSettingsRow(id);
          expect(
            entry,
            `settings drift [${c}]: rendered but not in the catalog — data-setting-id="${id}". Add an entry to catalog/ or remove the stamp.`,
          ).toBeDefined();
          if (entry === undefined) continue;
          expect(
            entry.category,
            `settings drift [${c}]: "${id}" is rendered on the ${c} pane but the catalog files it under ${entry.category}. Move the row or fix its category.`,
          ).toBe(c);

          if (entry.repeats === undefined) {
            expect(
              els.length,
              `settings drift [${c}]: "${id}" rendered ${els.length}× but is not repeats:'perProfile'. A row rendered twice is a bug; if it is genuinely repeated, declare repeats.`,
            ).toBe(1);
            expect(
              els[0].dataset.profileId,
              `settings drift [${c}]: "${id}" carries data-profile-id but declares no repeats.`,
            ).toBeUndefined();
          } else {
            const got = els.map((el) => el.dataset.profileId);
            const want = [...REPEAT_INSTANCES[entry.repeats](fx)].sort();
            expect(
              got.every((g) => g !== undefined) && new Set(got).size === got.length,
              `settings drift [${c}]: "${id}" has instances with missing/duplicate data-profile-id: ${JSON.stringify(got)}.`,
            ).toBe(true);
            expect(
              [...got].sort(),
              `settings drift [${c}]: "${id}" rendered for profiles ${JSON.stringify(got)} but the fixture has ${JSON.stringify(want)}. One card is dropping the row.`,
            ).toEqual(want);
            for (const el of els) {
              expect(
                el.closest('[data-profile-id]'),
                `settings drift [${c}]: "${id}" inherits data-profile-id from an ancestor instead of carrying it.`,
              ).toBe(el);
            }
          }
        }

        // (3) Set-equality, BOTH directions.
        expect(
          [...byId.keys()].sort(),
          `settings drift [${c}]: catalog and DOM disagree. In the catalog but not rendered, or rendered but not catalogued — see the per-id failures above.`,
        ).toEqual(expected.map((e) => e.id).sort());

        // (4) Per-instance shape and naming.
        for (const entry of expected) {
          for (const el of byId.get(entry.id) ?? []) {
            assertRowShape(c, entry, el, fx);
          }
        }

        // (5) The wholly-gated pane: no rows at all means the pane owes the user
        // an explanation, not a blank column (AM-4b FAIL-N / UI §1.2).
        if (expected.length === 0) {
          expect(
            stamped.length,
            `settings drift [${c}]: every row is gated off in this fixture, so the pane must render no rows; found ${stamped.length}.`,
          ).toBe(0);
          expect(
            pane.querySelector('.settings-empty'),
            `settings drift [${c}]: every row is gated off in this fixture, so the pane must render a SettingsEmpty block explaining why.`,
          ).not.toBeNull();
        }
      });
    }
  });
}

function assertRowShape(
  c: SettingsCategoryId,
  entry: SettingsIndexEntry,
  el: HTMLElement,
  fx: Fixture,
): void {
  // The row's GROUP header. Unlike the label — which `SettingsRow` reads straight
  // out of the catalog, so it cannot diverge — the group title is a string literal
  // at each `<SettingsGroup title=…>` call site. Search groups its results by
  // `entry.group`, so a divergence files a row under a header the pane never
  // shows. This is the one remaining hand-copied string, so it is the one that
  // needs checking.
  const groupEl = el.closest('.settings-group');
  if (groupEl === null) {
    // UI §1.1 puts exactly ONE row outside the groups: the Git-config scope
    // switch, which lives in the pane header because it retargets the whole
    // pane. Its catalog `group` still files it under "Scope" for search
    // results. Anything else with no group header is drift, so the exemption is
    // asserted rather than assumed.
    expect(
      el.closest('.settings-pane-header'),
      `settings drift [${c}]: "${entry.id}" is neither inside a .settings-group nor in the pane header, so it has no visible group header.`,
    ).not.toBeNull();
    // Scoped by row IDENTITY, not merely by DOM position: the pane header is not
    // a general escape hatch from the group rule, it is the home of this one row.
    expect(
      entry.id,
      `settings drift [${c}]: "${entry.id}" renders in the pane header, but only "git-config.scope" may (UI §1.1). Put it in a group.`,
    ).toBe('git-config.scope');
  } else {
    const groupTitle = groupEl.querySelector('.settings-group-title')?.textContent?.trim() ?? null;
    expect(
      groupTitle,
      `settings drift [${c}]: "${entry.id}" renders under the group header "${groupTitle ?? '(none)'}" but the catalog files it under "${entry.group}". Search would group this row under a heading the pane never shows.`,
    ).toBe(entry.group);
  }

  if (entry.control === 'group') {
    expect(el.getAttribute('role'), `settings drift [${c}]: "${entry.id}" is not role="group".`).toBe(
      'group',
    );
    expect(
      accName(el),
      `settings drift [${c}]: group row "${entry.id}" has accessible name "${accName(el) ?? '(none)'}", expected "${entry.label}". The heading and the catalog label must match byte-for-byte.`,
    ).toBe(entry.label);
    expect(
      el.querySelectorAll('input,select,button,textarea').length,
      `settings drift [${c}]: group row "${entry.id}" contains no controls in the ${JSON.stringify(fx.repoPath)} fixture — the fixture no longer exercises this block, so the guard is checking nothing.`,
    ).toBeGreaterThan(0);
  } else {
    expect(
      el.querySelector('[data-setting-control]'),
      `settings drift [${c}]: "${entry.id}" has no [data-setting-control] descendant.`,
    ).not.toBeNull();

    if (entry.control === 'readonly') {
      // No role to query: pin the row's VISIBLE label instead, so a rename here
      // still has to be mirrored in the catalog.
      expect(
        el.textContent ?? '',
        `settings drift [${c}]: read-only row "${entry.id}" does not show its label "${entry.label}".`,
      ).toContain(entry.label);
    } else {
      const role = ROLE_FOR[entry.control];
      expect(
        () => within(el).getByRole(role, { name: entry.label }),
        `settings drift [${c}]: "${entry.id}" — no ${role} named "${entry.label}" inside the row. Catalog label and rendered accessible name disagree, so search would match text the user cannot see.`,
      ).not.toThrow();
      if (entry.control === 'numberSlider') {
        // The range twin is named by `NumberSlider`'s own `label` prop, a literal
        // at the call site — the one accessible name on a slider row that is NOT
        // derived from the catalog, and therefore the one that can drift.
        expect(
          () => within(el).getByRole('slider', { name: entry.label }),
          `settings drift [${c}]: "${entry.id}" — no slider named "${entry.label}". The NumberSlider \`label\` prop and the catalog label disagree.`,
        ).not.toThrow();
      }
      if (role === 'radiogroup') {
        // A named-but-empty radiogroup would sail through the check above.
        const group = within(el).getByRole('radiogroup', { name: entry.label });
        expect(
          within(group).getAllByRole('radio').length,
          `settings drift [${c}]: radiogroup "${entry.id}" contains no radios.`,
        ).toBeGreaterThan(0);
      }
    }
  }

  if (entry.reset !== undefined) {
    const present =
      within(el).queryByRole('button', { name: `Reset ${entry.label} to default` }) !== null;
    expect(
      present,
      `settings drift [${c}]: "${entry.id}" ↺ visibility disagrees with its reset descriptor.`,
    ).toBe(!entry.reset.isDefault(valuesOf(fx), DEFAULT_UI_SETTINGS));
  }
}

describe('the fixtures themselves stay honest', () => {
  it('the maximal fixture carries two profiles with stable ids', () => {
    expect(MAXIMAL.profiles.map((p) => p.id)).toEqual(['p-1', 'p-2']);
    expect(FIXTURE_PROFILES).toHaveLength(2);
  });

  it('the maximal Git config view carries a curated AND a custom key (AM-8)', () => {
    expect(FIXTURE_CONFIG_VIEW.curated.length).toBeGreaterThan(0);
    expect(FIXTURE_CONFIG_VIEW.advanced.length).toBeGreaterThan(0);
  });

  it('every resettable MIGRATED row is off-default in maximal and at-default in minimal', () => {
    // Without this the ↺ half of the guard could pass by never firing.
    const max = valuesOf(MAXIMAL);
    const min = valuesOf(MINIMAL);
    for (const entry of SETTINGS_INDEX) {
      if (entry.reset === undefined || !MIGRATED.includes(entry.category)) continue;
      expect(entry.reset.isDefault(max, DEFAULT_UI_SETTINGS), `${entry.id} in maximal`).toBe(false);
      expect(entry.reset.isDefault(min, DEFAULT_UI_SETTINGS), `${entry.id} in minimal`).toBe(true);
    }
  });
});
