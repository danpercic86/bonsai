/** T3.3a — PrCreateForm: submit gating, and the P64a AI-generate seam
 *  (eligibility gate, fill-never-submit, generating lock, rejection → toast). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { PrCreateForm } from './PrCreateForm';
import { ToastContext } from '../ToastContext';
import type { PrDescription } from '../ipc';

type Props = Parameters<typeof PrCreateForm>[0];

const proposal: PrDescription = {
  title: 'feat: add widgets',
  body: 'Adds the widget panel.',
  base: 'main',
  head: 'feature/widgets',
  commitCount: 3,
  costUsd: null,
};

function renderForm(over: Partial<Props> = {}, pushToast = vi.fn()) {
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  const utils = render(
    <ToastContext.Provider value={pushToast}>
      <PrCreateForm
        defaultHead="feature/widgets"
        defaultBase="main"
        submitting={false}
        error={null}
        onSubmit={onSubmit}
        onCancel={onCancel}
        {...over}
      />
    </ToastContext.Provider>,
  );
  return { ...utils, onSubmit, onCancel, pushToast };
}

const titleInput = () => screen.getByPlaceholderText('Add a title');
const submitBtn = () => screen.getByRole('button', { name: 'Create pull request' });
// NOTE: the generate button lives inside the Description <label>, so its
// computed accessible name is "Description" (label descendants are named by the
// label) — select by class. Cosmetic a11y nit, not a functional bug.
const generateBtn = () =>
  document.querySelector<HTMLButtonElement>('.pr-generate-button')!;

describe('PrCreateForm', () => {
  it('submit is disabled until title+head+base are all present', () => {
    renderForm();
    expect(submitBtn()).toBeDisabled();
    fireEvent.change(titleInput(), { target: { value: 'My PR' } });
    expect(submitBtn()).toBeEnabled();
  });

  it('explicit submit passes trimmed branches + draft flag', () => {
    const { onSubmit } = renderForm();
    fireEvent.change(titleInput(), { target: { value: '  My PR  ' } });
    fireEvent.click(screen.getByRole('checkbox', { name: /Create as draft/ }));
    fireEvent.click(submitBtn());
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith({
      title: 'My PR',
      body: '',
      sourceBranch: 'feature/widgets',
      targetBranch: 'main',
      draft: true,
      maintainerCanModify: true,
    });
  });

  it('no generate button at all when the seam is not provided', () => {
    renderForm({ onGenerateDescription: undefined });
    expect(document.querySelector('.pr-generate-button')).not.toBeInTheDocument();
  });

  it('AI-ineligible: button rendered but disabled with the settings tooltip', () => {
    renderForm({ onGenerateDescription: vi.fn(), aiEligible: false });
    expect(generateBtn()).toBeDisabled();
    expect(generateBtn()).toHaveAttribute(
      'title',
      'Enable AI features in settings to generate a description',
    );
  });

  it('incomplete range disables generate with the range tooltip', () => {
    renderForm({ onGenerateDescription: vi.fn(), aiEligible: true, defaultBase: '' });
    expect(generateBtn()).toBeDisabled();
    expect(generateBtn()).toHaveAttribute(
      'title',
      'Enter a base and compare branch to generate a description',
    );
  });

  it('generate fills title+body from the proposal and NEVER auto-submits', async () => {
    const onGenerateDescription = vi.fn().mockResolvedValue(proposal);
    const { onSubmit } = renderForm({ onGenerateDescription, aiEligible: true });
    fireEvent.click(generateBtn());
    await waitFor(() => expect(titleInput()).toHaveValue('feat: add widgets'));
    expect(screen.getByPlaceholderText(/Describe the change/)).toHaveValue(
      'Adds the widget panel.',
    );
    expect(onGenerateDescription).toHaveBeenCalledWith('main', 'feature/widgets');
    expect(onSubmit).not.toHaveBeenCalled();
    // Submit still requires the explicit user action — and then works.
    fireEvent.click(submitBtn());
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it('generating lock: a second click while pending does not double-call', async () => {
    let resolve!: (v: PrDescription) => void;
    const onGenerateDescription = vi.fn(
      () => new Promise<PrDescription>((r) => (resolve = r)),
    );
    renderForm({ onGenerateDescription, aiEligible: true });
    fireEvent.click(generateBtn());
    const pending = generateBtn();
    expect(pending).toHaveTextContent('Generating…');
    expect(pending).toBeDisabled();
    fireEvent.click(pending);
    expect(onGenerateDescription).toHaveBeenCalledTimes(1);
    resolve(proposal);
    await waitFor(() => expect(titleInput()).toHaveValue('feat: add widgets'));
    expect(generateBtn()).toHaveTextContent('Generate with AI');
    expect(generateBtn()).toBeEnabled();
  });

  it('rejection raises an error toast, does not crash, and leaves fields untouched', async () => {
    const onGenerateDescription = vi
      .fn()
      .mockRejectedValue({ kind: 'other', message: 'CLI not installed' });
    const { pushToast, onSubmit } = renderForm({ onGenerateDescription, aiEligible: true });
    fireEvent.change(titleInput(), { target: { value: 'draft title' } });
    fireEvent.click(generateBtn());
    await waitFor(() =>
      expect(pushToast).toHaveBeenCalledWith(
        'error',
        'Could not generate a description: CLI not installed',
      ),
    );
    expect(titleInput()).toHaveValue('draft title');
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('submitting locks every field (incl. Cancel) and shows Opening…', () => {
    renderForm({ submitting: true });
    expect(screen.getByRole('button', { name: 'Opening…' })).toBeDisabled();
    expect(titleInput()).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled();
  });

  it('Cancel routes onCancel without submitting', () => {
    const { onCancel, onSubmit } = renderForm();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('error prop renders the alert banner', () => {
    renderForm({ error: 'boom from forge' });
    expect(screen.getByRole('alert')).toHaveTextContent('boom from forge');
  });
});

// P78 — Base/Compare fields are Comboboxes (allowFreeInput) fed by base/compareOptions.
describe('PrCreateForm — P78 Base/Compare branch dropdowns', () => {
  const baseOptions = [
    { value: 'main', label: 'main' },
    { value: 'origin/main', label: 'origin/main' },
    { value: 'develop', label: 'develop' },
  ];
  const compareOptions = [
    { value: 'feature/widgets', label: 'feature/widgets' },
    { value: 'feature/logs', label: 'feature/logs' },
  ];

  // Placeholders are load-bearing (e2e selectors); keep querying by them.
  const baseInput = () =>
    screen.getByPlaceholderText('target branch (e.g. main)') as HTMLInputElement;
  const compareInput = () =>
    screen.getByPlaceholderText('source branch') as HTMLInputElement;

  it('defaultBase seeds the Base field', () => {
    renderForm({ defaultBase: 'develop', baseOptions, compareOptions });
    expect(baseInput()).toHaveValue('develop');
  });

  it('focusing Base renders its options as selectable dropdown suggestions', () => {
    // Start empty so the popover shows the full unfiltered list.
    renderForm({ defaultBase: '', defaultHead: '', baseOptions, compareOptions });
    // No popover before focus.
    expect(screen.queryByRole('option')).toBeNull();
    fireEvent.focus(baseInput());
    const opts = screen.getAllByRole('option').map((o) => o.textContent);
    expect(opts).toEqual(['main', 'origin/main', 'develop']);
  });

  it('selecting a Base option sets the value; selecting Compare enables + drives submit', () => {
    const { onSubmit } = renderForm({
      defaultBase: '',
      defaultHead: '',
      baseOptions,
      compareOptions,
    });
    fireEvent.change(titleInput(), { target: { value: 'My PR' } });
    // Base + Compare empty → submit still disabled.
    expect(submitBtn()).toBeDisabled();

    // Pick Base from the dropdown (select fires on mouseDown).
    fireEvent.focus(baseInput());
    fireEvent.mouseDown(screen.getByRole('option', { name: 'origin/main' }));
    expect(baseInput()).toHaveValue('origin/main');
    // Compare still empty → still disabled.
    expect(submitBtn()).toBeDisabled();

    // Pick Compare from the dropdown.
    fireEvent.focus(compareInput());
    fireEvent.mouseDown(screen.getByRole('option', { name: 'feature/logs' }));
    expect(compareInput()).toHaveValue('feature/logs');
    expect(submitBtn()).toBeEnabled();

    fireEvent.click(submitBtn());
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({
        sourceBranch: 'feature/logs',
        targetBranch: 'origin/main',
      }),
    );
  });

  it('a free-typed branch not in the options list still works and submits (allowFreeInput)', () => {
    const { onSubmit } = renderForm({
      defaultBase: '',
      defaultHead: '',
      baseOptions,
      compareOptions,
    });
    fireEvent.change(titleInput(), { target: { value: 'My PR' } });
    // Type branch names absent from the option lists.
    fireEvent.change(baseInput(), { target: { value: 'release/1.2' } });
    fireEvent.change(compareInput(), { target: { value: 'wip/experiment' } });
    expect(baseInput()).toHaveValue('release/1.2');
    expect(compareInput()).toHaveValue('wip/experiment');
    expect(submitBtn()).toBeEnabled();

    fireEvent.click(submitBtn());
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({
        sourceBranch: 'wip/experiment',
        targetBranch: 'release/1.2',
      }),
    );
  });

  it('empty options list: fields accept typed text and popover shows "No matches"', () => {
    renderForm({ defaultBase: '', defaultHead: '', baseOptions: [], compareOptions: [] });
    fireEvent.focus(baseInput());
    // A filter with no matching options renders the empty-affordance row.
    fireEvent.change(baseInput(), { target: { value: 'anything' } });
    expect(screen.getByRole('option')).toHaveTextContent('No matches');
    expect(baseInput()).toHaveValue('anything');
  });
});
