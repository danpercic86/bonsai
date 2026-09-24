/** P120 part a — React commit counter for `window.__bonsai.perf`. main.tsx mounts
 *  it ONLY when `VITE_MOCK_IPC=1`, so the production tree has no Profiler. */
import { Profiler } from 'react';
import type { ReactNode } from 'react';
import { countReactCommit } from './perfCounters';

export function MockProfiler({ children }: { children: ReactNode }) {
  return (
    <Profiler id="app" onRender={countReactCommit}>
      {children}
    </Profiler>
  );
}
