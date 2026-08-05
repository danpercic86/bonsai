import { Component } from 'react';
import type { ErrorInfo, ReactNode } from 'react';

export interface ErrorBoundaryProps {
  /** Names which pane/subtree this boundary guards (shown in the fallback +
   *  console label). */
  label?: string;
  children: ReactNode;
  /** Custom fallback renderer; receives the caught error and a reset callback
   *  that clears the boundary so the subtree re-mounts. */
  fallback?: (error: Error, reset: () => void) => ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

/**
 * Recoverable render-error boundary (T0.4). A throw anywhere in the wrapped
 * subtree is caught here — instead of white-screening the whole app — logged to
 * the console, and replaced with a small fallback offering a "Try again" reset.
 * Wrap the app root once, then each heavy pane (graph canvas, diff view,
 * conflict editor) so a failure stays contained to that pane.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    const where = this.props.label !== undefined ? ` [${this.props.label}]` : '';
    console.error(`[bonsai] ErrorBoundary${where} caught a render error`, error, info.componentStack);
  }

  private reset = (): void => {
    this.setState({ error: null });
  };

  render(): ReactNode {
    const { error } = this.state;
    if (error === null) return this.props.children;

    const { fallback, label } = this.props;
    if (fallback !== undefined) return fallback(error, this.reset);

    return (
      <div className="error-boundary" role="alert">
        <p className="error-boundary-title">
          {label !== undefined ? `${label} failed to render` : 'Something went wrong'}
        </p>
        <p className="error-boundary-message mono">{error.message}</p>
        <button
          type="button"
          className="btn-secondary error-boundary-retry"
          onClick={this.reset}
        >
          Try again
        </button>
      </div>
    );
  }
}
