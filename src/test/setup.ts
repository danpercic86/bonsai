import '@testing-library/jest-dom/vitest';
import { afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';

afterEach(() => {
  cleanup();
  localStorage.clear(); // the DOM env provides localStorage; keep tests isolated
});

// Canvas 2D stub — neither jsdom nor happy-dom has a canvas backend;
// GraphCanvas et al. need a
// tolerant 2D context. Proxy returns no-op fns for anything not overridden.
const ctx2d = new Proxy(
  {
    canvas: null as unknown,
    measureText: (s: string) => ({ width: s.length * 7 }),
    getImageData: () => ({ data: new Uint8ClampedArray(4), width: 1, height: 1 }),
    createLinearGradient: () => ({ addColorStop: () => {} }),
  },
  {
    get: (t, p) => (p in t ? (t as any)[p] : () => undefined),
    set: () => true,
  },
);
HTMLCanvasElement.prototype.getContext = vi.fn(() => ctx2d) as never;

// scrollIntoView stub — the DOM envs have no layout, so keyboard-nav "keep the active
// row visible" effects (CommandPalette, Combobox) need a no-op.
Element.prototype.scrollIntoView ??= () => {};

// ResizeObserver stub
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver = ResizeObserverStub as never;

// matchMedia stub (theme detection)
window.matchMedia ??= ((query: string) => ({
  matches: false,
  media: query,
  onchange: null,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
  dispatchEvent: () => false,
})) as never;

// ---------------------------------------------------------------------------
// happy-dom UA-stylesheet shim (EXPERIMENT — see report).
// happy-dom's getComputedStyle omits `display` for every element whose UA
// default is `inline`/`table-cell`, and omits `visibility` for all elements
// (jsdom returns "inline"/"visible"). `dom-accessibility-api` branches on
// `display` to decide whether to insert a space between child text
// alternatives, so without this shim every accessible name computed across
// nested inline elements gains spurious spaces.
const UA_INLINE = new Set([
  'A','ABBR','B','BDI','BDO','BIG','BR','CITE','CODE','DATA','DEL','DFN','EM','I',
  'IMG','INS','KBD','LABEL','MAP','MARK','OUTPUT','Q','RUBY','S','SAMP','SMALL',
  'SPAN','STRONG','SUB','SUP','TIME','U','VAR','WBR','SVG','PATH','G','CIRCLE',
  'RECT','LINE','POLYLINE','POLYGON','TEXT','TSPAN','USE','DEFS','SELECT',
]);
const UA_DISPLAY: Record<string, string> = { TD: 'table-cell', TH: 'table-cell' };

function uaDisplay(el: Element): string | undefined {
  const tag = el.tagName.toUpperCase();
  if (UA_DISPLAY[tag]) return UA_DISPLAY[tag];
  if (UA_INLINE.has(tag)) return 'inline';
  return undefined;
}

const nativeGetComputedStyle = window.getComputedStyle.bind(window);

// Only patch an environment that actually has the gap: jsdom already reports
// the UA defaults, and wrapping it there would mask genuine `''` results.
const probe = document.createElement('span');
document.documentElement.appendChild(probe);
const NEEDS_UA_SHIM = nativeGetComputedStyle(probe).display === '';
probe.remove();

if (NEEDS_UA_SHIM) {
window.getComputedStyle = ((el: Element, pseudo?: string | null) => {
  const decl = nativeGetComputedStyle(el, pseudo as never);
  const patch = (prop: string, raw: string): string => {
    if (raw !== '') return raw;
    if (prop === 'visibility') return 'visible';
    if (prop === 'display') return uaDisplay(el) ?? 'block';
    return raw;
  };
  return new Proxy(decl, {
    get(target, prop, recv) {
      if (prop === 'getPropertyValue') {
        return (name: string) => patch(name, target.getPropertyValue(name));
      }
      const v = Reflect.get(target, prop, recv);
      if (typeof v === 'function') return v.bind(target);
      if (typeof prop === 'string' && typeof v === 'string') return patch(prop, v);
      return v;
    },
  });
}) as typeof window.getComputedStyle;
globalThis.getComputedStyle = window.getComputedStyle;
}
