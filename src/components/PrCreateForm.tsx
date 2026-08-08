import { useState } from 'react';
import type { CreatePrInput } from '../ipc';

// P62c: presentational "open a pull request" form. Local field state only; the
// container (PrPanel) owns submission + the forgeCreatePr call.
// OQ-4 seam: the "Generate with AI" button renders ONLY when
// `onGenerateDescription` is provided — P64 wires the command; P62 leaves it off.

export interface PrCreateFormProps {
  defaultHead?: string | null;
  defaultBase?: string | null;
  submitting: boolean;
  error: string | null;
  onSubmit(input: CreatePrInput): void;
  onCancel(): void;
  /** P64 seam (OQ-4). When provided, renders a "Generate with AI" button. */
  onGenerateDescription?: () => void;
}

export function PrCreateForm({
  defaultHead,
  defaultBase,
  submitting,
  error,
  onSubmit,
  onCancel,
  onGenerateDescription,
}: PrCreateFormProps) {
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [head, setHead] = useState(defaultHead ?? '');
  const [base, setBase] = useState(defaultBase ?? '');
  const [draft, setDraft] = useState(false);

  const canSubmit =
    !submitting && title.trim() !== '' && head.trim() !== '' && base.trim() !== '';

  function submit() {
    if (!canSubmit) return;
    onSubmit({
      title: title.trim(),
      body,
      sourceBranch: head.trim(),
      targetBranch: base.trim(),
      draft,
      maintainerCanModify: true,
    });
  }

  return (
    <form
      className="pr-create"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <div className="pr-create-header">
        <button type="button" className="section-action pr-back-button" onClick={onCancel}>
          {'← Pull requests'}
        </button>
        <h3 className="pr-create-heading">Open a pull request</h3>
      </div>

      <div className="pr-create-branches">
        <label className="pr-field">
          <span className="pr-field-label">Base</span>
          <input
            className="pr-input mono"
            type="text"
            placeholder="target branch (e.g. main)"
            value={base}
            disabled={submitting}
            onChange={(e) => setBase(e.target.value)}
          />
        </label>
        <span className="pr-create-arrow" aria-hidden="true">
          ←
        </span>
        <label className="pr-field">
          <span className="pr-field-label">Compare</span>
          <input
            className="pr-input mono"
            type="text"
            placeholder="source branch"
            value={head}
            disabled={submitting}
            onChange={(e) => setHead(e.target.value)}
          />
        </label>
      </div>

      <label className="pr-field">
        <span className="pr-field-label">Title</span>
        <input
          className="pr-input"
          type="text"
          placeholder="Add a title"
          value={title}
          disabled={submitting}
          onChange={(e) => setTitle(e.target.value)}
        />
      </label>

      <label className="pr-field">
        <span className="pr-field-label">
          Description
          {onGenerateDescription !== undefined && (
            <button
              type="button"
              className="section-action pr-generate-button"
              disabled={submitting}
              onClick={onGenerateDescription}
            >
              {'✨ Generate with AI'}
            </button>
          )}
        </span>
        <textarea
          className="pr-input pr-textarea"
          placeholder="Describe the change (optional)"
          value={body}
          disabled={submitting}
          rows={8}
          onChange={(e) => setBody(e.target.value)}
        />
      </label>

      <label className="pr-draft-toggle">
        <input
          type="checkbox"
          checked={draft}
          disabled={submitting}
          onChange={(e) => setDraft(e.target.checked)}
        />
        <span>Create as draft</span>
      </label>

      {error !== null && (
        <div className="error-banner error-banner-dismissible pr-error" role="alert">
          <span className="error-banner-text">{error}</span>
        </div>
      )}

      <div className="pr-create-actions">
        <button type="button" className="btn-secondary" disabled={submitting} onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" className="btn-primary" disabled={!canSubmit}>
          {submitting ? 'Opening…' : 'Create pull request'}
        </button>
      </div>
    </form>
  );
}
