/**
 * P112 rule 6 — the CLASS guard: nothing may write `terminalTool` / `editorTool`
 * without going through the rule that governs them.
 *
 * Four review rounds each found a different control mutating the owed-adopt
 * state without going through `useExternalToolScan.changeTool` — the recovery
 * not retracting its own message, an unreachable epoch guard, an owed adopt
 * dying on remount, and each row's `↺`. Every one of them passed a review before
 * the next surfaced, which says the mechanism was right and its ENTRY POINTS
 * were never enumerated. This file enumerates them and fails when the set grows.
 *
 * Four prongs, because no single one of them sees the whole class:
 *
 *   1. the `routed` set is exactly the two picker rows;
 *   2. no catalog descriptor patches either key unless it is routed — evaluated
 *      at RUNTIME, which is the only way to see it: the ↺ defect was
 *      `resetKey('editorTool', …)` building `{ [key]: d[key] }` from a variable,
 *      and no source grep for `editorTool:` can find that;
 *   3. the pinned set of shipped files that so much as NAME either key;
 *   4. `adoptToolSelection` — the non-writing disk adopt — has exactly one
 *      production caller.
 *
 * What prong 3 does NOT catch, stated plainly rather than implied: a new writer
 * added INSIDE one of the pinned files (they are the state owner, the defaults
 * table, the mock backend and the fixtures — each of which legitimately names
 * the keys); a patch assembled from a computed key outside the catalog
 * (`change({ [k]: v })`); a whole-`UiSettings` spread into a patch; and anything
 * on the Rust side. Prong 2 covers the computed-key case for catalog resets, the
 * one place it has actually happened.
 */
import { describe, expect, it } from 'vitest';

import { DEFAULT_UI_SETTINGS } from '../../settings/defaults';
import { SETTINGS_INDEX } from './settingsCatalog';
import type { UiSettings } from '../../ipc';

/** The two keys, spelled once. */
const TOOL_KEYS = ['terminalTool', 'editorTool'];

/** The two rows whose ↺ is performed by their container (`ToolPickerRow`'s
 *  `reset` override → `onChange` → `changeTool`), never by `resetRow`. */
const ROUTED_ROWS = ['general.terminal-tool', 'general.editor-tool'];

/** Both keys off their default, so a descriptor whose patch varies with the
 *  current value is still observed producing one. */
const MUTATED: UiSettings = {
  ...DEFAULT_UI_SETTINGS,
  terminalTool: 'windows-terminal',
  editorTool: 'vscode',
};

/**
 * Every shipped module under `src/` as raw text (`obsExport.test.ts`'s
 * technique — this tsconfig has no node types, so no `node:fs`).
 *
 * Comments are stripped before the check, for UA17's reason: `useExternalToolScan`'s
 * header and `catalog/reset.ts`'s both NAME the two keys in order to state the
 * rule about them, and a check that forbade the words would forbid documenting
 * it.
 */
const SOURCES = import.meta.glob('/src/**/*.{ts,tsx}', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

const BLOCK_COMMENT = /\/\*[\s\S]*?\*\//g;
const LINE_COMMENT = /^\s*\/\/.*$/gm;

/**
 * Every shipped file that names either key, and WHY it is allowed to.
 *
 * Adding a file here is the point: it forces the next author to say which of the
 * two writer kinds theirs is. An **explicit selection** (the user said what they
 * want) MUST go through `changeTool`, which clears that kind's owed adopt in the
 * same act. A **disk read** must NOT clear it, because an owed adopt is itself a
 * pending disk read. Anything that is neither has no business naming these keys.
 */
const PINNED: Readonly<Record<string, string>> = {
  // ── the state owner and its two channels ──────────────────────────────────
  'src/hooks/useUiSettings.ts':
    'holds both values; `handleSettingsChange` (the write), `adoptToolSelection` (§16.16-5 disk adopt) and `hydrateUiSettings` (launch-time disk read, one-shot behind App’s `launchedRef`) are its only setters',
  'src/App.tsx': 'threads the two values and the adopt callback into SettingsPanel; reads only',
  // ── the rule and the controls it governs ──────────────────────────────────
  'src/components/settings/useExternalToolScan.ts':
    'rule 6 itself: the ONLY producer of a tool-selection patch (`selectionFor`), for both the list and the ↺',
  'src/components/settings/catalog/reset.ts':
    '`RoutedToolKey` — makes `resetKey(\'editorTool\', …)` a compile error and `resetRouted` the only descriptor that may name either key',
  'src/components/settings/catalog/general.ts': 'the two rows’ catalog entries, via `resetRouted`',
  'src/components/settings/ToolPickerRow.tsx':
    'both write controls of one row: the list and the ↺, both through the row’s one `onChange`',
  'src/components/settings/categories/GeneralCategory.tsx':
    'the container: reads the values, hands `change` to the hook as `changeToolSelection`',
  'src/components/settings/SettingsExternalToolsSection.tsx': 'props only — presentational leaf',
  'src/components/settings/useSettingsPanelAdapter.ts':
    'props → context values + the reset snapshot; `resetRow` REFUSES routed descriptors',
  'src/components/settings/SettingsContext.ts': 'the context key union',
  // ── declarations, defaults, fixtures ──────────────────────────────────────
  'src/ipc/types/settings.ts': '`UiSettings` / `UiSettingsPatch` declarations',
  'src/settings/defaults.ts': 'the defaults table (both default to `\'\'` — auto-detect)',
  'src/components/settings/coverageFixtures.ts': 'settings-panel fixtures',
  'src/test/uiSettingsKit.ts': 'test kit fixtures',
  // ── the mock backend: the simulated DISK, not renderer selection state ────
  'src/ipc/mock/persistence.ts': 'mock disk: reads and normalises the stored settings',
  'src/ipc/mock/handlers/session.ts': 'mock `set_ui_settings`: merges the patch into stored state',
  'src/ipc/mock/handlers/tools.ts':
    'mock `pick_external_tool`: persists `custom` the way the backend does, before the renderer re-reads',
};

/** Shipped (non-test) files whose comment-stripped source matches `pattern`. */
function shippedMatching(pattern: RegExp): string[] {
  return Object.entries(SOURCES)
    .filter(([path]) => !/\.test\.(ts|tsx)$/.test(path))
    .filter(([, text]) => pattern.test(text.replace(BLOCK_COMMENT, '').replace(LINE_COMMENT, '')))
    .map(([path]) => path.replace(/^\//, ''))
    .sort();
}

describe('P112 rule 6 — the set of tool-setting writers is closed', () => {
  it('marks exactly the two picker rows as routed', () => {
    const routed = SETTINGS_INDEX.filter((e) => e.reset?.routed === true).map((e) => e.id);
    // Both directions matter. Losing the flag re-arms the generic `resetRow` for
    // that key; adding it to an unrelated row silently removes that row's ↺,
    // since a routed descriptor renders none without its container's override.
    expect(routed).toEqual(ROUTED_ROWS);
  });

  it('lets no catalog descriptor patch either key unless it is routed', () => {
    const producers: string[] = [];
    for (const entry of SETTINGS_INDEX) {
      const reset = entry.reset;
      if (reset === undefined) continue;
      for (const current of [DEFAULT_UI_SETTINGS, MUTATED]) {
        const patched = Object.keys(reset.patch(current, DEFAULT_UI_SETTINGS));
        if (!patched.some((key) => TOOL_KEYS.includes(key))) continue;
        producers.push(entry.id);
        expect(
          reset.routed,
          `${entry.id} patches a tool key through the generic \`resetRow\`, which cannot clear that kind's owed adopt (rule 6). Build it with \`resetRouted\` and perform the ↺ in the row's container.`,
        ).toBe(true);
        break;
      }
    }
    // Positive control: the sweep is not passing because it found nothing.
    expect(producers).toEqual(ROUTED_ROWS);
  });

  it('names either key in exactly the pinned files, each with a stated role', () => {
    // Guards the guard: a glob that silently matched nothing would satisfy every
    // assertion below.
    expect(Object.keys(SOURCES).length).toBeGreaterThan(50);
    // Set EQUALITY in both directions: a new writer fails on the left, and a
    // renamed file fails on the right instead of leaving a dead pin that quietly
    // checks nothing.
    expect(shippedMatching(/terminalTool|editorTool/)).toEqual(Object.keys(PINNED).sort());
    // The role is the whole point of the pin — an empty one would let a file be
    // added without saying which kind of writer it is.
    for (const [path, role] of Object.entries(PINNED)) {
      expect(role.length, `${path}: pin it with a role, not an empty string`).toBeGreaterThan(0);
    }
  });

  it('keeps the non-writing disk adopt to one production caller', () => {
    // The three interface/prop DECLARATIONS of the same name are not callers,
    // so the lookahead skips them.
    expect(shippedMatching(/\badoptToolSelection\s*\((?!selection: ToolSelection\))/)).toEqual([
      'src/components/settings/useExternalToolScan.ts',
    ]);
  });
});
