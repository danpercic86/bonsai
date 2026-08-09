/** T3.5 — the presentational Settings*Section leaves: Graph (whole-struct
 *  patches), External tools (template edits + reset), and Updates (state
 *  machine rendering). GitConfig/Profiles sections own IPC and are covered via
 *  their own flows elsewhere; range clamping is unit-tested in settings/ranges. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsGraphSection } from './SettingsGraphSection';
import { SettingsExternalToolsSection } from './SettingsExternalToolsSection';
import { SettingsUpdatesSection } from './SettingsUpdatesSection';
import type { GraphPrefs, UpdateCheckResult } from '../ipc';
import { ROW_HEIGHT_MAX, ROW_HEIGHT_MIN } from '../settings/ranges';

const GRAPH: GraphPrefs = {
  avatarRadius: 9,
  rowHeight: 28,
  laneWidth: 14,
  showSha: true,
  showAuthor: false,
  showDate: true,
  dateBasis: 'author',
  showAheadBehind: true,
  compact: false,
  showSignatureBadge: true,
  showPrBadge: false,
  showCiStatus: false,
};

describe('SettingsGraphSection', () => {
  function renderSection(graph: GraphPrefs = GRAPH) {
    const onChange = vi.fn();
    render(<SettingsGraphSection graph={graph} onChange={onChange} />);
    return onChange;
  }

  it('checkbox toggles patch the WHOLE graph struct with one field flipped', () => {
    const onChange = renderSection();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Author name' }));
    expect(onChange).toHaveBeenCalledWith({ graph: { ...GRAPH, showAuthor: true } });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Short SHA' }));
    expect(onChange).toHaveBeenCalledWith({ graph: { ...GRAPH, showSha: false } });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Compact rows' }));
    expect(onChange).toHaveBeenCalledWith({ graph: { ...GRAPH, compact: true } });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Signature badge' }));
    expect(onChange).toHaveBeenCalledWith({ graph: { ...GRAPH, showSignatureBadge: false } });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Show PR badges' }));
    expect(onChange).toHaveBeenCalledWith({ graph: { ...GRAPH, showPrBadge: true } });
  });

  it('date-basis radios reflect the current value and patch the other choice', () => {
    const onChange = renderSection();
    expect(screen.getByRole('radio', { name: 'Author' })).toBeChecked();
    fireEvent.click(screen.getByRole('radio', { name: 'Committer' }));
    expect(onChange).toHaveBeenCalledWith({ graph: { ...GRAPH, dateBasis: 'committer' } });
  });

  it('geometry sliders render current values and clamp typed input to the range', () => {
    const onChange = renderSection();
    const row = document.getElementById('settings-graph-row')!;
    expect(row).toHaveValue(28);
    fireEvent.change(row, { target: { value: '1' } });
    expect(onChange).toHaveBeenCalledWith({ graph: { ...GRAPH, rowHeight: ROW_HEIGHT_MIN } });
    fireEvent.change(row, { target: { value: '500' } });
    expect(onChange).toHaveBeenCalledWith({ graph: { ...GRAPH, rowHeight: ROW_HEIGHT_MAX } });
  });
});

describe('SettingsExternalToolsSection', () => {
  function renderTools(terminalCommand = '', editorCommand = '') {
    const onChange = vi.fn();
    render(
      <SettingsExternalToolsSection
        terminalCommand={terminalCommand}
        editorCommand={editorCommand}
        onChange={onChange}
      />,
    );
    return onChange;
  }

  it('edits patch the matching template key', () => {
    const onChange = renderTools();
    fireEvent.change(screen.getByLabelText('Terminal command'), {
      target: { value: 'wt -d {path}' },
    });
    expect(onChange).toHaveBeenCalledWith({ terminalCommand: 'wt -d {path}' });
    fireEvent.change(screen.getByLabelText('Editor command'), {
      target: { value: 'code {path}' },
    });
    expect(onChange).toHaveBeenCalledWith({ editorCommand: 'code {path}' });
  });

  it('reset is disabled when empty (auto-detect) and clears a set template', () => {
    const empty = renderTools();
    const resets = screen.getAllByRole('button', { name: 'Reset to auto-detect' });
    expect(resets[0]).toBeDisabled();
    expect(resets[1]).toBeDisabled();
    expect(empty).not.toHaveBeenCalled();
    const onChange = renderTools('wt -d {path}', '');
    const [termReset] = screen
      .getAllByRole('button', { name: 'Reset to auto-detect' })
      .slice(2); // second render's buttons
    expect(termReset).toBeEnabled();
    fireEvent.click(termReset);
    expect(onChange).toHaveBeenCalledWith({ terminalCommand: '' });
  });
});

describe('SettingsUpdatesSection', () => {
  const INFO: UpdateCheckResult = {
    available: true,
    version: '2.0.0',
    notes: 'notes',
    currentVersion: '1.2.3',
  } as UpdateCheckResult;

  function renderUpdates(over: Partial<Parameters<typeof SettingsUpdatesSection>[0]> = {}) {
    const props = {
      currentVersion: '1.2.3',
      autoCheckUpdates: true,
      onToggleAutoCheck: vi.fn(),
      checkState: { status: 'idle' } as const,
      onCheck: vi.fn(),
      onOpenDialog: vi.fn(),
      ...over,
    };
    render(<SettingsUpdatesSection {...props} />);
    return props;
  }

  it('idle: shows version, check button fires, toggle patches', () => {
    const props = renderUpdates();
    expect(screen.getByText('1.2.3')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Check for updates' }));
    expect(props.onCheck).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('checkbox', { name: /Automatically check/ }));
    expect(props.onToggleAutoCheck).toHaveBeenCalledWith(false);
  });

  it('checking disables the button; upToDate and error render their lines', () => {
    renderUpdates({ checkState: { status: 'checking' } });
    expect(screen.getByRole('button', { name: 'Checking…' })).toBeDisabled();
    renderUpdates({ checkState: { status: 'upToDate' } });
    expect(screen.getByText(/up to date/)).toBeInTheDocument();
    renderUpdates({ checkState: { status: 'error', message: 'offline' } });
    expect(screen.getByRole('alert')).toHaveTextContent('offline');
  });

  it('available state offers the What’s-new link into the dialog', () => {
    const props = renderUpdates({ checkState: { status: 'available', info: INFO } });
    expect(screen.getByText(/Version 2\.0\.0 is available/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: "What's new" }));
    expect(props.onOpenDialog).toHaveBeenCalledTimes(1);
  });
});
