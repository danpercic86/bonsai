// Spec-007 UI contract §2.2: the replay fab — third member of the graph-pane
// fab cluster (leftmost; search · filter · replay right→left). Same 30px
// circular-fab recipe as the search/filter fabs. Disabled (visible, 40%
// opacity glyph, swapped tooltip) on an empty/unborn repo — discoverability
// over tidiness.
export interface ReplayFabProps {
  disabled: boolean;
  onOpen(): void;
}

export function ReplayFab({ disabled, onOpen }: ReplayFabProps) {
  const label = disabled ? 'No commits to replay' : 'Replay history';
  return (
    <button
      type="button"
      className="graph-replay-fab"
      disabled={disabled}
      aria-label="Replay history"
      title={label}
      onClick={onOpen}
    >
      <span aria-hidden="true">▶</span>
    </button>
  );
}
