import { useEffect, useRef, useState } from 'react';
import type { AiDigestRange } from '../ipc';

export interface WhatChangedDialogProps {
  open: boolean;
  /** Ref suggestions for the datalist (already-loaded local + remote branch
   *  names — display aid only; any revparse-able ref is accepted). */
  branchNames: string[];
  /** Current branch shorthand (seeds the "to" field); null when detached/unborn. */
  currentBranch: string | null;
  /** Fire-and-forget: parent kicks off the digest (results + errors render in
   *  the AiOutputPanel) and closes the dialog. */
  onSubmit(range: AiDigestRange, title: string): void;
  onCancel(): void;
}

type Mode = 'betweenRefs' | 'lastDays' | 'sinceCommit';

/**
 * P28 §7: "✨ What changed…" range picker. Three modes — between refs / last
 * N days on the current branch / since a commit. Presentational + local form
 * state only; no git logic (bad refs surface as errors in the AiOutputPanel).
 * Modeled on WorktreeCreateDialog (`.dialog-overlay` / `.dialog-card`); Esc +
 * overlay-click cancel.
 */
export function WhatChangedDialog({
  open,
  branchNames,
  currentBranch,
  onSubmit,
  onCancel,
}: WhatChangedDialogProps) {
  const firstFieldRef = useRef<HTMLInputElement>(null);
  const [mode, setMode] = useState<Mode>('betweenRefs');
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [days, setDays] = useState('7');
  const [oid, setOid] = useState('');
  const [showErrors, setShowErrors] = useState(false);

  // Reset the form each time the dialog opens (defaults per P28 §7).
  useEffect(() => {
    if (!open) return;
    setMode('betweenRefs');
    setFrom('');
    setTo(currentBranch ?? 'HEAD');
    setDays('7');
    setOid('');
    setShowErrors(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Focus after the reset effect has settled the mode, so the ref points at a mounted input.
  useEffect(() => {
    if (!open) return;
    firstFieldRef.current?.focus();
  }, [open, mode]);

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

  if (!open) return null;

  const daysNum = Number.parseInt(days, 10);
  const validationError: string | null = (() => {
    switch (mode) {
      case 'betweenRefs':
        if (from.trim() === '' || to.trim() === '') return 'Enter both refs';
        if (from.trim() === to.trim()) return 'Refs must differ';
        return null;
      case 'lastDays':
        if (!Number.isInteger(daysNum) || daysNum < 1) return 'Enter a number of days (min 1)';
        return null;
      case 'sinceCommit':
        if (oid.trim() === '') return 'Enter a commit or ref';
        return null;
    }
  })();

  const submit = () => {
    if (validationError !== null) {
      setShowErrors(true);
      return;
    }
    // Titles per P28 §7.
    switch (mode) {
      case 'betweenRefs': {
        const f = from.trim();
        const t = to.trim();
        onSubmit({ kind: 'betweenRefs', from: f, to: t }, `What changed: ${f}..${t}`);
        break;
      }
      case 'lastDays':
        onSubmit(
          { kind: 'lastDays', days: daysNum },
          `What changed: last ${daysNum} ${daysNum === 1 ? 'day' : 'days'}`,
        );
        break;
      case 'sinceCommit': {
        const o = oid.trim();
        onSubmit({ kind: 'sinceCommit', oid: o }, `What changed since ${o.slice(0, 7)}`);
        break;
      }
    }
  };

  const modeRadio = (value: Mode, label: string) => (
    <label className="dialog-radio">
      <input
        type="radio"
        name="what-changed-mode"
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
        className="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-label="What changed"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">✨ What changed</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            submit();
          }}
        >
          <div className="dialog-body">
            <div className="dialog-radio-group" role="radiogroup" aria-label="Range">
              {modeRadio('betweenRefs', 'Between refs')}
              {modeRadio('lastDays', 'Last N days')}
              {modeRadio('sinceCommit', 'Since commit')}
            </div>

            {mode === 'betweenRefs' && (
              <>
                <label className="dialog-label">
                  From (base ref)
                  <input
                    ref={firstFieldRef}
                    className="dialog-input"
                    value={from}
                    onChange={(e) => setFrom(e.target.value)}
                    placeholder="origin/main"
                    list="what-changed-refs"
                    spellCheck={false}
                  />
                </label>
                <label className="dialog-label">
                  To
                  <input
                    className="dialog-input"
                    value={to}
                    onChange={(e) => setTo(e.target.value)}
                    placeholder={currentBranch ?? 'HEAD'}
                    list="what-changed-refs"
                    spellCheck={false}
                  />
                </label>
                <datalist id="what-changed-refs">
                  {branchNames.map((n) => (
                    <option key={n} value={n} />
                  ))}
                </datalist>
                <p className="dialog-body-note">
                  Digests commits in "To" that are not in "From" (merge-base range).
                </p>
              </>
            )}

            {mode === 'lastDays' && (
              <>
                <label className="dialog-label">
                  Days
                  <input
                    className="dialog-input"
                    type="number"
                    min={1}
                    step={1}
                    value={days}
                    onChange={(e) => setDays(e.target.value)}
                  />
                </label>
                <p className="dialog-body-note">
                  Digests the last N days on the current branch
                  {currentBranch !== null ? ` (${currentBranch})` : ''}.
                </p>
              </>
            )}

            {mode === 'sinceCommit' && (
              <>
                <label className="dialog-label">
                  Commit or ref
                  <input
                    className="dialog-input"
                    value={oid}
                    onChange={(e) => setOid(e.target.value)}
                    placeholder="abc1234"
                    list="what-changed-refs"
                    spellCheck={false}
                  />
                </label>
                <datalist id="what-changed-refs">
                  {branchNames.map((n) => (
                    <option key={n} value={n} />
                  ))}
                </datalist>
                <p className="dialog-body-note">
                  Digests everything on the current branch since this commit.
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
              Digest
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
