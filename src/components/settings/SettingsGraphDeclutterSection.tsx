// Spec-003 (UI contract §4): the Commit-graph "Declutter" group — the
// first-parent switch (the same persisted value as the graph-pane popover
// switch: one setting, two controls) and the read-only branch-filters summary
// with its Clear action (the SIDEBAR is the editor; Settings builds no ref
// picker). Leaf section (§2.3): its props are its only value source, so the
// existing bare-render test idiom keeps working.
//
// The stale sentence of the contract's note matrix is NOT rendered here: the
// backend's `seedRefsApplied` verdict is per-repo runtime state living in the
// workspace, and Settings is App-level — see the spec-003 report (deviation).

import type { GraphRefFilter, UiSettingsPatch } from '../../ipc';
import { graphFilterSummary, shortRefName } from '../../hooks/useGraphFilter';
import { SettingsGroup } from './SettingsGroup';
import { SettingsRow } from './SettingsRow';
import { SettingsSwitchRow } from './SettingsSwitchRow';

const FIRST_PARENT = 'graph.first-parent';
const BRANCH_FILTERS = 'graph.branch-filters';

export interface SettingsGraphDeclutterSectionProps {
  graphFirstParent: boolean;
  graphRefFilter: GraphRefFilter | null;
  /** The shared debounced settings patch channel (App owns the persist). */
  onChange(patch: UiSettingsPatch): void;
}

/** The §4.2 stateful note: `Solo: x +n` / `n branches hidden` / the empty hint. */
function filterNote(refFilter: GraphRefFilter | null): string {
  if (refFilter === null || refFilter.refs.length === 0) {
    return 'None. Right-click a branch in the sidebar to solo or hide it.';
  }
  if (refFilter.mode === 'solo') {
    const n = refFilter.refs.length;
    const first = shortRefName(refFilter.refs[0]);
    return n === 1 ? `Solo: ${first}` : `Solo: ${first} +${n - 1}`;
  }
  return graphFilterSummary(false, refFilter); // "1 branch hidden" / "n branches hidden"
}

export function SettingsGraphDeclutterSection({
  graphFirstParent,
  graphRefFilter,
  onChange,
}: SettingsGraphDeclutterSectionProps) {
  const hasRefFilter = graphRefFilter !== null && graphRefFilter.refs.length > 0;
  const note = filterNote(graphRefFilter);
  return (
    <SettingsGroup id="graph-declutter" title="Declutter">
      <SettingsSwitchRow
        id={FIRST_PARENT}
        checked={graphFirstParent}
        onChange={(next) => onChange({ graphFirstParent: next })}
      />
      <SettingsRow
        id={BRANCH_FILTERS}
        hint={
          <p className="settings-row-note" title={note}>
            {note}
          </p>
        }
      >
        <button
          type="button"
          className="btn-secondary"
          aria-label="Clear branch filters"
          disabled={!hasRefFilter}
          onClick={() => onChange({ graphRefFilter: null })}
        >
          Clear
        </button>
      </SettingsRow>
    </SettingsGroup>
  );
}
