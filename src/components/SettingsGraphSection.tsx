// P51b: Settings "Commit graph" leaf section (own file, mirrors the other
// extracted settings sections). Three geometry sliders (avatar / row / lane) plus
// the P51 per-row detail controls — short SHA, author-name column, date column, a
// date-basis choice, an ahead/behind chip toggle, and compact rows. Every control
// fires `onChange` with a WHOLE-STRUCT `graph` patch (the backend patch path is
// whole-struct); App updates state live and debounces the persist, exactly like
// the pre-P51 inline sliders.
//
// P69j: re-skinned onto the canonical row (UI §5.1) in the three catalog groups —
// Geometry / Row details / Badges. Eight checkboxes became switches and the
// date-basis fieldset became a segmented control (UI §5.3 items 5 and 6); both
// are CSS skins over the SAME native inputs, so every
// `getByRole('checkbox'|'radio', {name})` in the vitest and e2e suites still
// resolves, and `#settings-graph-row` + all eight toggle labels are frozen (§11).
//
// The section keeps its own props (§2.3 leaf boundary) so `SettingsSections.test`
// keeps rendering it bare; labels, help text and the `↺` descriptors all come
// from the catalog via the row id, so nothing here restates them.
//
// DENSITY: one geometry in both `cozy` and `compact` (UI §5.1 / D10). These rows
// are 44px min in BOTH densities on purpose — do not add a density variant here.

import type { GraphPrefs, UiSettingsPatch } from '../ipc';
import { NumberSlider } from './NumberSlider';
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';
import { SettingsSegmented } from './settings/SettingsSegmented';
import { SettingsSwitchRow } from './settings/SettingsSwitchRow';
import { settingsRowHelpId, settingsRowLabelId } from './settings/settingsCatalog';
import {
  AVATAR_RADIUS_MAX,
  AVATAR_RADIUS_MIN,
  LANE_WIDTH_MAX,
  LANE_WIDTH_MIN,
  ROW_HEIGHT_MAX,
  ROW_HEIGHT_MIN,
} from '../settings/ranges';

const NODE_SIZE = 'graph.node-size';
const ROW_HEIGHT = 'graph.row-height';
const LANE_WIDTH = 'graph.lane-width';
const COMPACT = 'graph.compact-rows';
const SHORT_SHA = 'graph.short-sha';
const AUTHOR = 'graph.author-name';
const DATE = 'graph.date';
const DATE_BASIS = 'graph.date-basis';
const AHEAD_BEHIND = 'graph.ahead-behind';
const SIGNATURE = 'graph.signature-badge';
const PR_BADGE = 'graph.pr-badges';
const CI_STATUS = 'graph.ci-status';

export interface SettingsGraphSectionProps {
  graph: GraphPrefs;
  /** Whole-struct `graph` patch channel (App owns the debounced persist). */
  onChange(patch: UiSettingsPatch): void;
}

export function SettingsGraphSection({ graph, onChange }: SettingsGraphSectionProps) {
  // Each control sends the ENTIRE graph object with one field replaced.
  const patch = (next: Partial<GraphPrefs>): void => onChange({ graph: { ...graph, ...next } });

  return (
    <>
      <SettingsGroup id="graph-geometry" title="Geometry">
        {/* P11d/P51: single node-size knob == avatarRadius (post-P7 the graph has
            no commit dot — each commit is an avatar disc). The dead `dotRadius`
            field was removed entirely in P51 (D7). */}
        <SettingsRow id={NODE_SIZE} controlId="settings-graph-avatar">
          <NumberSlider
            id="settings-graph-avatar"
            label="Commit node size"
            value={graph.avatarRadius}
            min={AVATAR_RADIUS_MIN}
            max={AVATAR_RADIUS_MAX}
            unit="px"
            describedBy={settingsRowHelpId(NODE_SIZE)}
            onChange={(v) => patch({ avatarRadius: v })}
          />
        </SettingsRow>

        {/* §11: the id AND the label are frozen — e2e and vitest query both. */}
        <SettingsRow id={ROW_HEIGHT} controlId="settings-graph-row">
          <NumberSlider
            id="settings-graph-row"
            label="Row height"
            value={graph.rowHeight}
            min={ROW_HEIGHT_MIN}
            max={ROW_HEIGHT_MAX}
            unit="px"
            describedBy={settingsRowHelpId(ROW_HEIGHT)}
            onChange={(v) => patch({ rowHeight: v })}
          />
        </SettingsRow>

        <SettingsRow id={LANE_WIDTH} controlId="settings-graph-lane">
          <NumberSlider
            id="settings-graph-lane"
            label="Lane width"
            value={graph.laneWidth}
            min={LANE_WIDTH_MIN}
            max={LANE_WIDTH_MAX}
            unit="px"
            describedBy={settingsRowHelpId(LANE_WIDTH)}
            onChange={(v) => patch({ laneWidth: v })}
          />
        </SettingsRow>

        <SettingsSwitchRow
          id={COMPACT}
          checked={graph.compact}
          onChange={(v) => patch({ compact: v })}
        />
      </SettingsGroup>

      {/* P51b: per-row detail toggles. Turning a column off hides it AND reclaims
          its width — the summary reflows (see rightColumns.ts). */}
      <SettingsGroup id="graph-row-details" title="Row details">
        <SettingsSwitchRow
          id={SHORT_SHA}
          checked={graph.showSha}
          onChange={(v) => patch({ showSha: v })}
        />
        <SettingsSwitchRow
          id={AUTHOR}
          checked={graph.showAuthor}
          onChange={(v) => patch({ showAuthor: v })}
        />
        <SettingsSwitchRow
          id={DATE}
          checked={graph.showDate}
          onChange={(v) => patch({ showDate: v })}
        />

        {/* Date basis: which timestamp the date column + hover tooltip use. Kept
            enabled even when the date column is hidden — it still drives the
            absolute-date hover tooltip. */}
        <SettingsRow id={DATE_BASIS}>
          <SettingsSegmented<GraphPrefs['dateBasis']>
            name="graph-date-basis"
            value={graph.dateBasis}
            labelledBy={settingsRowLabelId(DATE_BASIS)}
            describedBy={settingsRowHelpId(DATE_BASIS)}
            options={[
              { value: 'author', label: 'Author' },
              { value: 'committer', label: 'Committer' },
            ]}
            onChange={(dateBasis) => patch({ dateBasis })}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup id="graph-badges" title="Badges">
        {/* P51c: ahead/behind chip on local-branch pills. Renders only on diverged
            branches (ahead/behind > 0) — low clutter, so on by default. */}
        <SettingsSwitchRow
          id={AHEAD_BEHIND}
          checked={graph.showAheadBehind}
          onChange={(v) => patch({ showAheadBehind: v })}
        />
        {/* P58c: light the per-row signature badge (verified/unverified/unknown)
            from git's signature check. Off ⇒ the faint stub renders and NO
            verification is requested. */}
        <SettingsSwitchRow
          id={SIGNATURE}
          checked={graph.showSignatureBadge}
          onChange={(v) => patch({ showSignatureBadge: v })}
        />
        {/* P63: forge-driven, branch-tip-scoped badges. Default OFF (they need a
            connected forge + network) and are suppressed while Compact is on. */}
        <SettingsSwitchRow
          id={PR_BADGE}
          checked={graph.showPrBadge}
          onChange={(v) => patch({ showPrBadge: v })}
        />
        <SettingsSwitchRow
          id={CI_STATUS}
          checked={graph.showCiStatus}
          onChange={(v) => patch({ showCiStatus: v })}
        />
        <p className="settings-group-note">
          Requires a connected forge (GitHub, GitLab, Bitbucket, or Azure DevOps). Hidden in Compact
          mode.
        </p>
      </SettingsGroup>
    </>
  );
}
