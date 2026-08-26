// Spec-003 (UI contract §3.1): the solo/hide context-menu item group appended to
// branch / remote-branch / tag row menus. Pure item construction — the actions
// come from useGraphFilter via RepoWorkspace. Mode-exclusive rules:
//
//   | Current state              | Items shown on the row                          |
//   | no ref filter              | Solo {noun} · Hide {noun}                       |
//   | solo active, not in set    | Add to solo                                     |
//   | solo active, in set        | Remove from solo                                |
//   | hide active, not hidden    | Solo {noun} (replaces the hide set) · Hide {noun}|
//   | hide active, hidden        | Solo {noun} · Show {noun}                       |
//
// The first item carries `separatorBefore` so the group reads as its own block.
import { createElement } from 'react';
import { EyeOff, ListFilter } from 'lucide-react';

import type { ContextMenuItem } from './ContextMenu';
import type { PaletteAction } from './paletteActions';
import type { GraphFilterController } from '../hooks/useGraphFilter';

const soloIcon = () => createElement(ListFilter, { size: 16, strokeWidth: 2 });
const hideIcon = () => createElement(EyeOff, { size: 16, strokeWidth: 2 });

export function refFilterMenuItems(
  controller: GraphFilterController,
  fullRef: string,
  noun: 'branch' | 'tag',
): ContextMenuItem[] {
  const mode = controller.refFilter !== null ? controller.refFilter.mode : null;
  const member = controller.refFilter?.refs.includes(fullRef) === true;
  const items: ContextMenuItem[] = [];

  if (mode === 'solo') {
    // A non-member deliberately gets ONLY "Add to solo" (no second solo verb —
    // two solo verbs one line apart invite replacing a built-up set by mistake).
    items.push(
      member
        ? {
            label: 'Remove from solo',
            icon: soloIcon(),
            disabled: false,
            onSelect: () => controller.unfilterRef(fullRef),
          }
        : {
            label: 'Add to solo',
            icon: soloIcon(),
            disabled: false,
            onSelect: () => controller.addToSolo(fullRef),
          },
    );
  } else {
    // No filter, or hide mode: Solo always starts a fresh solo set [ref]
    // (replacing any hide set — nondestructive, no confirm).
    items.push({
      label: `Solo ${noun}`,
      icon: soloIcon(),
      disabled: false,
      onSelect: () => controller.soloRef(fullRef),
    });
    items.push(
      mode === 'hide' && member
        ? {
            label: `Show ${noun}`,
            icon: hideIcon(),
            disabled: false,
            onSelect: () => controller.unfilterRef(fullRef),
          }
        : {
            label: `Hide ${noun}`,
            icon: hideIcon(),
            disabled: false,
            onSelect: () => controller.hideRef(fullRef),
          },
    );
  }
  items[0].separatorBefore = true;
  return items;
}

/** Spec-003 §3.1: the checked-out branch's sidebar row — its regular branch menu
 *  is empty, so it gets the solo/hide group ALONE (no separator when it is the
 *  only group; hiding HEAD's branch only drops its pill). */
export function headRowFilterMenuItems(
  controller: GraphFilterController,
  name: string,
): ContextMenuItem[] {
  return refFilterMenuItems(controller, `refs/heads/${name}`, 'branch').map((it, i) =>
    i === 0 ? { ...it, separatorBefore: undefined } : it,
  );
}

/** Spec-003 §3.2: the two command-palette declutter actions (palette-only, no
 *  shortcut assigned — the shortcut map is crowded). */
export function graphFilterPaletteEntries(controller: GraphFilterController): PaletteAction[] {
  return [
    {
      id: 'graph.toggle-first-parent',
      title: 'Toggle first-parent view',
      group: 'action',
      keywords: 'graph filter declutter simplify mainline',
      run: controller.toggleFirstParent,
    },
    // Spec-004 §3: palette-only, no shortcut (same call as spec-003).
    {
      id: 'graph.toggle-fold-linear',
      title: 'Toggle fold linear runs',
      group: 'action',
      keywords: 'fold collapse linear runs condense declutter graph',
      run: controller.toggleFoldLinear,
    },
    {
      id: 'graph.clear-filters',
      title: 'Clear graph filters',
      group: 'action',
      keywords: 'solo hide show full graph',
      disabled: !controller.requested,
      run: controller.clearAll,
    },
  ];
}
