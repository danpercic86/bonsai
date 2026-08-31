// P69j — the canonical row + switch pairing, in one place (UI §5.1 + §5.5).
//
// Not a new control: it is the two contract primitives composed, because the
// pairing is verbatim identical at sixteen call sites (graph row details and
// badges 8, AI limits 3, AI runs 2, AI assistance 1, MCP 2) and the only thing that ever varies is the
// row id, the value and the handler. The `{id}-input` convention is what wires
// the row's `<label for>` to the native checkbox, so the label text stays the
// control's accessible name and `getByRole('checkbox', {name})` keeps resolving.
//
// Rows whose control is NOT a bare switch (sliders, segmented, buttons) still
// spell out `<SettingsRow>` at the call site — this covers the one shape that
// has no per-site decisions left in it.

import type { ReactNode } from 'react';

import { settingsRowHelpId } from './settingsCatalog';
import { SettingsRow } from './SettingsRow';
import { SettingsSwitch } from './SettingsSwitch';
import type { SettingsRowId } from './types';

export function SettingsSwitchRow({
  id,
  checked,
  disabled,
  describedBy,
  hint,
  onChange,
}: {
  id: SettingsRowId;
  checked: boolean;
  /** Dims the row (UI §5.4) AND makes the input inert. */
  disabled?: boolean;
  /** Overrides the default `{rowId}-help` target when the row carries a hint
   *  paragraph whose sentence changes with the value (the AI-run sentinels). */
  describedBy?: string;
  /** Extra paragraph(s) in the help cell, under the catalog help line. */
  hint?: ReactNode;
  onChange(next: boolean): void;
}) {
  const inputId = `${id}-input`;
  return (
    <SettingsRow id={id} controlId={inputId} disabled={disabled} hint={hint}>
      <SettingsSwitch
        id={inputId}
        checked={checked}
        disabled={disabled}
        describedBy={describedBy ?? settingsRowHelpId(id)}
        onChange={onChange}
      />
    </SettingsRow>
  );
}
