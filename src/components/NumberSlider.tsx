// P51b: extracted verbatim from SettingsPanel so the Graph section
// (SettingsGraphSection) and SettingsPanel's other sliders share one control.
import { useRef, useState } from 'react';

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

/** A labeled number input + range slider bound to the same value. Clamps and
 *  ignores non-numeric input (empty field) before calling `onChange`.
 *
 *  P69c: the number input DISPLAYS a draft string while it is being typed, because
 *  binding it straight to the already-clamped setting made two-digit entry impossible —
 *  with `min: 24`, typing `3` snapped the field to `24`, so the next `0` read `240` and
 *  `30` was unreachable. Patch semantics are unchanged: every keystroke still commits
 *  the clamped value, so the live graph preview keeps updating (contract OQ-1; the
 *  commit-on-blur alternative was rejected). Transiently the field may read `3` while
 *  the setting is already `24` — that is what makes `30` typeable. */
export function NumberSlider({
  id,
  label,
  value,
  min,
  max,
  unit,
  disabled,
  describedBy,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  unit?: string;
  disabled?: boolean;
  /** P68g §1.6: id(s) of the hint paragraph(s) describing this control, wired onto
   *  the number input so the explanation is announced rather than orphaned. */
  describedBy?: string;
  onChange(next: number): void;
}) {
  /** What the number input shows while the user is typing in it; `null` = show the
   *  authoritative `value`. */
  const [draft, setDraft] = useState<string | null>(null);
  /** The last value THIS control committed, so an incoming `value` that differs can be
   *  recognised as somebody else's change. */
  const lastCommitted = useRef(value);

  const commit = (raw: string): void => {
    // An empty (or non-numeric — the platform blanks the field) input patches
    // NOTHING: `Number('')` is 0, so without this guard clearing the field would
    // silently snap the setting to `min`, which is what the doc comment above
    // always claimed but the code did not do (P68g §6.1 acceptance 5).
    if (raw.trim() === '') return;
    const n = Number(raw);
    if (Number.isNaN(n)) return;
    const next = clamp(Math.round(n), min, max);
    lastCommitted.current = next;
    onChange(next);
  };

  // Resync rule (P69c): the draft is display-only state that lives ONLY while this
  // number input is the source of the changes. It is dropped
  //   (a) on blur and on Enter — editing has ended, so the canonical value takes over;
  //   (b) when the range input is used — that is a different editor for the same value;
  //   (c) here, when `value` arrives differing from what we last committed, i.e. the
  //       change came from outside (reset-to-default, a programmatic patch, another
  //       control). Clearing state during render is the intentional derive-from-props
  //       pattern: it re-renders immediately and cannot loop, since `draft` is then null.
  // (c) compares against the last COMMITTED value, so it is blind to an external write
  // that happens to set the value we already hold (e.g. a reset while the setting is
  // already at its default): the draft would survive and the field could show text the
  // setting does not hold. Harmless today — every external writer is pointer-driven and
  // therefore blurs this input first, which is rule (a) — but it is a gap, not a proof.
  if (draft !== null && value !== lastCommitted.current) {
    lastCommitted.current = value;
    setDraft(null);
  }

  /** Any edit that is not this number input ends the draft first (rule b). */
  const commitExternal = (raw: string): void => {
    setDraft(null);
    commit(raw);
  };

  return (
    <div className={`settings-control${disabled === true ? ' is-disabled' : ''}`}>
      <label className="settings-control-label" htmlFor={id}>
        {label}
      </label>
      <div className="settings-control-inputs">
        <input
          className="settings-range"
          type="range"
          min={min}
          max={max}
          step={1}
          value={value}
          disabled={disabled}
          onChange={(e) => commitExternal(e.target.value)}
          aria-label={label}
        />
        <input
          id={id}
          className="settings-number"
          type="number"
          min={min}
          max={max}
          step={1}
          value={draft ?? String(value)}
          disabled={disabled}
          aria-describedby={describedBy}
          onChange={(e) => {
            setDraft(e.target.value);
            commit(e.target.value);
          }}
          onBlur={() => setDraft(null)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') setDraft(null);
          }}
        />
        {unit !== undefined && <span className="settings-unit">{unit}</span>}
      </div>
    </div>
  );
}
