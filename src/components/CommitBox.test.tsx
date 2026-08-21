/** T3.3a — CommitBox: validation gating, Ctrl+Enter wiring, busy/submitting
 *  locks, amend/merge modes, sign & skip-hooks toggles, and the AI generate
 *  flow (replace-confirm + rejection surfaced, never a commit). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { CommitBox } from './CommitBox';
import type { SigningStatus } from '../ipc';

type Props = Parameters<typeof CommitBox>[0];

function renderBox(over: Partial<Props> = {}) {
  const onCommit = vi.fn<Props['onCommit']>().mockResolvedValue(undefined);
  const utils = render(<CommitBox stagedCount={1} busy={false} onCommit={onCommit} {...over} />);
  return { ...utils, onCommit: (over.onCommit ?? onCommit) as ReturnType<typeof vi.fn> };
}

const textarea = () => screen.getByPlaceholderText('Commit message');
const commitBtn = () => screen.getByRole('button', { name: 'Commit' });
const generateBtn = () => screen.getByRole('button', { name: 'Generate commit message' });
/** P80 §2b: sign/skip/amend/compose now live in the `⋯` menu — open it first. */
const openMenu = () => fireEvent.click(screen.getByRole('button', { name: 'Commit options' }));

describe('CommitBox', () => {
  it('empty message disables Commit; typing enables it', () => {
    renderBox();
    expect(commitBtn()).toBeDisabled();
    fireEvent.change(textarea(), { target: { value: 'feat: x' } });
    expect(commitBtn()).toBeEnabled();
  });

  it('nothing staged disables Commit even with a message (non-amend)', () => {
    renderBox({ stagedCount: 0 });
    fireEvent.change(textarea(), { target: { value: 'feat: x' } });
    expect(commitBtn()).toBeDisabled();
  });

  it('amend allows a message-only commit with 0 staged and labels the button Amend', () => {
    const { onCommit } = renderBox({ stagedCount: 0, amend: true });
    const btn = screen.getByRole('button', { name: 'Amend' });
    fireEvent.change(textarea(), { target: { value: 'reworded' } });
    expect(btn).toBeEnabled();
    fireEvent.click(btn);
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('reworded', null, false);
  });

  it('busy disables Commit but the textarea stays typable', () => {
    renderBox({ busy: true });
    fireEvent.change(textarea(), { target: { value: 'feat: x' } });
    expect(textarea()).toBeEnabled();
    expect(commitBtn()).toBeDisabled();
  });

  it('Ctrl+Enter submits: sign=null (no signingStatus), skipHooks=false, clears on success', async () => {
    const { onCommit } = renderBox();
    fireEvent.change(textarea(), { target: { value: 'feat: x' } });
    fireEvent.keyDown(textarea(), { key: 'Enter', ctrlKey: true });
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('feat: x', null, false);
    await waitFor(() => expect(textarea()).toHaveValue(''));
  });

  it('Cmd+Enter (metaKey) submits too — macOS parity with Ctrl+Enter', async () => {
    const { onCommit } = renderBox();
    fireEvent.change(textarea(), { target: { value: 'feat: mac' } });
    fireEvent.keyDown(textarea(), { key: 'Enter', metaKey: true });
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('feat: mac', null, false);
    await waitFor(() => expect(textarea()).toHaveValue(''));
  });

  it('plain Enter does NOT submit', () => {
    const { onCommit } = renderBox();
    fireEvent.change(textarea(), { target: { value: 'feat: x' } });
    fireEvent.keyDown(textarea(), { key: 'Enter' });
    expect(onCommit).not.toHaveBeenCalled();
  });

  it('rejection keeps the message and shows the error banner; configMissing gets its action', async () => {
    const onCommit = vi
      .fn<Props['onCommit']>()
      .mockRejectedValue({ kind: 'configMissing', message: 'user.name is unset' });
    const onOpenIdentitySettings = vi.fn();
    renderBox({ onCommit, onOpenIdentitySettings });
    fireEvent.change(textarea(), { target: { value: 'feat: x' } });
    fireEvent.click(commitBtn());
    await screen.findByRole('alert');
    expect(textarea()).toHaveValue('feat: x');
    expect(screen.getByText(/Set your Git identity: user\.name is unset/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Set identity…' }));
    expect(onOpenIdentitySettings).toHaveBeenCalledTimes(1);
  });

  it('summary counter appears once typed and flags >72-char first lines', () => {
    renderBox();
    expect(screen.queryByText(/\/72/)).not.toBeInTheDocument();
    fireEvent.change(textarea(), { target: { value: 'x'.repeat(80) } });
    const counter = screen.getByText('80/72');
    expect(counter).toHaveClass('commit-counter-over');
  });

  it('signing toggle (⋯ menu) defaults from config and sends the explicit value', () => {
    const signingStatus: SigningStatus = { enabled: true, hasKey: true, format: 'ssh' } as SigningStatus;
    const { onCommit } = renderBox({ signingStatus });
    // The will-sign note shows below the toolbar regardless of menu open state.
    expect(screen.getByText('Commits will be signed (SSH)')).toBeInTheDocument();
    openMenu();
    const toggle = screen.getByRole('menuitemcheckbox', { name: /Sign commit/ });
    expect(toggle).toHaveAttribute('aria-checked', 'true');
    fireEvent.click(toggle); // explicit off
    fireEvent.change(textarea(), { target: { value: 'feat: x' } });
    fireEvent.click(commitBtn());
    expect(onCommit).toHaveBeenCalledWith('feat: x', false, false);
  });

  it('skip-hooks menuitemcheckbox (⋯ menu) is sent as skipHooks=true with a hint', () => {
    const { onCommit } = renderBox();
    openMenu();
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: /Skip hooks/ }));
    expect(screen.getByText(/won’t run for this commit/)).toBeInTheDocument();
    fireEvent.change(textarea(), { target: { value: 'feat: x' } });
    fireEvent.click(commitBtn());
    expect(onCommit).toHaveBeenCalledWith('feat: x', null, true);
  });

  // P80 §2b: sign + skip fold into the `⋯` overflow menu as menuitemcheckboxes.
  it('sign and skip-hooks are menuitemcheckboxes in the ⋯ menu', () => {
    const signingStatus: SigningStatus = { enabled: true, hasKey: true, format: 'ssh' };
    renderBox({ signingStatus });
    openMenu();
    expect(screen.getByRole('menuitemcheckbox', { name: /Sign commit/ })).toBeInTheDocument();
    expect(screen.getByRole('menuitemcheckbox', { name: /Skip hooks/ })).toBeInTheDocument();
  });

  it('merge mode: conflicts gate "Commit merge"; resolving enables it', () => {
    const onCommit = vi.fn<Props['onCommit']>().mockResolvedValue(undefined);
    const { rerender } = render(
      <CommitBox
        stagedCount={0}
        busy={false}
        onCommit={onCommit}
        mode="merge"
        initialMessage="Merge branch 'dev'"
        conflictCount={2}
      />,
    );
    const btn = screen.getByRole('button', { name: 'Commit merge' });
    expect(btn).toBeDisabled();
    rerender(
      <CommitBox
        stagedCount={0}
        busy={false}
        onCommit={onCommit}
        mode="merge"
        initialMessage="Merge branch 'dev'"
        conflictCount={0}
      />,
    );
    expect(btn).toBeEnabled();
    fireEvent.click(btn);
    expect(onCommit).toHaveBeenCalledWith("Merge branch 'dev'", null, false);
  });

  it('split control renders Commit & Push when onCommitAndPush is provided (not amend/merge)', () => {
    const onCommitAndPush = vi.fn<NonNullable<Props['onCommitAndPush']>>().mockResolvedValue(undefined);
    const { onCommit } = renderBox({ onCommitAndPush });
    fireEvent.change(textarea(), { target: { value: 'feat: x' } });
    fireEvent.click(screen.getByRole('button', { name: 'Commit & Push' }));
    expect(onCommitAndPush).toHaveBeenCalledTimes(1);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it('generate: disabled when AI-ineligible; fills an empty box without committing', async () => {
    const onGenerate = vi.fn().mockResolvedValue('feat: generated');
    const { rerender } = render(
      <CommitBox stagedCount={1} busy={false} onCommit={vi.fn()} onGenerate={onGenerate} aiEligible={false} />,
    );
    expect(generateBtn()).toBeDisabled();
    const onCommit = vi.fn();
    rerender(
      <CommitBox stagedCount={1} busy={false} onCommit={onCommit} onGenerate={onGenerate} aiEligible />,
    );
    fireEvent.click(generateBtn());
    await waitFor(() => expect(textarea()).toHaveValue('feat: generated'));
    expect(onGenerate).toHaveBeenCalledTimes(1);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it('generate over a non-empty box requires the Replace confirm; cancel keeps the text', async () => {
    const onGenerate = vi.fn().mockResolvedValue('feat: generated');
    renderBox({ onGenerate, aiEligible: true });
    fireEvent.change(textarea(), { target: { value: 'my draft' } });
    fireEvent.click(generateBtn());
    const dialog = await screen.findByRole('dialog', { name: 'Replace the current message?' });
    expect(dialog).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onGenerate).not.toHaveBeenCalled();
    expect(textarea()).toHaveValue('my draft');
    // Confirming does replace.
    fireEvent.click(generateBtn());
    fireEvent.click(await screen.findByRole('button', { name: 'Replace' }));
    await waitFor(() => expect(textarea()).toHaveValue('feat: generated'));
  });

  it('generate rejection surfaces the error without crashing or committing', async () => {
    const onGenerate = vi.fn().mockRejectedValue({ kind: 'other', message: 'CLI missing' });
    const { onCommit } = renderBox({ onGenerate, aiEligible: true });
    fireEvent.click(generateBtn());
    await screen.findByRole('alert');
    expect(screen.getByText('CLI missing')).toBeInTheDocument();
    expect(onCommit).not.toHaveBeenCalled();
  });

  it('blocked (non-merge op in progress) disables everything', () => {
    renderBox({ blocked: true });
    expect(screen.getByPlaceholderText('An operation is in progress')).toBeDisabled();
    expect(commitBtn()).toBeDisabled();
  });
});
