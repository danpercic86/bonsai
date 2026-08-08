import type { PrState, PrSummary } from '../ipc';

// P62c: one presentational PR row. Selectable button; no data fetching. The
// container (PrPanel) owns selection + loading.

const STATE_LABEL: Record<PrState, string> = {
  open: 'Open',
  merged: 'Merged',
  closed: 'Closed',
};

export interface PrListItemProps {
  pr: PrSummary;
  selected: boolean;
  onSelect(number: number): void;
}

export function PrListItem({ pr, selected, onSelect }: PrListItemProps) {
  return (
    <button
      type="button"
      className={`pr-row${selected ? ' selected' : ''}`}
      aria-pressed={selected}
      onClick={() => onSelect(pr.number)}
    >
      <div className="pr-row-top">
        <span className={`pr-state-pill pr-state-${pr.state}`}>{STATE_LABEL[pr.state]}</span>
        {pr.isDraft && <span className="pr-draft-tag">Draft</span>}
        <span className="pr-row-title">{pr.title}</span>
        <span className="pr-row-num mono">{`#${pr.number}`}</span>
      </div>
      <div className="pr-row-meta">
        <span className="pr-row-author">{pr.author}</span>
        <span
          className="pr-row-branches mono"
          title={`${pr.sourceBranch} → ${pr.targetBranch}`}
        >
          {`${pr.sourceBranch} → ${pr.targetBranch}`}
        </span>
        {pr.comments > 0 && (
          <span
            className="pr-row-comments"
            title={`${pr.comments} comment${pr.comments === 1 ? '' : 's'}`}
          >
            {`💬 ${pr.comments}`}
          </span>
        )}
      </div>
    </button>
  );
}
