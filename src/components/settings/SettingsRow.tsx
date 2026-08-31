// P69g — UI §5.1 / ui-reference §12.2: THE canonical settings row.
//
//   grid-template-columns: 1fr auto 24px
//   row 1: [ label ] [ control ] [ ↺ ]
//   row 2: [ help / draft hint ]   (control spans both rows)
//
// The row resolves its label, help text and reset descriptor from the CATALOG by
// `id`, so a row can never render text the search index does not know about —
// that is exactly the drift `settingsCatalog.coverage.test.tsx` checks, and doing
// it here means the check cannot be defeated by a call site passing its own copy.
//
// The `↺` is rendered ONLY when the value differs from its default (UI §5.7). Its
// 24px column is always present, so appearing/disappearing never shifts the row.

import { useContext, type ReactNode } from 'react';

import { DEFAULT_UI_SETTINGS } from '../../settings/defaults';
import { SettingsActionsContext, SettingsValuesContext } from './SettingsContext';
import { findSettingsRow, settingsRowHelpId, settingsRowLabelId } from './settingsCatalog';
import { highlightTerms } from './settingsHighlight';
import { useSettingsSearch } from './SettingsSearchContext';
import type { SettingsRowId } from './types';

export interface SettingsRowResetOverride {
  isDefault: boolean;
  onReset(): void;
}

export function SettingsRow({
  id,
  profileId,
  controlId,
  rowLabel,
  stacked,
  disabled,
  reset,
  hint,
  children,
}: {
  id: SettingsRowId;
  /** Amendment A (AM-1): the instance stamp of a `repeats: 'perProfile'` row.
   *  Instance identity is `(data-setting-id, data-profile-id)`; the coverage
   *  guard asserts the rendered instance set equals the fixture's profile ids,
   *  which is what catches ONE card silently dropping a field. */
  profileId?: string;
  /** DOM id of the control. Present ⇒ the label is a `<label for>`, which both
   *  names the control and makes the label text a second hit target. */
  controlId?: string;
  /** Visible label when it differs from the control's accessible name — the only
   *  case is a button row (catalog `label` is the BUTTON text, e.g. `Show tour`
   *  under the row title `Welcome tour`). */
  rowLabel?: string;
  /** UI §5.1: label / help / control each on their own grid row, control 100%. */
  stacked?: boolean;
  disabled?: boolean;
  /** Reset source for a leaf section rendered OUTSIDE the provider (its props are
   *  its only value source). Omitted ⇒ resolved from the catalog + context. */
  reset?: SettingsRowResetOverride;
  /** P69c §13.2.1: the draft-divergence hint shares the help cell and hides the
   *  help text while it shows. Wired by the increment that lands the hint hook. */
  hint?: ReactNode;
  children: ReactNode;
}) {
  const entry = findSettingsRow(id);
  const values = useContext(SettingsValuesContext);
  const actions = useContext(SettingsActionsContext);
  // P69k: inside a search result block this row renders only if it is one of the
  // hits. The page above it is the REAL page, so this is what turns it into a
  // result list without a second renderer (UI §3.1).
  const search = useSettingsSearch();

  if (import.meta.env.DEV && entry === undefined) {
    console.error(
      `SettingsRow "${id}" has no catalog entry — the row is unsearchable and the coverage guard will fail. Add it to src/components/settings/catalog/.`,
    );
  }

  const label = entry?.label ?? id;
  const help = entry?.help;
  const descriptor = entry?.reset;

  // Context is optional on purpose: the leaf sections keep their own props (§2.3)
  // and their suites render them bare, with no provider above.
  const resolved: SettingsRowResetOverride | null =
    reset ??
    (descriptor !== undefined && values !== null && actions !== null
      ? {
          isDefault: descriptor.isDefault(values.snapshot, DEFAULT_UI_SETTINGS),
          onReset: () => actions.resetRow(id),
        }
      : null);
  const showReset = descriptor !== undefined && resolved !== null && !resolved.isDefault;

  const className = [
    'settings-row',
    stacked === true ? 'settings-row--stacked' : '',
    // P69c §13.2.1: reserve the help line on slider rows so nothing below moves
    // when the draft hint replaces the help text mid-typing.
    entry?.control === 'numberSlider' ? 'settings-row--slider' : '',
    disabled === true ? 'is-disabled' : '',
  ]
    .filter((c) => c !== '')
    .join(' ');

  // A `repeats: 'perProfile'` row exists once per card, so its label/help
  // element ids must be per-INSTANCE or the DOM would carry N duplicates of each
  // (and every card's `aria-describedby` would resolve to the first one).
  const labelId = settingsRowLabelId(id, profileId);
  const labelText = rowLabel ?? label;
  // UI §3.2: only the LABEL is highlighted — help text at 12px turns into noise.
  const labelNode = search === null ? labelText : highlightTerms(labelText, search.terms);

  if (search !== null && !search.visible.has(id)) return null;

  return (
    <div className={className} data-setting-id={id} data-profile-id={profileId}>
      {controlId === undefined ? (
        <span className="settings-row-label" id={labelId}>
          {labelNode}
        </span>
      ) : (
        <label className="settings-row-label" id={labelId} htmlFor={controlId}>
          {labelNode}
        </label>
      )}
      <div className="settings-row-control" data-setting-control="">
        {children}
      </div>
      <div className="settings-row-reset">
        {showReset && resolved !== null && (
          <button
            type="button"
            className="btn-icon settings-reset"
            aria-label={`Reset ${label} to default`}
            title={`Reset to default (${descriptor?.defaultLabel ?? ''})`}
            /* A disabled row is dimmed to .55, which puts the glyph under 3:1.
               A control the user cannot read may not still be clickable, so the
               ↺ disables with its row rather than being excluded from the dim —
               the value is unreachable anyway while its control is inert. */
            disabled={disabled}
            onClick={resolved.onReset}
          >
            {'↺'}
          </button>
        )}
      </div>
      <div className="settings-row-help-slot">
        {help !== undefined && (
          <p className="settings-row-help" id={settingsRowHelpId(id, profileId)}>
            {help}
          </p>
        )}
        {hint}
      </div>
    </div>
  );
}
