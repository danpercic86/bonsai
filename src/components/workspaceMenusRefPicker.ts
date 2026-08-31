/** P92 §1.2 / §2.2 — the shared "pick a ref" menu level.
 *
 *  One pattern serves both problems the contract describes: the "+N" overflow
 *  chip opens this level for the HIDDEN entities of a row, and the commit-row
 *  menu prepends it when the row carries ≥2 actionable refs. Each row is a pure
 *  parent: its `children` are the SAME menu the entity's own pill would open
 *  (built by the caller-supplied `refItemsFor`, i.e. `buildContextItems`'
 *  `kind:'ref'` branch), so overflow refs and visible pills can never diverge.
 *
 *  PURE: builds arrays, performs no side effects until a leaf's `onSelect` runs.
 */

import { createElement } from 'react';
import type { ContextMenuItem, ContextMenuState } from './ContextMenu';
import { BranchIcon, StashApplyIcon, TagIcon } from './menuIcons';
import type { GraphContextTarget } from '../graph/GraphCanvas';
import { entityLabel, type RefEntity } from '../graph/refLabels';
import { targetRefOf } from '../graph/hitTest';

/** Builds the full menu for one entity — the row's flyout contents. Supplied by
 *  `workspaceMenus` so this module stays free of deps/handlers. */
export type RefItemsFor = (entity: RefEntity, oid: string) => ContextMenuItem[];

/** P92 §1.2 / §8.3: the row label — the entity's DISPLAY label. Re-exported from
 *  `refLabels.entityLabel`, the SAME function `entityStyle().label` uses, so the
 *  canvas pill / graph tooltip and this menu row can never drift: the short
 *  branch name ONCE (`main`, never `main` + `origin/main`), `# v1.5.0` for tags,
 *  `stash@{0}` for stashes. Verbs live in the submenu, never here. */
export { entityLabel as pickerLabel } from '../graph/refLabels';

/** The FULL ref name behind an entity (`origin/topic/x` for a remote-only
 *  branch) — the row's `title`, since the visible label is the collapsed form. */
export function pickerTitle(e: RefEntity): string {
  return targetRefOf(e)?.name ?? e.name;
}

function pickerIcon(e: RefEntity): React.ReactNode {
  if (e.kind === 'tag') return createElement(TagIcon);
  if (e.kind === 'stash') return createElement(StashApplyIcon);
  return createElement(BranchIcon);
}

/** P92 §2.2: the entities on a row that actually have something to offer — the
 *  candidate list whose length decides whether the picker level appears at all
 *  (`≤ 1` ⇒ today's flat menu, unchanged). Order is `groupRefs` order; never
 *  re-sorted, so the menu and the pills agree. */
export function actionableEntities(
  entities: readonly RefEntity[],
  oid: string,
  refItemsFor: RefItemsFor,
): RefEntity[] {
  return entities.filter((e) => refItemsFor(e, oid).length > 0);
}

/** P92 §1.2: one row per entity, in `groupRefs` order.
 *
 *  Rows deliberately OMIT `onSelect` entirely (verified against
 *  `ContextMenu.tsx` `activate()`: it fires `onSelect` and CLOSES the menu when
 *  one is defined, and only otherwise toggles the flyout). A no-op `onSelect`
 *  would therefore make "pick `main`" silently close the menu; passing the first
 *  child's action would make it mean "checkout main". Both are wrong: a picker
 *  row must only ever open its flyout.
 *
 *  An entity with an empty menu renders DISABLED with no chevron — never a
 *  chevron that opens nothing. */
export function refPickerItems(
  entities: readonly RefEntity[],
  oid: string,
  refItemsFor: RefItemsFor,
): ContextMenuItem[] {
  return entities.map((e) => {
    const children = refItemsFor(e, oid);
    const base = {
      label: entityLabel(e),
      title: pickerTitle(e),
      icon: pickerIcon(e),
    };
    if (children.length === 0) return { ...base, disabled: true };
    return { ...base, disabled: false, children };
  });
}

/** P92 §4: the overflow menu's header + accessible name. `1 more ref` is
 *  reachable (a band that fits all but one), so both forms are specced. */
export function overflowHeaderText(n: number): string {
  return n === 1 ? '1 more ref' : `${n} more refs`;
}

export function overflowMenuLabel(n: number, oid: string): string {
  return `${overflowHeaderText(n)} on commit ${oid.slice(0, 7)}`;
}

/** P92 §4: the open-menu state for a graph right-click. The "+N" overflow picker
 *  is titled with what it stands for and names the menu root for screen readers;
 *  every other target keeps the plain (headerless) menu it has always had. */
export function graphMenuState(
  target: GraphContextTarget,
  items: ContextMenuItem[],
  x: number,
  y: number,
): ContextMenuState {
  if (target.kind !== 'refPicker') return { x, y, items };
  const n = target.entities.length;
  return { x, y, items, header: overflowHeaderText(n), ariaLabel: overflowMenuLabel(n, target.oid) };
}
