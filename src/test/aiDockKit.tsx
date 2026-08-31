/**
 * P68e shared fixtures for the AI-dock component tests (split per the ~500-line rule,
 * the same way `aiRunsKit.ts` serves the `useAiRuns` files).
 *
 * Lives in `src/test/` so it stays out of coverage. It holds ONLY data + a render
 * helper: every assertion stays in the test file that owns its concern.
 */
import { vi } from 'vitest';
import { render } from '@testing-library/react';

import { AiActivityPanel } from '../components/AiActivityPanel';
import { classifyLogLine } from '../components/aiDockFormat';
import type {
  AiActivityPanelProps,
  AiActivityRun,
} from '../components/aiDockFormat';
import type { AiRunLogLine } from '../components/repoWorkspace/useAiRuns';

/** One log line with the store's own `kind` classification applied. */
export function line(seq: number, text: string): AiRunLogLine {
  return { seq, text, kind: classifyLogLine(text) };
}

export function run(over: Partial<AiActivityRun> = {}): AiActivityRun {
  return {
    key: 'conflict:src/locales/de.json',
    label: 'src/locales/de.json',
    status: 'running',
    elapsedMs: 67_000,
    costUsd: null,
    question: null,
    error: null,
    partialText: null,
    log: [],
    logDropped: 0,
    files: [],
    paths: ['src/locales/de.json'],
    cancelRequested: false,
    turn: 0,
    thinkingTokens: null,
    openedInPane: false,
    ...over,
  };
}

export function props(over: Partial<AiActivityPanelProps> = {}): AiActivityPanelProps {
  return {
    runs: [run()],
    activeKey: null,
    onSelectRun: vi.fn(),
    collapsed: false,
    onToggleCollapsed: vi.fn(),
    height: 180,
    onResizeHeight: vi.fn(),
    onCancel: vi.fn(),
    onReply: vi.fn(),
    onDismiss: vi.fn(),
    onReviewFile: vi.fn(),
    onRetryFile: vi.fn(),
    density: 'cozy',
    streamLogEnabled: true,
    atCapacity: false,
    ...over,
  };
}

/** Render the panel and hand back the props that were used, so a test can assert on
 *  the exact `vi.fn()` instances it was given. */
export function mount(over: Partial<AiActivityPanelProps> = {}) {
  const p = props(over);
  const view = render(<AiActivityPanel {...p} />);
  return { ...view, p };
}
