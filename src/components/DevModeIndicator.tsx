// P91 §5 — the app-level "logging is active" pill.
//
// Presentational. Mounted (via `DevModePill`) by `HeaderToolbar` ONLY while
// `dev.enabled` is true (§5.1: zero pixels in the default state), immediately
// left of the gear. The healthy label is the stable word "Dev mode", never
// "Recording" — the word must not flicker as records arrive (§5.2); write-activity
// lives in the Settings status card, which can be read rather than glanced at. The
// record dot pulses via CSS while the pill is mounted; `prefers-reduced-motion`
// removes the pulse (the pill's presence, not the motion, carries "logging is on").
//
// §8.4 disk-write-error variant: when `writeFailed`, the dot becomes a `--danger`
// triangle and the label becomes "Not logging" — the pill must never claim to be
// recording when the sink has stopped reaching disk. Generic copy only; the
// underlying error string (with its path) never crosses IPC.

const HEALTHY_LABEL = 'Dev mode is on — Bonsai is writing a diagnostic log. Open Developer settings.';
const FAILED_LABEL = 'Dev mode is on but Bonsai stopped writing the log. Open Developer settings.';

export function DevModeIndicator({
  onOpen,
  writeFailed = false,
}: {
  onOpen(): void;
  writeFailed?: boolean;
}) {
  const label = writeFailed ? FAILED_LABEL : HEALTHY_LABEL;
  return (
    <button
      type="button"
      className={`dev-mode-pill${writeFailed ? ' is-write-failed' : ''}`}
      onClick={onOpen}
      title={label}
      aria-label={label}
    >
      {writeFailed ? (
        <span className="dev-mode-pill-glyph" aria-hidden="true">
          ▲
        </span>
      ) : (
        <span className="dev-mode-pill-dot" aria-hidden="true" />
      )}
      <span className="dev-mode-pill-label">{writeFailed ? 'Not logging' : 'Dev mode'}</span>
    </button>
  );
}
