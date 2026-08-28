// P91 §6 / §16.5 — the live session status card (`dev.session-info`).
//
// Presentational: a `role="group"` READOUT, not a settings row — it has five
// live facts and no control, so it is catalogued as `control: 'group'`. The dot's
// colour never carries the state alone: the word beside it (`Recording` / `Idle`)
// does. `salt` is never rendered (§16.1). The truncation line (§16.5, driven by
// `droppedParts > 0`) is a DISTINCT signal from the in-memory `dropped` line and
// the two never merge.

import type { LogSessionInfo } from '../../ipc';
import { formatBytes } from '../../utils/format';

const NUM = new Intl.NumberFormat();

/** The status-card heading id — the `aria-labelledby` target (must read
 *  "Logging status", the catalog label for `dev.session-info`). */
const HEADING_ID = 'dev-session-info-title';

function plural(n: number, one: string, many: string): string {
  return `${NUM.format(n)} ${n === 1 ? one : many}`;
}

export function DevSessionStatus({
  info,
  enabled,
}: {
  info: LogSessionInfo | null;
  enabled: boolean;
}) {
  const rawNames = info?.redaction === 'raw';

  let body;
  if (!enabled) {
    // Dev mode off: the files deliberately persist, so say so.
    body =
      info !== null && info.totalFiles > 0 ? (
        <p className="dev-status-line dev-status-idle">
          {`Idle · last session: ${plural(info.totalFiles, 'file', 'files')} · ${formatBytes(
            info.totalBytes,
          )} · kept on this computer`}
        </p>
      ) : (
        <p className="dev-status-line dev-status-empty">No logs yet.</p>
      );
  } else if (info === null) {
    body = <p className="dev-status-line dev-status-empty">Starting…</p>;
  } else {
    const recording = info.records > 0;
    const facts: string[] = [
      plural(info.files.length, 'file', 'files'),
      formatBytes(info.bytes),
      plural(info.records, 'record', 'records'),
    ];
    const fileName = info.files[0] ?? info.sessionId;
    body = (
      <>
        <p className="dev-status-line">
          <span
            className={`dev-status-dot${recording ? ' is-recording' : ''}`}
            aria-hidden="true"
          />
          <span className="dev-status-state">{recording ? 'Recording' : 'Idle'}</span>
          <span className="dev-status-facts">
            {' · '}
            {facts.join(' · ')}
            {info.anomalies > 0 && (
              <span
                className="dev-status-flagged"
                title="Possible problems Bonsai noticed and marked in the log"
              >
                {` · ${NUM.format(info.anomalies)} flagged`}
              </span>
            )}
            {info.dropped > 0 && (
              <span
                className="dev-status-dropped"
                title="The log was written faster than the disk could keep up. Timing in the file may be incomplete."
              >
                {` · ${plural(info.dropped, 'record dropped', 'records dropped')}`}
              </span>
            )}
          </span>
        </p>
        <p className="dev-status-file mono" title={fileName}>
          {fileName}
        </p>
        {rawNames && (
          <p className="dev-status-warn dev-warning-bar">
            <span className="dev-warning-glyph" aria-hidden="true">
              ⚠
            </span>
            Raw names are on. Read an exported log before sending it to anyone.
          </p>
        )}
        {info.droppedParts > 0 && (
          <p className="dev-status-warn dev-warning-bar">
            <span className="dev-warning-glyph" aria-hidden="true">
              ⚠
            </span>
            This session reached its size limit, so its earliest records were removed. Lower the
            detail level or turn off captures you don&rsquo;t need, then reproduce the problem in a
            shorter session.
          </p>
        )}
      </>
    );
  }

  return (
    <div
      className="dev-status-card"
      role="group"
      aria-labelledby={HEADING_ID}
      data-setting-id="dev.session-info"
    >
      <h5 className="sr-only" id={HEADING_ID}>
        Logging status
      </h5>
      {body}
    </div>
  );
}
