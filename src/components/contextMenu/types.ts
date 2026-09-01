export interface ContextMenuItem {
  label: string;
  /** P92: native `title` tooltip on the row (picker rows carry the FULL ref
   *  name, since the label is the short/collapsed form and may ellipsise). */
  title?: string;
  /** P92: `true` ⇒ this entry renders as a non-interactive `role="separator"`
   *  rule instead of a row. Keyboard navigation skips it for free (the focus
   *  queries scope to `button[role="menuitem*"]`). `label` is ignored. */
  separator?: true;
  /** Optional: a pure-submenu parent may omit a default action. */
  onSelect?(): void;
  disabled?: boolean;
  /** P10 T2: optional leading 16×16 monochrome glyph, inherits item text color. */
  icon?: React.ReactNode;
  /** Present ⇒ this row opens a flyout submenu of variants. */
  children?: ContextMenuItem[];
  /** 'danger' ⇒ red icon + label (destructive action). */
  tone?: 'default' | 'danger';
  /**
   * P69i (UI §4.4). Present ⇒ the row renders `role="menuitemradio"` with
   * `aria-checked`, and the whole list reserves a 16px leading check column
   * (a ✓ glyph in `--text-1`, never a background tint — so a checked row stays
   * legible in both themes and in forced-colours mode).
   *
   * Absent ⇒ `role="menuitem"` and no column: byte-identical to pre-P69i.
   * `menuitemradio` rather than `menuitemcheckbox` because these lists are
   * "at most one in effect" (the identity menu), not multi-select.
   */
  checked?: boolean;
  /**
   * P69i (UI §4.4). One secondary line under the label — 12px `--text-2`,
   * ellipsised, never focusable. The row grows 32 → 46px when present.
   */
  detail?: string;
}

/** P92: the open-menu state a container holds (position + items + the optional
 *  §4 header / accessible name). Shared so every ContextMenu owner spells it the
 *  same way. */
export interface ContextMenuState {
  x: number;
  y: number;
  items: ContextMenuItem[];
  header?: string;
  ariaLabel?: string;
}

export interface ContextMenuProps {
  /** clientX anchor. */
  x: number;
  /** clientY anchor. */
  y: number;
  items: ContextMenuItem[];
  /** Fired by every dismiss path AND after an enabled item activates. */
  onClose(): void;
  /**
   * P69i (UI §4.4). Non-interactive block rendered above the list inside
   * `.context-menu`, `role="presentation"`. Excluded from keyboard navigation
   * for free: the focus queries scope to the row buttons.
   */
  header?: React.ReactNode;
  /** P92: accessible name for the menu root (e.g. `3 more refs on commit 4f2a91c`).
   *  Absent ⇒ no `aria-label` (byte-identical to pre-P92). */
  ariaLabel?: string;
  /**
   * P69i. `aria-busy` on the menu root, for a menu that deliberately stays open
   * while an activated row's write settles (UI §4.5). Additive and generic.
   */
  busy?: boolean;
}
