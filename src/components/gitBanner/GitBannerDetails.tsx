// P70 UI §5.3–§5.5: the notice bar's disclosure region. Presentational — every
// string arrives resolved from `gitBannerCopy`; the container owns open/closed.
import { useEffect, useState } from 'react';

import { CAPABILITY_ROWS } from './gitBannerCopy';

export interface GitBannerDetailsProps {
  id: string;
  /** Secondary remedies (the headline one stays in the collapsed row). */
  remedies: string[];
  /** The paste-into-a-bug-report block; `null` ⇒ the whole section is omitted,
   *  because there is nothing truthful to put in it yet (§6, latch-before-probe). */
  technical: string | null;
}

export function GitBannerDetails({ id, remedies, technical }: GitBannerDetailsProps) {
  // Transient "Copied" confirmation, self-cleaning (matches AiOutputPanel).
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const t = window.setTimeout(() => setCopied(false), 1200);
    return () => window.clearTimeout(t);
  }, [copied]);

  const onCopy = () => {
    if (technical === null) return;
    // Silent on failure by design: a clipboard error must not add a toast to a
    // surface whose whole purpose is to REPLACE toast noise.
    const p =
      navigator.clipboard?.writeText(technical) ??
      Promise.reject(new Error('Clipboard unavailable'));
    void p.then(() => setCopied(true)).catch(() => setCopied(false));
  };

  return (
    <div className="git-banner-details" id={id}>
      <div className="git-banner-section">
        <div className="git-banner-label">OTHER THINGS TO TRY</div>
        <ul className="git-banner-list">
          {remedies.map((remedy) => (
            <li key={remedy}>
              <span className="git-banner-bullet" aria-hidden="true">
                ·
              </span>
              {remedy}
            </li>
          ))}
        </ul>
      </div>

      <div className="git-banner-section">
        <div className="git-banner-label">WHILE GIT IS MISSING</div>
        {CAPABILITY_ROWS.map((row) => (
          <div key={row.tone} className={`git-banner-cap git-banner-cap-${row.tone}`}>
            <span className="git-banner-cap-glyph" aria-hidden="true">
              {row.tone === 'works' ? '✓' : '✕'}
            </span>
            <span className="git-banner-cap-leader">{row.leader}</span> {row.text}
          </div>
        ))}
      </div>

      {technical !== null && (
        <div className="git-banner-section">
          <div className="git-banner-label-row">
            <div className="git-banner-label">TECHNICAL DETAILS</div>
            <button type="button" className="btn-secondary git-banner-copy" onClick={onCopy}>
              {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
          <div className="git-banner-tech">{technical}</div>
        </div>
      )}
    </div>
  );
}
