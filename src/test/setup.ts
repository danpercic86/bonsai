import '@testing-library/jest-dom/vitest';
import { afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';

afterEach(() => {
  cleanup();
  localStorage.clear(); // jsdom provides localStorage; keep tests isolated
});

// Canvas 2D stub — jsdom has no canvas backend; GraphCanvas et al. need a
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
