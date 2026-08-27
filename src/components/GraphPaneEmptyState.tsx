// P43b unborn-HEAD empty state — extracted VERBATIM from WorkspaceGraphPane
// (spec-007 size offset; the pane sits at the 500-line cap). No behavior
// change: same markup, classes, and copy.
export interface GraphPaneEmptyStateProps {
  /** Bonsai style swaps the copy ("empty pot"). */
  bonsai: boolean;
  onOpenIdentitySettings(): void;
}

export function GraphPaneEmptyState({ bonsai, onOpenIdentitySettings }: GraphPaneEmptyStateProps) {
  return (
    <div className="graph-pane-empty">
      <div className="graph-pane-empty-card">
        <span className="graph-pane-empty-mark" aria-hidden="true">
          {'🌱'}
        </span>
        <p className="graph-pane-empty-title">No commits yet</p>
        <p className="pane-empty">
          {bonsai
            ? 'This branch is an empty pot — make your first commit to grow it.'
            : 'Stage your changes and write your first commit in the panel on the right — it will appear here as the root of your history.'}
        </p>
        <button
          type="button"
          className="btn-secondary graph-pane-empty-identity"
          onClick={onOpenIdentitySettings}
        >
          Set your Git identity
        </button>
      </div>
    </div>
  );
}
