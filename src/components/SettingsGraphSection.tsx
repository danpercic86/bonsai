// P51b: Settings "Graph" section (own file, mirrors the other extracted
// settings sections). Three geometry sliders (avatar / row / lane) plus the
// P51 per-row detail controls — short SHA, author-name column, date column, a
// date-basis choice, an ahead/behind chip toggle, and compact rows. Every
// control fires `onChange` with a WHOLE-STRUCT `graph` patch (the backend patch
// path is whole-struct); App updates state live and debounces the persist,
// exactly like the pre-P51 inline sliders.

import type { GraphPrefs, UiSettingsPatch } from '../ipc';
import { NumberSlider } from './NumberSlider';
import {
  AVATAR_RADIUS_MAX,
  AVATAR_RADIUS_MIN,
  LANE_WIDTH_MAX,
  LANE_WIDTH_MIN,
  ROW_HEIGHT_MAX,
  ROW_HEIGHT_MIN,
} from '../settings/ranges';

export interface SettingsGraphSectionProps {
  graph: GraphPrefs;
  /** Whole-struct `graph` patch channel (App owns the debounced persist). */
  onChange(patch: UiSettingsPatch): void;
}

export function SettingsGraphSection({ graph, onChange }: SettingsGraphSectionProps) {
  // Each control sends the ENTIRE graph object with one field replaced.
  const patch = (next: Partial<GraphPrefs>): void => onChange({ graph: { ...graph, ...next } });
  return (
    <section className="settings-section">
      <h3 className="settings-section-title">Graph</h3>
      <p className="settings-section-desc">
        Tune the commit-graph geometry and per-row details. Changes preview live.
      </p>
      {/* P11d/P51: single node-size knob == avatarRadius (post-P7 the graph has
          no commit dot — each commit is an avatar disc). The dead `dotRadius`
          field was removed entirely in P51 (D7). */}
      <NumberSlider
        id="settings-graph-avatar"
        label="Commit node size"
        value={graph.avatarRadius}
        min={AVATAR_RADIUS_MIN}
        max={AVATAR_RADIUS_MAX}
        unit="px"
        onChange={(v) => patch({ avatarRadius: v })}
      />
      <NumberSlider
        id="settings-graph-row"
        label="Row height"
        value={graph.rowHeight}
        min={ROW_HEIGHT_MIN}
        max={ROW_HEIGHT_MAX}
        unit="px"
        onChange={(v) => patch({ rowHeight: v })}
      />
      <NumberSlider
        id="settings-graph-lane"
        label="Lane width"
        value={graph.laneWidth}
        min={LANE_WIDTH_MIN}
        max={LANE_WIDTH_MAX}
        unit="px"
        onChange={(v) => patch({ laneWidth: v })}
      />

      {/* P51b: per-row detail toggles. Turning a column off hides it AND reclaims
          its width — the summary reflows (see rightColumns.ts). */}
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={graph.showSha}
          onChange={(e) => patch({ showSha: e.target.checked })}
        />
        <span>Short SHA</span>
      </label>
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={graph.showAuthor}
          onChange={(e) => patch({ showAuthor: e.target.checked })}
        />
        <span>Author name</span>
      </label>
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={graph.showDate}
          onChange={(e) => patch({ showDate: e.target.checked })}
        />
        <span>Date</span>
      </label>

      {/* Date basis: which timestamp the date column + hover tooltip use. Kept
          enabled even when the date column is hidden — it still drives the
          absolute-date hover tooltip. */}
      <fieldset className="settings-radio-group">
        <legend className="settings-radio-legend">Date basis</legend>
        <label className="settings-radio">
          <input
            type="radio"
            name="graph-date-basis"
            checked={graph.dateBasis === 'author'}
            onChange={() => patch({ dateBasis: 'author' })}
          />
          <span>Author</span>
        </label>
        <label className="settings-radio">
          <input
            type="radio"
            name="graph-date-basis"
            checked={graph.dateBasis === 'committer'}
            onChange={() => patch({ dateBasis: 'committer' })}
          />
          <span>Committer</span>
        </label>
      </fieldset>

      {/* P51c: ahead/behind chip on local-branch pills. Renders only on diverged
          branches (ahead/behind > 0) — low clutter, so on by default. */}
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={graph.showAheadBehind}
          onChange={(e) => patch({ showAheadBehind: e.target.checked })}
        />
        <span>Ahead/behind on branches</span>
      </label>

      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={graph.compact}
          onChange={(e) => patch({ compact: e.target.checked })}
        />
        <span>Compact rows</span>
      </label>
    </section>
  );
}
