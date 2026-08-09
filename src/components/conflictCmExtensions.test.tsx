/** T3.7 — conflictCmExtensions: the CodeMirror region toolbars + overview ruler.
 *  Mounted as .test.tsx so it runs in jsdom (CodeMirror needs a DOM). Asserts the
 *  widgets derive from the live doc: one toolbar per region, Accept-Ours rewrites
 *  the doc via applyResolution (marker count drops), and the ruler draws one tick
 *  per region and none when clean. */
import { describe, it, expect, afterEach } from 'vitest';
import { EditorView } from '@codemirror/view';
import { EditorState } from '@codemirror/state';
import { conflictRegionWidgets, conflictOverviewRuler } from './conflictCmExtensions';
import { parseConflictRegions } from '../utils/conflictRegions';

const TWO_REGIONS = [
  'top',
  '<<<<<<< HEAD',
  'a-ours',
  '=======',
  'a-theirs',
  '>>>>>>> branch-a',
  'middle',
  '<<<<<<< HEAD',
  'b-ours',
  '=======',
  'b-theirs',
  '>>>>>>> branch-b',
  'bottom',
].join('\n');

let view: EditorView | null = null;

function mount(doc: string): EditorView {
  const host = document.createElement('div');
  document.body.appendChild(host);
  view = new EditorView({
    parent: host,
    state: EditorState.create({
      doc,
      extensions: [conflictRegionWidgets(), conflictOverviewRuler()],
    }),
  });
  return view;
}

afterEach(() => {
  view?.destroy();
  view = null;
  document.body.innerHTML = '';
});

describe('conflictRegionWidgets', () => {
  it('renders one accept toolbar per region with the branch-label captions', () => {
    const v = mount(TWO_REGIONS);
    const toolbars = v.dom.querySelectorAll('.conflict-region-toolbar');
    expect(toolbars).toHaveLength(2);
    // Each toolbar carries the three accept buttons.
    expect(v.dom.querySelectorAll('.conflict-region-btn')).toHaveLength(6);
    expect(v.dom.textContent).toContain('Ours (HEAD)');
    expect(v.dom.textContent).toContain('Theirs (branch-a)');
  });

  it('Accept Ours rewrites the doc, dropping that region', () => {
    const v = mount(TWO_REGIONS);
    expect(parseConflictRegions(v.state.doc.toString())).toHaveLength(2);
    // First toolbar's "Accept Ours" (buttons are Ours/Theirs/Both per region).
    const firstOurs = v.dom.querySelectorAll<HTMLButtonElement>('.conflict-region-btn')[0];
    firstOurs.click();
    const after = v.state.doc.toString();
    expect(parseConflictRegions(after)).toHaveLength(1);
    expect(after).toContain('a-ours');
    expect(after).not.toContain('a-theirs');
    // The surviving region keeps its own bodies.
    expect(after).toContain('b-ours');
    expect(after).toContain('b-theirs');
  });

  it('renders no toolbar for a marker-free doc', () => {
    const v = mount('plain\nfile\ncontents\n');
    expect(v.dom.querySelectorAll('.conflict-region-toolbar')).toHaveLength(0);
  });
});

describe('conflictOverviewRuler', () => {
  it('draws one tick per region', () => {
    const v = mount(TWO_REGIONS);
    const ruler = v.dom.querySelector('.conflict-overview-ruler') as HTMLElement;
    expect(ruler).not.toBeNull();
    expect(ruler.style.display).toBe('block');
    expect(ruler.querySelectorAll('.conflict-overview-tick')).toHaveLength(2);
  });

  it('hides the ruler when there are no regions', () => {
    const v = mount('no conflicts here\n');
    const ruler = v.dom.querySelector('.conflict-overview-ruler') as HTMLElement;
    expect(ruler.style.display).toBe('none');
    expect(ruler.querySelectorAll('.conflict-overview-tick')).toHaveLength(0);
  });
});
