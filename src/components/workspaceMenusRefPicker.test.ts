// P92 — the ref-picker menu level: the "+N" overflow chip's menu and the
// multi-ref commit-row menu. Covers the acceptance criteria that are pure
// item-building (§6.1–§6.5); the DOM-level menu behaviour lives in
// ContextMenu.test.tsx.
import { describe, expect, it } from 'vitest';

import { createWorkspaceMenus } from './workspaceMenus';
import {
  graphMenuState,
  overflowHeaderText,
  overflowMenuLabel,
  pickerLabel,
  pickerTitle,
} from './workspaceMenusRefPicker';
import { OID_OTHER, labelsOf, makeDeps } from '../test/workspaceMenusFixtures';
import { groupRefs } from '../graph/refLabels';
import type { RefLabel } from '../ipc';
import type { GraphContextTarget } from '../graph/GraphCanvas';

const ref = (name: string, kind: RefLabel['kind'], isHead = false): RefLabel => ({
  name,
  kind,
  isHead,
});

const menus = () => createWorkspaceMenus(makeDeps());

/** A commit/whole-row target carrying the row's grouped entities. */
const rowTarget = (refs: RefLabel[]): GraphContextTarget => ({
  kind: 'commit',
  index: 0,
  oid: OID_OTHER,
  entities: groupRefs(refs),
});

describe('P92 §2.2 — commit-row menu with ≤1 actionable ref (no regression)', () => {
  it('a single-branch row is byte-identical to the pre-P92 branch menu', () => {
    const items = menus().buildContextItems(rowTarget([ref('feature', 'localBranch')]));
    const before = menus().buildContextItems({
      kind: 'ref',
      ref: ref('feature', 'localBranch'),
      oid: OID_OTHER,
    });
    expect(labelsOf(items)).toEqual(labelsOf(before));
    // Flat: no picker level, no separator, no `Merge… ▸` indirection.
    expect(items.some((i) => i.separator === true)).toBe(false);
    expect(labelsOf(items)).toContain('Merge feature into main');
  });

  it('a ref-less row is the plain commit menu', () => {
    const items = menus().buildContextItems(rowTarget([]));
    expect(labelsOf(items)).toEqual(
      labelsOf(menus().buildContextItems({ kind: 'commit', index: 0, oid: OID_OTHER })),
    );
  });

  it('a row whose only entity is a tag keeps the pre-P92 commit menu', () => {
    const items = menus().buildContextItems(rowTarget([ref('v1.0', 'tag')]));
    expect(labelsOf(items)).toEqual(
      labelsOf(menus().buildContextItems({ kind: 'commit', index: 0, oid: OID_OTHER })),
    );
  });
});

describe('P92 §2.2 — commit-row menu with ≥2 actionable refs', () => {
  const target = rowTarget([
    ref('main', 'localBranch', true),
    ref('feature', 'localBranch'),
    ref('origin/main', 'remoteBranch'),
    ref('origin/feature', 'remoteBranch'),
    ref('v1.0', 'tag'),
  ]);

  it('prepends one row per ref in groupRefs order, above the commit actions', () => {
    const items = menus().buildContextItems(target);
    // §6.5: main + origin/main collapse to ONE row; likewise feature.
    expect(labelsOf(items).slice(0, 3)).toEqual(['main', 'feature', '# v1.0']);
    // §6.4: the unchanged commit actions follow, behind a separator.
    expect(items[3].separator).toBe(true);
    expect(labelsOf(items)).toContain('Create branch here');
    expect(labelsOf(items)).toContain('Compare with HEAD');
  });

  it('picker rows OMIT onSelect (clicking one opens its flyout, never mutates)', () => {
    const items = menus().buildContextItems(target);
    for (const row of items.slice(0, 3)) {
      expect(row.onSelect).toBeUndefined();
      expect(row.children?.length).toBeGreaterThan(0);
      expect(row.disabled).toBe(false);
    }
  });

  it('each row opens the SAME menu that ref’s pill would open', () => {
    const items = menus().buildContextItems(target);
    const pill = (r: RefLabel) =>
      labelsOf(menus().buildContextItems({ kind: 'ref', ref: r, oid: OID_OTHER }));
    expect(labelsOf(items[1].children ?? [])).toEqual(pill(ref('feature', 'localBranch')));
    expect(labelsOf(items[2].children ?? [])).toEqual(pill(ref('v1.0', 'tag')));
  });

  it('§2.2: the HEAD branch is included — its submenu is Rename… + commit actions', () => {
    const items = menus().buildContextItems(target);
    expect(items[0].label).toBe('main');
    expect(labelsOf(items[0].children ?? [])[0]).toBe('Rename…');
  });

  it('rows carry the FULL ref name as `title` (remote-only keeps its remote)', () => {
    const items = menus().buildContextItems(
      rowTarget([ref('main', 'localBranch', true), ref('origin/feature', 'remoteBranch'), ref('v1.0', 'tag')]),
    );
    // The remote-only entity's LABEL is the short `feature`; its title keeps the
    // full remote shorthand it actually targets.
    expect(labelsOf(items).slice(0, 3)).toEqual(['main', 'feature', '# v1.0']);
    expect(items.map((i) => i.title).slice(0, 3)).toEqual(['main', 'origin/feature', 'v1.0']);
  });
});

describe('P92 §1.1 — the "+N" overflow picker', () => {
  it('lists exactly the hidden entities passed in, in order', () => {
    const entities = groupRefs([
      ref('feature', 'localBranch'),
      ref('v1.0', 'tag'),
      ref('stash@{0}', 'stash'),
    ]);
    const items = menus().buildContextItems({ kind: 'refPicker', entities, oid: OID_OTHER });
    expect(labelsOf(items)).toEqual(['feature', '# v1.0', 'stash@{0}']);
    expect(labelsOf(items[2].children ?? [])).toEqual(['Apply', 'Pop', 'Drop']);
  });

  it('an entity with no actions renders disabled and WITHOUT a chevron', () => {
    const entities = groupRefs([ref('HEAD', 'head', true), ref('feature', 'localBranch')]);
    const items = menus().buildContextItems({ kind: 'refPicker', entities, oid: OID_OTHER });
    expect(items[0].disabled).toBe(true);
    expect(items[0].children).toBeUndefined();
    expect(items[1].disabled).toBe(false);
  });

  it('§4: header + menu-root accessible name (singular and plural)', () => {
    expect(overflowHeaderText(1)).toBe('1 more ref');
    expect(overflowHeaderText(3)).toBe('3 more refs');
    expect(overflowMenuLabel(3, 'abcdef1234567890')).toBe('3 more refs on commit abcdef1');
  });

  it('graphMenuState titles ONLY the overflow picker', () => {
    const entities = groupRefs([ref('feature', 'localBranch')]);
    const picker = graphMenuState({ kind: 'refPicker', entities, oid: OID_OTHER }, [], 4, 8);
    expect(picker).toMatchObject({ x: 4, y: 8, header: '1 more ref' });
    const plain = graphMenuState({ kind: 'commit', index: 0, oid: OID_OTHER }, [], 4, 8);
    expect(plain.header).toBeUndefined();
    expect(plain.ariaLabel).toBeUndefined();
  });
});

describe('P92 — label/title helpers', () => {
  it('tags are prefixed, branches show the short name once', () => {
    const [branch] = groupRefs([ref('main', 'localBranch'), ref('origin/main', 'remoteBranch')]);
    expect(pickerLabel(branch)).toBe('main');
    expect(pickerTitle(branch)).toBe('main'); // local-first, like the pill hit-test
    const [tag] = groupRefs([ref('v1.5.0', 'tag')]);
    expect(pickerLabel(tag)).toBe('# v1.5.0');
  });
});
