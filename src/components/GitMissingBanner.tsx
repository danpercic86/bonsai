// P70: the "Git is not available" notice bar (contract docs/contracts/P70-ui.md).
//
// A direct child of `.app`, immediately after the header: git availability is a
// process-global fact, so the bar must NOT live per-tab (it would render once
// per open tab and vanish on the no-repo empty state — exactly a state a broken
// install can be stuck in). In-flow and non-dismissable, but never an overlay
// and never a focus trap: everything that still works stays reachable.
import { useEffect, useRef, useState } from 'react';

import type { GitAvailability } from '../ipc';
import type { GitAvailabilityState } from '../hooks/useGitAvailability';
import { GitBannerDetails } from './gitBanner/GitBannerDetails';
import {
  ANNOUNCE_STILL_UNAVAILABLE,
  announceAvailable,
  bannerCopy,
  buildAnnouncement,
  buildTechnicalDetails,
  checkedAtLine,
  gitAvailableToastText,
  otherRemedies,
  resolveOsFamily,
} from './gitBanner/gitBannerCopy';

export interface GitMissingBannerProps {
  git: GitAvailabilityState;
  /** Fires ONCE on a user-initiated `false → true` transition (UI §5.6). */
  onGitAvailable: (text: string) => void;
}

const DETAILS_ID = 'git-banner-details';

export function GitMissingBanner({ git, onGitAvailable }: GitMissingBannerProps) {
  const { status, checking, latched, recheck } = git;
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [checkedAt, setCheckedAt] = useState<Date | null>(null);
  const [announcement, setAnnouncement] = useState('');
  // The live region must be mounted EMPTY and populated one state-change later —
  // a live region inserted together with its content is unreliably announced.
  // Holds the last text announced for the current episode (null = not announced).
  const announced = useRef<string | null>(null);

  const visible = status !== null ? !status.found : latched;
  const os = resolveOsFamily();
  // UI §3 / §5.7: ONE copy derivation feeds the eye and the screen reader, so
  // the two structurally cannot drift.
  const copy = bannerCopy(status, os);
  const announceText = buildAnnouncement(copy);

  useEffect(() => {
    if (!visible) {
      announced.current = null;
      return;
    }
    // Re-announces only when the derived text actually changes (e.g. a latch
    // showed Variant A and the probe then landed Variant B) — never on an
    // unrelated re-render, and never over the re-check status strings below.
    if (announced.current === announceText) return;
    announced.current = announceText;
    setAnnouncement(announceText);
  }, [visible, announceText]);

  const onRecheck = () => {
    void recheck().then((next: GitAvailability | null) => {
      if (next === null) return;
      if (next.found) {
        setCheckedAt(null);
        setAnnouncement(announceAvailable(next));
        onGitAvailable(gitAvailableToastText(next));
        return;
      }
      // Failure produces NO toast — only the in-banner readout, so repeated
      // re-checks can never build a toast stack.
      setCheckedAt(new Date());
      setAnnouncement(ANNOUNCE_STILL_UNAVAILABLE);
    });
  };

  const technical = buildTechnicalDetails(status);

  // The announcer is a SIBLING of the bar, at a fixed position in the tree, so
  // showing/hiding the bar never remounts the live region (a remounted region
  // loses its pending announcement on several screen readers). Rendering only
  // the 1×1 clipped span on the healthy path keeps it at zero layout shift.
  return (
    <>
      <span className="git-banner-announce" role="status" aria-live="polite">
        {announcement}
      </span>
      {visible && (
        <section className="git-banner" role="region" aria-labelledby="git-banner-title">
          <span className="git-banner-icon" aria-hidden="true">
            ⚠
          </span>
          <div className="git-banner-text">
            <div className="git-banner-title" id="git-banner-title">
              {copy.title}
            </div>
            <div className="git-banner-sub">{copy.explanation}</div>
            <div className="git-banner-remedy">{copy.remedy}</div>
            {copy.triedPath !== null && (
              <div className="git-banner-path" title={copy.triedPath}>
                Tried: {copy.triedPath}
              </div>
            )}
          </div>
          <div className="git-banner-actions">
            <button
              type="button"
              className="btn-primary git-banner-btn"
              onClick={onRecheck}
              disabled={checking}
              aria-busy={checking || undefined}
            >
              {checking ? 'Checking…' : 'Re-check'}
            </button>
            <button
              type="button"
              className="btn-secondary git-banner-toggle"
              onClick={() => setDetailsOpen((open) => !open)}
              aria-expanded={detailsOpen}
              aria-controls={DETAILS_ID}
            >
              <span
                className={`file-chevron${detailsOpen ? ' file-chevron-open' : ''}`}
                aria-hidden="true"
              >
                ›
              </span>
              Details
            </button>
            {checkedAt !== null && (
              <div className="git-banner-checked">{checkedAtLine(checkedAt)}</div>
            )}
          </div>
          {detailsOpen && (
            <GitBannerDetails
              id={DETAILS_ID}
              remedies={otherRemedies(copy.variant, os, copy.source)}
              technical={technical}
            />
          )}
        </section>
      )}
    </>
  );
}
