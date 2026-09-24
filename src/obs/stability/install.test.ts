// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { startStability, stopStability } from './install';
import { stabilitySinkCountForTests } from './sinks';

afterEach(() => {
  stopStability();
  vi.restoreAllMocks();
});

describe('startStability outside mock mode (part a)', () => {
  it('registers no sink, so no observer or listener is attached; idempotent', () => {
    expect(import.meta.env.VITE_MOCK_IPC).not.toBe('1');
    const add = vi.spyOn(window, 'addEventListener');
    startStability();
    startStability();
    expect(stabilitySinkCountForTests()).toBe(0);
    expect(add).not.toHaveBeenCalled();
  });
});
