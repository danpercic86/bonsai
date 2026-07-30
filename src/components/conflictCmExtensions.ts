// P12 §3: CodeMirror extensions for the conflict editor — per-region accept
// widgets (§3.1) and the scrollbar overview ruler (§3.2). Both derive entirely
// from the pure `conflictRegions` helpers over the CURRENT doc, so the doc stays
// the single source of truth (the editor's updateListener mirrors it into React
// `result`). This module is imported ONLY by ConflictEditor (a lazy chunk), so
// CodeMirror never leaks into the main bundle.

import {
  EditorView,
  Decoration,
  WidgetType,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from '@codemirror/view';
import { StateField, type EditorState, type Extension, type Range } from '@codemirror/state';
import { parseConflictRegions, applyResolution } from '../utils/conflictRegions';

type Choice = 'ours' | 'theirs' | 'both';

// ---- per-region accept widgets (§3.1) -----------------------------------

/** A non-editable block widget placed at a region's `<<<<<<<` line: a toolbar
 *  with Accept Ours / Theirs / Both plus the branch-label captions. Each button
 *  re-parses the CURRENT doc, finds the region by `index` (indices are always in
 *  sync because the field rebuilds on every doc change), computes the rewrite via
 *  `applyResolution`, and dispatches a whole-doc replace — the updateListener then
 *  mirrors it into React state and the field re-parses (resolved widget vanishes,
 *  remaining regions keep correct indices). */
class RegionToolbarWidget extends WidgetType {
  constructor(
    readonly index: number,
    readonly oursLabel: string,
    readonly theirsLabel: string,
  ) {
    super();
  }

  eq(other: RegionToolbarWidget): boolean {
    return (
      other instanceof RegionToolbarWidget &&
      other.index === this.index &&
      other.oursLabel === this.oursLabel &&
      other.theirsLabel === this.theirsLabel
    );
  }

  toDOM(view: EditorView): HTMLElement {
    const bar = document.createElement('div');
    bar.className = 'conflict-region-toolbar';
    bar.setAttribute('contenteditable', 'false');

    const oursCap = document.createElement('span');
    oursCap.className = 'conflict-region-caption conflict-region-caption-ours';
    oursCap.textContent = this.oursLabel ? `Ours (${this.oursLabel})` : 'Ours';

    const theirsCap = document.createElement('span');
    theirsCap.className = 'conflict-region-caption conflict-region-caption-theirs';
    theirsCap.textContent = this.theirsLabel ? `Theirs (${this.theirsLabel})` : 'Theirs';

    const mkBtn = (label: string, choice: Choice): HTMLButtonElement => {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'btn-secondary conflict-region-btn';
      btn.textContent = label;
      // Prevent the editor stealing focus / moving selection before our click.
      btn.addEventListener('mousedown', (e) => e.preventDefault());
      btn.addEventListener('click', (e) => {
        e.preventDefault();
        this.apply(view, choice);
      });
      return btn;
    };

    bar.appendChild(oursCap);
    bar.appendChild(mkBtn('Accept Ours', 'ours'));
    bar.appendChild(mkBtn('Accept Theirs', 'theirs'));
    bar.appendChild(mkBtn('Accept Both', 'both'));
    bar.appendChild(theirsCap);
    return bar;
  }

  private apply(view: EditorView, choice: Choice): void {
    const doc = view.state.doc.toString();
    const region = parseConflictRegions(doc).find((r) => r.index === this.index);
    if (region === undefined) return;
    const next = applyResolution(doc, region, choice);
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
  }

  // CM6 default: the editor IGNORES events originating inside a widget, so they
  // are handled purely by the widget's own DOM listeners (our button `click`
  // handlers + the `mousedown` preventDefault above). Returning true keeps that
  // safe behavior for this interactive toolbar.
  ignoreEvent(): boolean {
    return true;
  }
}

const OURS_LINE = Decoration.line({ class: 'cm-conflict-ours' });
const THEIRS_LINE = Decoration.line({ class: 'cm-conflict-theirs' });

/** Rebuild the region decorations (block toolbars + tinted body lines) from the
 *  current doc. Cheap: one linear parse + one decoration per region/body line. */
function buildRegionDecorations(state: EditorState): DecorationSet {
  const regions = parseConflictRegions(state.doc.toString());
  if (regions.length === 0) return Decoration.none;

  const doc = state.doc;
  const ranges: Range<Decoration>[] = [];
  for (const region of regions) {
    // Regions are 0-based; CM lines are 1-based.
    const startFrom = doc.line(region.startLine + 1).from;
    ranges.push(
      Decoration.widget({
        widget: new RegionToolbarWidget(region.index, region.oursLabel, region.theirsLabel),
        block: true,
        side: -1,
      }).range(startFrom),
    );
    // Tint the ours body (between <<<<<<< and =======) and theirs body (between
    // ======= and >>>>>>>). Marker lines themselves are left untinted.
    for (let ln = region.startLine + 1; ln < region.sepLine; ln++) {
      ranges.push(OURS_LINE.range(doc.line(ln + 1).from));
    }
    for (let ln = region.sepLine + 1; ln < region.endLine; ln++) {
      ranges.push(THEIRS_LINE.range(doc.line(ln + 1).from));
    }
  }
  // sort=true — mixed block-widget + line decorations need CM's ordering.
  return Decoration.set(ranges, true);
}

const regionField = StateField.define<DecorationSet>({
  create: (state) => buildRegionDecorations(state),
  update: (deco, tr) => (tr.docChanged ? buildRegionDecorations(tr.state) : deco),
  provide: (f) => EditorView.decorations.from(f),
});

/** Per-region accept toolbars + body tinting (§3.1). */
export function conflictRegionWidgets(): Extension {
  return regionField;
}

// ---- scrollbar overview ruler (§3.2) ------------------------------------

/** Absolutely-positioned overlay pinned to the editor's right edge, drawing one
 *  tick per UNRESOLVED region at vertical fraction
 *  `region.startLine / max(1, totalLines - 1)`. Clicking a tick scrolls that
 *  region to center. Recomputes on doc change; renders nothing at zero regions.
 *
 *  NOTE: exact pixel placement is a USER CHECKPOINT — rAF/layout is throttled in
 *  the harness (`document.hidden`). The geometry here is per contract §3.2. */
class ConflictOverviewRuler {
  private readonly ruler: HTMLDivElement;

  constructor(private readonly view: EditorView) {
    this.ruler = document.createElement('div');
    this.ruler.className = 'conflict-overview-ruler';
    // `.cm-editor` is `position: relative`, so this pins to its right edge.
    view.dom.appendChild(this.ruler);
    this.render();
  }

  update(update: ViewUpdate): void {
    if (update.docChanged || update.geometryChanged) this.render();
  }

  private render(): void {
    const regions = parseConflictRegions(this.view.state.doc.toString());
    this.ruler.replaceChildren();
    if (regions.length === 0) {
      this.ruler.style.display = 'none';
      return;
    }
    this.ruler.style.display = 'block';

    const totalLines = this.view.state.doc.lines;
    const denom = Math.max(1, totalLines - 1);
    for (const region of regions) {
      const tick = document.createElement('div');
      tick.className = 'conflict-overview-tick';
      const frac = region.startLine / denom;
      tick.style.top = `${(frac * 100).toFixed(3)}%`;
      tick.title = `Conflict at line ${region.startLine + 1}`;
      const targetLine = region.startLine + 1;
      tick.addEventListener('mousedown', (e) => e.preventDefault());
      tick.addEventListener('click', (e) => {
        e.preventDefault();
        const pos = this.view.state.doc.line(targetLine).from;
        this.view.dispatch({ effects: EditorView.scrollIntoView(pos, { y: 'center' }) });
      });
      this.ruler.appendChild(tick);
    }
  }

  destroy(): void {
    this.ruler.remove();
  }
}

/** Custom conflict overview ruler on the editor's right edge (§3.2). */
export function conflictOverviewRuler(): Extension {
  return ViewPlugin.fromClass(ConflictOverviewRuler);
}
