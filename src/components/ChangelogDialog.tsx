import { useEffect, useMemo, useState } from 'react';
import type { ChangelogRange } from '../ipc';
import { Combobox, type ComboboxOption } from './Combobox';

export interface ChangelogDialogProps {
  open: boolean;
  /** Ref suggestions for the comboboxes (already-loaded tag + local/remote branch
   *  names — display aid only; any revparse-able ref is accepted). */
  refNames: string[];
  /** Current branch shorthand (seeds the "to" field); null when detached/unborn. */
  currentBranch: string | null;
  /** Fire-and-forget: parent kicks off the changelog (results + errors render in
   *  the AiOutputPanel) and closes the dialog. */
  onSubmit(range: ChangelogRange, title: string): void;
  onCancel(): void;
}

type Mode = 'betweenRefs' | 'sinceLastTag';

/**
 * P56b §6: "Release notes…" range picker. Two modes — between refs
 * (`v1.2.0`..`v1.3.0`, tags are the common case) / since the last tag reachable
 * from an optional target (default HEAD). Presentational + local form state only;
 * no git logic (bad refs / empty range surface as errors in the AiOutputPanel).
 * Modeled on WhatChangedDialog (`.dialog-overlay` / `.dialog-card`, shared radio
 * + Combobox idioms); Esc + overlay-click cancel.
 */
export function ChangelogDialog({
  open,
  refNames,
  currentBranch,
  onSubmit,
  onCancel,
}: ChangelogDialogProps) {
  const [mode, setMode] = useState<Mode>('betweenRefs');
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [target, setTarget] = useState('');
  const [showErrors, setShowErrors] = useState(false);

  // Reset the form each time the dialog opens (defaults per §6: `to`→current
  // branch/HEAD; `target` empty → the backend resolves the latest tag from HEAD).
  useEffect(() => {
    if (!open) return;
    setMode('betweenRefs');
    setFrom('');
    setTo(currentBranch ?? 'HEAD');
    setTarget('');
    setShowErrors(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Capture phase + stopPropagation: App's global Esc-deselect must not fire.
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onCancel]);

  const refOptions: ComboboxOption[] = useMemo(
    () => refNames.map((n) => ({ value: n, label: n })),
    [refNames],
  );

  if (!open) return null;

  const validationError: string | null = (() => {
    if (mode !== 'betweenRefs') return null;
    if (from.trim() === '' || to.trim() === '') return 'Enter both refs';
    if (from.trim() === to.trim()) return 'Refs must differ';
    return null;
  })();

  const submit = () => {
    if (validationError !== null) {
      setShowErrors(true);
      return;
    }
    if (mode === 'betweenRefs') {
      const f = from.trim();
      const t = to.trim();
      onSubmit({ kind: 'betweenRefs', from: f, to: t }, `Release notes: ${f}..${t}`);
      return;
    }
    const tgt = target.trim();
    if (tgt === '') {
      onSubmit({ kind: 'sinceLastTag' }, 'Release notes since last tag');
    } else {
      onSubmit({ kind: 'sinceLastTag', target: tgt }, `Release notes for ${tgt}`);
    }
  };

  const modeRadio = (value: Mode, label: string) => (
    <label className="dialog-radio">
      <input
        type="radio"
        name="changelog-mode"
        value={value}
        checked={mode === value}
        onChange={() => {
          setMode(value);
          setShowErrors(false);
        }}
      />
      {label}
    </label>
  );

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        className="dialog-card changelog-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Release notes"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">✨ Release notes</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            submit();
          }}
        >
          <div className="dialog-body">
            <div className="dialog-radio-group" role="radiogroup" aria-label="Range">
              {modeRadio('betweenRefs', 'Between refs')}
              {modeRadio('sinceLastTag', 'Since last tag')}
            </div>

            {mode === 'betweenRefs' && (
              <>
                <label className="dialog-label">
                  From (previous ref)
                  <Combobox
                    allowFreeInput
                    ariaLabel="From (previous ref)"
                    value={from}
                    onChange={setFrom}
                    options={refOptions}
                    placeholder="v1.2.0"
                    autoFocus
                  />
                </label>
                <label className="dialog-label">
                  To
                  <Combobox
                    allowFreeInput
                    ariaLabel="To"
                    value={to}
                    onChange={setTo}
                    options={refOptions}
                    placeholder={currentBranch ?? 'HEAD'}
                  />
                </label>
                <p className="dialog-body-note">
                  Notes for commits in "To" that are not in "From" (merge-base range).
                </p>
              </>
            )}

            {mode === 'sinceLastTag' && (
              <>
                <label className="dialog-label">
                  Target (optional)
                  <Combobox
                    allowFreeInput
                    ariaLabel="Target (optional)"
                    value={target}
                    onChange={setTarget}
                    options={refOptions}
                    placeholder="HEAD"
                    autoFocus
                  />
                </label>
                <p className="dialog-body-note">
                  Notes for everything since the most recent tag reachable from the
                  target (default HEAD) — i.e. what shipped in it.
                </p>
              </>
            )}

            {showErrors && validationError !== null && (
              <p className="dialog-error">{validationError}</p>
            )}
          </div>
          <div className="dialog-buttons">
            <button type="button" className="btn-secondary" onClick={onCancel}>
              Cancel
            </button>
            <button
              type="submit"
              className="btn-primary"
              disabled={showErrors && validationError !== null}
            >
              Generate
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
