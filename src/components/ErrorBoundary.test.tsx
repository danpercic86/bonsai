/** T3.3a — ErrorBoundary: a child throw renders the fallback (no rethrow),
 *  the label names the pane, custom fallback wins, and Try again re-mounts. */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ErrorBoundary } from './ErrorBoundary';

let consoleSpy: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  // React logs caught render errors loudly; keep test output clean and assert on it.
  consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
});
afterEach(() => {
  consoleSpy.mockRestore();
});

function Bomb({ when }: { when: boolean }) {
  if (when) throw new Error('kaboom');
  return <div>alive</div>;
}

describe('ErrorBoundary', () => {
  it('renders children when nothing throws', () => {
    render(
      <ErrorBoundary>
        <Bomb when={false} />
      </ErrorBoundary>,
    );
    expect(screen.getByText('alive')).toBeInTheDocument();
  });

  it('a throwing child renders the default fallback instead of propagating', () => {
    expect(() =>
      render(
        <ErrorBoundary>
          <Bomb when />
        </ErrorBoundary>,
      ),
    ).not.toThrow();
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
    expect(screen.getByText('kaboom')).toBeInTheDocument();
    // The boundary logged it with the bonsai prefix.
    expect(
      consoleSpy.mock.calls.some((c: unknown[]) => String(c[0]).includes('[bonsai] ErrorBoundary')),
    ).toBe(true);
  });

  it('label names the failed pane in the fallback title', () => {
    render(
      <ErrorBoundary label="Graph">
        <Bomb when />
      </ErrorBoundary>,
    );
    expect(screen.getByText('Graph failed to render')).toBeInTheDocument();
  });

  it('custom fallback receives the error and the reset callback', () => {
    render(
      <ErrorBoundary
        fallback={(error, reset) => (
          <div>
            <span>custom: {error.message}</span>
            <button type="button" onClick={reset}>
              retry
            </button>
          </div>
        )}
      >
        <Bomb when />
      </ErrorBoundary>,
    );
    expect(screen.getByText('custom: kaboom')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'retry' })).toBeInTheDocument();
  });

  it('Try again clears the boundary and re-renders a now-healthy subtree', () => {
    let shouldThrow = true;
    function Flaky() {
      if (shouldThrow) throw new Error('transient');
      return <div>recovered</div>;
    }
    render(
      <ErrorBoundary>
        <Flaky />
      </ErrorBoundary>,
    );
    expect(screen.getByText('transient')).toBeInTheDocument();
    shouldThrow = false;
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(screen.getByText('recovered')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
