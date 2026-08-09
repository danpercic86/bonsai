/** T3.3a — Combobox primitive: open-on-focus, filter-as-you-type, mouse and
 *  keyboard selection, disabled options, strict-mode revert, free-input mode,
 *  and the capture-phase Escape that closes only the popover. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Combobox, type ComboboxOption } from './Combobox';

const options: ComboboxOption[] = [
  { value: 'main', label: 'main' },
  { value: 'dev', label: 'dev' },
  { value: 'feature/x', label: 'feature/x', hint: 'ahead 2' },
  { value: 'wip', label: 'wip', disabled: true },
];

function renderStrict(value = 'main') {
  const onChange = vi.fn();
  const utils = render(
    <Combobox options={options} value={value} onChange={onChange} ariaLabel="Branch" />,
  );
  return { ...utils, onChange };
}

const input = () => screen.getByRole('combobox', { name: 'Branch' });

describe('Combobox (strict select)', () => {
  it('shows the selected label; focusing opens the popover with all options', () => {
    renderStrict();
    expect(input()).toHaveValue('main');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    fireEvent.focus(input());
    expect(screen.getByRole('listbox')).toBeInTheDocument();
    expect(screen.getAllByRole('option')).toHaveLength(4);
  });

  it('typing filters the options; "No matches" for garbage', () => {
    renderStrict();
    fireEvent.focus(input());
    fireEvent.change(input(), { target: { value: 'fea' } });
    expect(screen.getAllByRole('option')).toHaveLength(1);
    expect(screen.getByRole('option', { name: /feature\/x/ })).toBeInTheDocument();
    fireEvent.change(input(), { target: { value: 'zzz' } });
    expect(screen.getByText('No matches')).toBeInTheDocument();
  });

  it('mousedown on an option commits it and closes the popover', () => {
    const { onChange } = renderStrict();
    fireEvent.focus(input());
    fireEvent.mouseDown(screen.getByRole('option', { name: 'dev' }));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith('dev');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('Enter selects the keyboard-highlighted option, skipping disabled rows', () => {
    const { onChange } = renderStrict();
    fireEvent.focus(input());
    // Highlight starts on the first enabled option ('main'); two downs → feature/x.
    fireEvent.keyDown(input(), { key: 'ArrowDown' });
    fireEvent.keyDown(input(), { key: 'ArrowDown' });
    fireEvent.keyDown(input(), { key: 'Enter' });
    expect(onChange).toHaveBeenCalledWith('feature/x');
  });

  it('ArrowDown wraps past the disabled last option back to the top', () => {
    const { onChange } = renderStrict();
    fireEvent.focus(input());
    fireEvent.keyDown(input(), { key: 'ArrowDown' }); // dev
    fireEvent.keyDown(input(), { key: 'ArrowDown' }); // feature/x
    fireEvent.keyDown(input(), { key: 'ArrowDown' }); // skips wip → main
    fireEvent.keyDown(input(), { key: 'Enter' });
    expect(onChange).toHaveBeenCalledWith('main');
  });

  it('disabled option cannot be selected by mouse', () => {
    const { onChange } = renderStrict();
    fireEvent.focus(input());
    fireEvent.mouseDown(screen.getByRole('option', { name: 'wip' }));
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole('listbox')).toBeInTheDocument(); // still open
  });

  it('blur reverts an uncommitted query to the selected label (strict mode)', () => {
    const { onChange } = renderStrict();
    fireEvent.focus(input());
    fireEvent.change(input(), { target: { value: 'de' } });
    fireEvent.blur(input());
    expect(onChange).not.toHaveBeenCalled();
    expect(input()).toHaveValue('main');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('Escape closes only the popover (capture) and reverts the query', () => {
    renderStrict();
    fireEvent.focus(input());
    fireEvent.change(input(), { target: { value: 'de' } });
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(input()).toHaveValue('main');
  });
});

describe('Combobox (free input)', () => {
  it('reports every keystroke through onChange and shows the raw value', () => {
    const onChange = vi.fn();
    render(
      <Combobox
        options={options}
        value="or"
        onChange={onChange}
        allowFreeInput
        ariaLabel="Ref"
      />,
    );
    const el = screen.getByRole('combobox', { name: 'Ref' });
    expect(el).toHaveValue('or');
    fireEvent.change(el, { target: { value: 'orig' } });
    expect(onChange).toHaveBeenCalledWith('orig');
  });
});
