/**
 * P113 §17.3 — the save-failure banner: the one outcome in this sweep that is
 * not a row note.
 *
 * `useUiSettings` fires on EVERY failed settings write — every toggle, radio and
 * field rides that patch path — and until now it has always been a toast, which
 * on the Settings surface renders behind `.dialog-overlay` and cannot be
 * dismissed. The mock could not make the write fail either, so nobody had ever
 * seen this failure rendered at all.
 *
 * What is pinned here: the idle shape (present, empty, no chrome), the A4 copy
 * byte for byte, the Retry wiring, and the fact that the banner spans the card
 * rather than living inside any category page.
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { SETTINGS_SAVE_FAILURE_TEXT } from '../../hooks/useSettingsSaveFailure';
import { SettingsSaveBanner } from './SettingsSaveBanner';

const banner = () => document.querySelector('[data-settings-save-banner]');

describe('SettingsSaveBanner', () => {
  it('is PRESENT and truly empty while the write is fine', () => {
    render(<SettingsSaveBanner failed={false} onRetry={vi.fn()} />);
    const el = banner();
    expect(el).not.toBeNull();
    // `:empty` is the whole collapse mechanism, so a single stray child node —
    // including whitespace — would silently give the card a 12px seam it does
    // not have today. `display: none` is forbidden: it costs the announcement.
    expect(el?.childNodes.length).toBe(0);
  });

  /** A3 — the sequence that had NO announcement before: the write fails while
   *  Settings is open, the user closes it and reopens, and the banner MOUNTS
   *  already carrying its text. The old `{failed && …}` made that one commit,
   *  the shape §12.3.4 says is not reliably announced.
   *
   *  Measured on the mutation RECORDS, because `render` is `act`-wrapped and the
   *  DOM has already settled by the time an assertion can read it: two commits
   *  ⟺ a record whose TARGET is the banner (the text arrived into a div that was
   *  already in the document). One commit produces no such record — the div is
   *  inserted fully populated. */
  it('mounting into a STANDING failure still lands the text in a later commit', () => {
    const container = document.body.appendChild(document.createElement('div'));
    const observer = new MutationObserver(() => {});
    observer.observe(container, { childList: true, subtree: true });

    render(<SettingsSaveBanner failed onRetry={vi.fn()} />, { container });

    const records = observer.takeRecords();
    observer.disconnect();
    const el = banner();
    expect(el).not.toBeNull();
    const mounted = records.findIndex((r) => [...r.addedNodes].includes(el as Node));
    const filled = records.findIndex((r) => r.target === el && r.addedNodes.length > 0);
    expect(mounted, 'the banner div must be inserted').toBeGreaterThanOrEqual(0);
    expect(filled, 'the text must arrive in a LATER commit than the mount').toBeGreaterThan(
      mounted,
    );
    expect(el).toHaveTextContent(SETTINGS_SAVE_FAILURE_TEXT);
  });

  it('carries role="alert" from the start, so the text arrives in a LATER tick', () => {
    const { rerender } = render(<SettingsSaveBanner failed={false} onRetry={vi.fn()} />);
    const before = banner();
    expect(before).toHaveAttribute('role', 'alert');

    rerender(<SettingsSaveBanner failed onRetry={vi.fn()} />);
    // Same element, new content — which is what a live region announces on. A
    // region mounted together with its text is not reliably announced.
    expect(banner()).toBe(before);
  });

  it('renders the approved A4 copy, with no raw OS error', () => {
    render(<SettingsSaveBanner failed onRetry={vi.fn()} />);
    expect(banner()).toHaveTextContent(SETTINGS_SAVE_FAILURE_TEXT);
    expect(SETTINGS_SAVE_FAILURE_TEXT).toBe(
      "Your settings couldn't be saved. They still apply until Bonsai closes, and it will try again on your next change. If this keeps happening, check that Bonsai can write to its config folder.",
    );
  });

  it('offers Retry, and only while failing', () => {
    const onRetry = vi.fn();
    const { rerender } = render(<SettingsSaveBanner failed={false} onRetry={onRetry} />);
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull();

    rerender(<SettingsSaveBanner failed onRetry={onRetry} />);
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('composes the shared .error-banner recipe under its own scoping class', () => {
    render(<SettingsSaveBanner failed onRetry={vi.fn()} />);
    const el = banner();
    // Both classes: `--danger` tone comes from the shared recipe (work IS at
    // risk — the change is in memory only), and the `:empty` collapse is scoped
    // to the new class so the other thirteen `.error-banner` sites are untouched.
    expect(el?.classList.contains('error-banner')).toBe(true);
    expect(el?.classList.contains('settings-save-banner')).toBe(true);
  });
});
