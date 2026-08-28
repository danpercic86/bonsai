// P91 §5 — the app-level "logging is active" pill.
//
// Presentational. It is mounted by `HeaderToolbar` ONLY while `dev.enabled` is
// true (§5.1: zero pixels in the default state), immediately left of the gear.
// The label is the stable word "Dev mode", never "Recording" — the word must not
// flicker as records arrive (§5.2); write-activity lives in the Settings status
// card, which can be read rather than glanced at. The record dot pulses via CSS
// while the pill is mounted; `prefers-reduced-motion` removes the pulse (the
// pill's presence, not the motion, carries "logging is on").
//
// NOTE (P91 7b): the §8.4 disk-write-error variant ("Not logging", danger dot)
// is NOT rendered here — the shipped `LogSessionInfo` carries no write-failure
// field, so there is no signal to drive it. See the increment report.

export function DevModeIndicator({ onOpen }: { onOpen(): void }) {
  return (
    <button
      type="button"
      className="dev-mode-pill"
      onClick={onOpen}
      title="Dev mode is on — Bonsai is writing a diagnostic log. Open Developer settings."
      aria-label="Dev mode is on — Bonsai is writing a diagnostic log. Open Developer settings."
    >
      <span className="dev-mode-pill-dot" aria-hidden="true" />
      <span className="dev-mode-pill-label">Dev mode</span>
    </button>
  );
}
