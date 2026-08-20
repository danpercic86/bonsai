/** P69c — the shared Settings number+range control.
 *
 *  The defect these tests pin: the number input used to render the already-clamped
 *  setting, so its text snapped to `min` mid-typing and the next keystroke appended to
 *  the snapped text (`min: 60` → typing `6`,`0` produced `600`; `min: 24` → `240`). The
 *  fix is a draft DISPLAY string with an unchanged clamped commit per keystroke, so the
 *  live graph preview is preserved (contract OQ-1).
 *
 *  Most cases use a CONTROLLED harness, because the bug only reproduces when the parent
 *  feeds the clamped value straight back — which is what Settings does.
 */
import { StrictMode, useState } from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { NumberSlider } from './NumberSlider';

interface HarnessProps {
  initial: number;
  min: number;
  max: number;
  onCommit: (n: number) => void;
  /** An out-of-band writer, standing in for reset-to-default or a programmatic patch. */
  external?: number;
}

function Harness({ initial, min, max, onCommit, external }: HarnessProps) {
  const [value, setValue] = useState(initial);
  return (
    <>
      <NumberSlider
        id="ns-field"
        label="Row height"
        value={value}
        min={min}
        max={max}
        unit="px"
        describedBy="ns-hint"
        onChange={(n) => {
          onCommit(n);
          setValue(n);
        }}
      />
      {external !== undefined && (
        <button type="button" onClick={() => setValue(external)}>
          reset
        </button>
      )}
    </>
  );
}

function mount(opts: {
  initial: number;
  min: number;
  max: number;
  external?: number;
  /** Render under `<StrictMode>`, which double-invokes render — the environment the real
   *  app uses (`src/main.tsx`) and the one that punishes the render-phase `setDraft`. */
  strict?: boolean;
}) {
  const { strict = false, ...harness } = opts;
  const onCommit = vi.fn<(n: number) => void>();
  const tree = <Harness {...harness} onCommit={onCommit} />;
  render(strict ? <StrictMode>{tree}</StrictMode> : tree);
  const field = screen.getByRole('spinbutton');
  return { onCommit, field };
}

/** The first keystroke into a focused field whose content is selected: it REPLACES the
 *  text (how a user retypes a settings value). */
function typeFirst(field: HTMLElement, ch: string): void {
  fireEvent.change(field, { target: { value: ch } });
}

/** A later keystroke: it appends to whatever the field is NOW showing. This is the
 *  mechanism the old component broke — it re-rendered the clamped setting into the field,
 *  so the second keystroke appended to `min` (`6` → `60` → `600`). */
function typeMore(field: HTMLElement, ch: string): void {
  fireEvent.change(field, { target: { value: (field as HTMLInputElement).value + ch } });
}

describe('NumberSlider — two-digit entry (the reported defect)', () => {
  it('min 60: typing 6 then 0 ends at 60, not 600', () => {
    const { onCommit, field } = mount({ initial: 120, min: 60, max: 400 });

    typeFirst(field, '6');
    // The field must NOT have snapped to the clamped 60, or the next keystroke reads 600.
    expect(field).toHaveValue(6);
    expect(onCommit).toHaveBeenLastCalledWith(60);

    typeMore(field, '0');
    expect(field).toHaveValue(60);
    expect(onCommit).toHaveBeenLastCalledWith(60);
    expect(onCommit).not.toHaveBeenCalledWith(600);
  });

  it('min 24: typing 3 then 0 ends at 30', () => {
    const { onCommit, field } = mount({ initial: 28, min: 24, max: 200 });

    typeFirst(field, '3');
    expect(field).toHaveValue(3);
    expect(onCommit).toHaveBeenLastCalledWith(24);
    // The asymmetry is deliberate: only the NUMBER input shows the draft. The range keeps
    // reflecting the authoritative (clamped) value. (The blank-draft case below is what
    // actually discriminates the two, since a range input sanitizes an out-of-range draft
    // to the same clamp the setting already applied.)
    expect(screen.getByRole('slider')).toHaveValue('24');

    typeMore(field, '0');
    expect(field).toHaveValue(30);
    expect(onCommit).toHaveBeenLastCalledWith(30);
    expect(onCommit).not.toHaveBeenCalledWith(240);
  });
});

describe('NumberSlider — commit semantics are unchanged', () => {
  it('commits the clamped value on EVERY keystroke, keeping the live preview', () => {
    const { onCommit, field } = mount({ initial: 28, min: 24, max: 200 });
    typeFirst(field, '3');
    typeMore(field, '5');
    // One patch per keystroke: '3' → 24 (clamped), '35' → 35.
    expect(onCommit.mock.calls).toEqual([[24], [35]]);
  });

  it('clamps above max and below min on commit', () => {
    const { onCommit, field } = mount({ initial: 28, min: 24, max: 200 });
    fireEvent.change(field, { target: { value: '99999' } });
    expect(onCommit).toHaveBeenLastCalledWith(200);
    fireEvent.change(field, { target: { value: '1' } });
    expect(onCommit).toHaveBeenLastCalledWith(24);
  });

  it('rounds a fractional entry', () => {
    const { onCommit, field } = mount({ initial: 28, min: 24, max: 200 });
    fireEvent.change(field, { target: { value: '30.6' } });
    expect(onCommit).toHaveBeenLastCalledWith(31);
  });

  it('a cleared field patches NOTHING and shows empty (P68g §6.1 acceptance 5)', () => {
    const { onCommit, field } = mount({ initial: 28, min: 24, max: 200 });
    fireEvent.change(field, { target: { value: '' } });
    expect(onCommit).not.toHaveBeenCalled();
    // The blank must be visible: snapping back to 28 here is the old bug in reverse.
    expect((field as HTMLInputElement).value).toBe('');
    // ...but ONLY in the number input. Binding the range to the draft too (a plausible
    // "unify the two inputs" refactor) would push a blank into it, which a range input
    // sanitizes to the middle of its track — a thumb that lies about the setting.
    expect(screen.getByRole('slider')).toHaveValue('28');
  });

  it('non-numeric input (which the platform blanks) patches nothing', () => {
    const { onCommit, field } = mount({ initial: 28, min: 24, max: 200 });
    fireEvent.change(field, { target: { value: 'abc' } });
    expect(onCommit).not.toHaveBeenCalled();
  });
});

describe('NumberSlider — the draft never outlives editing', () => {
  it('blur drops the draft and restores the authoritative value', () => {
    const { field } = mount({ initial: 28, min: 24, max: 200 });
    typeFirst(field, '3');
    expect(field).toHaveValue(3);
    fireEvent.blur(field);
    // The setting is 24 (the clamped commit), so that is what must be shown.
    expect(field).toHaveValue(24);
  });

  it('blurring a blank field restores the current value', () => {
    const { onCommit, field } = mount({ initial: 28, min: 24, max: 200 });
    fireEvent.change(field, { target: { value: '' } });
    fireEvent.blur(field);
    expect(field).toHaveValue(28);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it('Enter drops the draft', () => {
    const { field } = mount({ initial: 28, min: 24, max: 200 });
    typeFirst(field, '3');
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(field).toHaveValue(24);
  });

  it('an external value change (reset-to-default) drops the draft without a blur', () => {
    const { field } = mount({ initial: 28, min: 24, max: 200, external: 40 });
    typeFirst(field, '3');
    expect(field).toHaveValue(3);
    fireEvent.click(screen.getByRole('button', { name: 'reset' }));
    expect(field).toHaveValue(40);
  });

  it('under StrictMode: the draft still displays, and an external change still drops it', () => {
    // The resync is a render-phase `setDraft(null)`, so StrictMode's double-invoked render
    // is the case that would expose a non-convergent or draft-eating implementation.
    const { onCommit, field } = mount({ initial: 28, min: 24, max: 200, external: 40, strict: true });

    typeFirst(field, '3');
    expect(field).toHaveValue(3);
    expect(onCommit).toHaveBeenLastCalledWith(24);

    // Second keystroke still appends to `3`, i.e. the draft was not eaten by the extra render.
    typeMore(field, '0');
    expect(field).toHaveValue(30);
    expect(onCommit).toHaveBeenLastCalledWith(30);

    typeFirst(field, '3');
    fireEvent.click(screen.getByRole('button', { name: 'reset' }));
    expect(field).toHaveValue(40);
  });

  it('using the range slider drops the draft and commits per change', () => {
    const { onCommit, field } = mount({ initial: 28, min: 24, max: 200 });
    typeFirst(field, '3');
    const range = screen.getByRole('slider', { name: 'Row height' });
    fireEvent.change(range, { target: { value: '96' } });
    expect(onCommit).toHaveBeenLastCalledWith(96);
    expect(field).toHaveValue(96);
  });
});

describe('NumberSlider — plumbing that e2e depends on', () => {
  it('keeps the id on the number input, the range aria-label, unit and describedby', () => {
    render(
      <NumberSlider
        id="settings-graph-row"
        label="Row height"
        value={28}
        min={24}
        max={200}
        unit="px"
        describedBy="ns-hint"
        onChange={vi.fn()}
      />,
    );
    const field = document.getElementById('settings-graph-row');
    expect(field).toBeInstanceOf(HTMLInputElement);
    expect(field).toHaveAttribute('type', 'number');
    expect(field).toHaveAttribute('aria-describedby', 'ns-hint');
    // P69k: the control renders ONLY the two inputs — the number input's name is
    // the owning `SettingsRow`'s `<label for={id}>`, and `label` survives as the
    // RANGE input's own name (a slider and a spinbutton in one row both need one).
    expect(screen.getByRole('slider')).toHaveAttribute('aria-label', 'Row height');
    expect(document.querySelector('.settings-control')).toBeNull();
    expect(screen.getByText('px')).toBeInTheDocument();
  });

  it('disabled: both inputs are inert', () => {
    const onChange = vi.fn();
    render(
      <NumberSlider
        id="ns-field"
        label="Row height"
        value={28}
        min={24}
        max={200}
        disabled
        onChange={onChange}
      />,
    );
    expect(screen.getByRole('spinbutton')).toBeDisabled();
    expect(screen.getByRole('slider')).toBeDisabled();
    // The row-level dim is the owning `SettingsRow`'s `is-disabled` (UI §5.4),
    // not a wrapper of this control's own — it has none.
    expect(onChange).not.toHaveBeenCalled();
  });
});
