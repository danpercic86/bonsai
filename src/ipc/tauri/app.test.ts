/** P124d — the Tauri adapter gates `onFocusChanged(true)`: WebView2 delivers
 *  two within ~22–47 ms and each used to run a full refresh round. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type FocusHandler = (e: { payload: boolean }) => void;
const handlers: FocusHandler[] = [];

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onFocusChanged: (h: FocusHandler) => {
      handlers.push(h);
      return Promise.resolve(() => {});
    },
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: () => Promise.resolve(null), Channel: class {} }));

const { appCommands } = await import('./app');

beforeEach(() => {
  handlers.length = 0;
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe('tauri onWindowFocus focus gate (P124d)', () => {
  it('two focused=true deliveries 47 ms apart fire the callback once', async () => {
    const cb = vi.fn();
    await appCommands.onWindowFocus(cb);
    expect(handlers).toHaveLength(1);
    const h = handlers[0];
    h({ payload: true });
    vi.advanceTimersByTime(47);
    h({ payload: true });
    expect(cb).toHaveBeenCalledTimes(1);
  });

  it('focused=false never fires, and a refocus after the window fires again', async () => {
    const cb = vi.fn();
    await appCommands.onWindowFocus(cb);
    const h = handlers[0];
    h({ payload: false });
    expect(cb).not.toHaveBeenCalled();
    h({ payload: true });
    vi.advanceTimersByTime(300);
    h({ payload: true });
    expect(cb).toHaveBeenCalledTimes(2);
  });
});
