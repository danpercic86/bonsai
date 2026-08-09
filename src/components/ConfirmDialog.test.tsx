/** T3.3a — shared modal primitives: ConfirmDialog (destructive default) and
 *  PromptDialog. The load-bearing safety property: a stray Enter must NEVER
 *  confirm a destructive ConfirmDialog (initial focus lands on Cancel and Enter
 *  only activates the focused button). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ConfirmDialog } from './ConfirmDialog';
import { PromptDialog } from './PromptDialog';

function renderConfirm(over: Partial<Parameters<typeof ConfirmDialog>[0]> = {}) {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  const utils = render(
    <ConfirmDialog
      open
      title="Delete branch"
      confirmLabel="Delete branch"
      busy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
      {...over}
    >
      <div>Delete branch "x"?</div>
    </ConfirmDialog>,
  );
  return { ...utils, onConfirm, onCancel };
}

describe('ConfirmDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = renderConfirm({ open: false });
    expect(container).toBeEmptyDOMElement();
  });

  it('renders title, body and both buttons; confirm defaults to danger styling', () => {
    renderConfirm();
    expect(screen.getByRole('dialog', { name: 'Delete branch' })).toBeInTheDocument();
    expect(screen.getByText('Delete branch "x"?')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete branch' })).toHaveClass('btn-danger');
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
  });

  it("confirmVariant='primary' renders a non-danger confirm button", () => {
    renderConfirm({ confirmVariant: 'primary', confirmLabel: 'Commit & Push' });
    const btn = screen.getByRole('button', { name: 'Commit & Push' });
    expect(btn).toHaveClass('btn-primary');
    expect(btn).not.toHaveClass('btn-danger');
  });

  it('initial focus lands on Cancel, so a stray Enter cancels instead of confirming', async () => {
    const { onConfirm, onCancel } = renderConfirm();
    expect(screen.getByRole('button', { name: 'Cancel' })).toHaveFocus();
    await userEvent.keyboard('{Enter}');
    expect(onConfirm).not.toHaveBeenCalled();
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('explicit click on the confirm button calls onConfirm exactly once', () => {
    const { onConfirm, onCancel } = renderConfirm();
    fireEvent.click(screen.getByRole('button', { name: 'Delete branch' }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('Escape cancels without confirming', () => {
    const { onConfirm, onCancel } = renderConfirm();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('overlay click cancels; clicks inside the card do not', () => {
    const { onCancel, container } = renderConfirm();
    fireEvent.click(screen.getByText('Delete branch "x"?'));
    expect(onCancel).not.toHaveBeenCalled();
    fireEvent.click(container.querySelector('.dialog-overlay')!);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('busy disables the confirm button (Cancel stays enabled)', () => {
    renderConfirm({ busy: true });
    expect(screen.getByRole('button', { name: 'Delete branch' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeEnabled();
  });
});

function renderPrompt(over: Partial<Parameters<typeof PromptDialog>[0]> = {}) {
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  const utils = render(
    <PromptDialog
      open
      title="Create branch here"
      label="Branch name"
      confirmLabel="Create branch"
      busy={false}
      onSubmit={onSubmit}
      onCancel={onCancel}
      {...over}
    />,
  );
  return { ...utils, onSubmit, onCancel };
}

describe('PromptDialog', () => {
  it('focuses the input on open and Enter submits the typed value (non-destructive)', () => {
    const { onSubmit } = renderPrompt();
    const input = screen.getByLabelText('Branch name');
    expect(input).toHaveFocus();
    fireEvent.change(input, { target: { value: 'feature/x' } });
    fireEvent.submit(input.closest('form')!);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith('feature/x');
  });

  it('validate error blocks submit and renders under the input', () => {
    const { onSubmit } = renderPrompt({
      validate: (v) => (v.trim() === '' ? 'Enter a valid branch name' : null),
    });
    expect(screen.getByText('Enter a valid branch name')).toBeInTheDocument();
    const confirm = screen.getByRole('button', { name: 'Create branch' });
    expect(confirm).toBeDisabled();
    fireEvent.submit(screen.getByLabelText('Branch name').closest('form')!);
    expect(onSubmit).not.toHaveBeenCalled();
    // Typing a valid value clears the error and enables submit.
    fireEvent.change(screen.getByLabelText('Branch name'), { target: { value: 'ok' } });
    expect(screen.queryByText('Enter a valid branch name')).not.toBeInTheDocument();
    expect(confirm).toBeEnabled();
  });

  it('initialValue seeds (and re-seeds) the input on open', () => {
    const props = {
      title: 'Rename branch',
      label: 'New branch name',
      confirmLabel: 'Rename',
      busy: false,
      initialValue: 'main',
      onSubmit: vi.fn(),
      onCancel: vi.fn(),
    };
    const { rerender } = render(<PromptDialog open {...props} />);
    const input = screen.getByLabelText('New branch name');
    expect(input).toHaveValue('main');
    fireEvent.change(input, { target: { value: 'scratch' } });
    rerender(<PromptDialog open={false} {...props} />);
    rerender(<PromptDialog open {...props} />);
    expect(screen.getByLabelText('New branch name')).toHaveValue('main');
  });

  it('Escape and overlay-click cancel without submitting', () => {
    const { onSubmit, onCancel, container } = renderPrompt();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    fireEvent.click(container.querySelector('.dialog-overlay')!);
    expect(onCancel).toHaveBeenCalledTimes(2);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('busy disables submit', () => {
    const { onSubmit } = renderPrompt({ busy: true });
    fireEvent.change(screen.getByLabelText('Branch name'), { target: { value: 'x' } });
    expect(screen.getByRole('button', { name: 'Create branch' })).toBeDisabled();
    fireEvent.submit(screen.getByLabelText('Branch name').closest('form')!);
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
