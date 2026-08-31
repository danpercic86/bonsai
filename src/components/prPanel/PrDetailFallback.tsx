// P93: the PR detail shell shown while the detail payload is still loading, or
// when loading it failed. Extracted from PrPanel (presentational-only split, to
// keep that container under the ~500-line limit): a Back button plus either the
// dismissible error banner or the skeleton rows.

import { SkeletonRows } from '../CommitPanel';

export interface PrDetailFallbackProps {
  /** Detail-load failure message, or null while still loading. */
  detailError: string | null;
  onBack(): void;
}

export function PrDetailFallback({ detailError, onBack }: PrDetailFallbackProps) {
  return (
    <div className="pr-detail pr-detail-shell">
      <div className="pr-detail-header pr-detail-title-row">
        <button type="button" className="section-action pr-back-button" onClick={onBack}>
          {'← Pull requests'}
        </button>
      </div>
      {detailError !== null ? (
        <div className="error-banner error-banner-dismissible pr-error" role="alert">
          <span className="error-banner-text">{detailError}</span>
        </div>
      ) : (
        <div className="pr-detail-loading">
          <SkeletonRows />
        </div>
      )}
    </div>
  );
}
