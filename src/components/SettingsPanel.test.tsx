/** T3.5 — SettingsPanel container wiring: AI-consent gating, MCP enable/write
 *  gating + registration rows, background-job controls (with NumberSlider
 *  clamping at the wiring level), appearance toggles, and close paths.
 *  Range math itself is unit-tested elsewhere (settings/ranges). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsPanel } from './SettingsPanel';
import type { SettingsPanelProps } from './SettingsPanel';
import type { GraphPrefs, McpStatus } from '../ipc';
import type { AiRunPrefs } from '../settings/aiRunPrefs';
import { AUTO_FETCH_INTERVAL_MAX } from '../settings/ranges';

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

/** P68g: the shipped AI-run defaults, including the two LOCKED zeros. */
const AI_RUN: AiRunPrefs = {
  aiConflictTools: 'readOnly',
  aiStreamLog: true,
  aiIncludePartialMessages: false,
  aiIdleTimeoutSecs: 300,
  aiHardCapSecs: 0,
  aiMaxTurns: 6,
  aiMaxBudgetUsd: 0,
  aiBulkMaxBytes: 400_000,
};

const RUNNING: McpStatus = {
  enabled: true,
  allowWrite: false,
  port: 8765,
  url: 'http://127.0.0.1:8765/mcp',
  token: 'tok-123',
  toolCount: 14,
};

// P69d: this suite needs no `resetEffectiveIdentityForTests` ONLY because `repoPath`
// is null below — the Git-config and profiles sections then issue no IPC and write
// nothing into the module-level identity cache. Set a repo path here and this suite
// starts leaking store state between tests; reset it in a beforeEach at that point.
function renderPanel(over: Partial<SettingsPanelProps> = {}) {
  const props: SettingsPanelProps = {
    open: true,
    onClose: vi.fn(),
    theme: 'dark',
    listView: 'flat',
    panelDensity: 'cozy',
    autoFetch: { enabled: true, intervalMinutes: 10 },
    healthRefresh: { enabled: false, intervalMinutes: 30 },
    graph: GRAPH,
    onChange: vi.fn(),
    onToggleTheme: vi.fn(),
    onToggleListView: vi.fn(),
    aiEnabled: false,
    aiConflictAutonomy: 'proposeReview',
    aiConsented: false,
    aiAvailability: null,
    onRequestEnableAi: vi.fn(),
    aiRun: AI_RUN,
    mcpStatus: null,
    mcpConsented: false,
    onSetMcpEnabled: vi.fn(),
    onRequestEnableMcp: vi.fn(),
    mcpWriteConsented: false,
    onSetMcpAllowWrite: vi.fn(),
    onRequestEnableMcpWrite: vi.fn(),
    repoPath: null,
    profiles: [],
    terminalCommand: '',
    editorCommand: '',
    onRegisterMcp: vi.fn(async () => {}),
    onShowOnboarding: vi.fn(),
    updateCurrentVersion: '1.2.3',
    autoCheckUpdates: true,
    updateState: { status: 'idle' },
    onCheckUpdate: vi.fn(),
    onOpenUpdateDialog: vi.fn(),
    ...over,
  };
  return { ...render(<SettingsPanel {...props} />), props };
}

describe('SettingsPanel', () => {
  it('renders nothing when closed', () => {
    const { container } = renderPanel({ open: false });
    expect(container).toBeEmptyDOMElement();
  });

  it('✕ and backdrop close; a click inside the card does not', () => {
    const { props, container } = renderPanel();
    fireEvent.mouseDown(screen.getByRole('dialog', { name: 'Settings' }));
    expect(props.onClose).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(props.onClose).toHaveBeenCalledTimes(1);
    fireEvent.mouseDown(container.querySelector('.dialog-overlay')!);
    expect(props.onClose).toHaveBeenCalledTimes(2);
  });

  it('appearance: theme/list buttons show the current value and fire the toggles', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('button', { name: 'Dark' }));
    expect(props.onToggleTheme).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Flat' }));
    expect(props.onToggleListView).toHaveBeenCalledTimes(1);
  });

  // P67c §7.1: the density control is ONE toggling button showing the current
  // value (same idiom as Theme / File lists), and it must patch density ALONE.
  it('appearance: panel-density button flips cozy → compact via a lone patch', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('button', { name: 'Cozy' }));
    expect(props.onChange).toHaveBeenCalledWith({ panelDensity: 'compact' });
  });

  it('appearance: panel-density button flips compact → cozy', () => {
    const { props } = renderPanel({ panelDensity: 'compact' });
    fireEvent.click(screen.getByRole('button', { name: 'Compact' }));
    expect(props.onChange).toHaveBeenCalledWith({ panelDensity: 'cozy' });
  });

  // P69d: the two rows are now "Fetch every" / "Refresh every" (UI §5.3.7). This is a
  // RENAME, not a weakening — the id-based assertions below are untouched, and the
  // accessible names are additionally pinned in the a11y test that follows.
  it('auto-fetch: checkbox + interval patch; interval clamps to the range max', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('checkbox', { name: /Enable auto-fetch/ }));
    expect(props.onChange).toHaveBeenCalledWith({
      autoFetch: { enabled: false, intervalMinutes: 10 },
    });
    const interval = screen.getByRole('spinbutton', { name: 'Fetch every' });
    expect(interval).toBe(document.getElementById('settings-auto-fetch-interval'));
    fireEvent.change(interval, { target: { value: '99999' } });
    expect(props.onChange).toHaveBeenCalledWith({
      autoFetch: { enabled: true, intervalMinutes: AUTO_FETCH_INTERVAL_MAX },
    });
  });

  it('health-refresh interval slider is disabled while the job is off', () => {
    renderPanel();
    expect(screen.getByRole('spinbutton', { name: 'Refresh every' })).toBeDisabled();
    expect(document.getElementById('settings-health-refresh-interval')).toBeDisabled();
    expect(document.getElementById('settings-auto-fetch-interval')).toBeEnabled();
  });

  it('the two background-job intervals have DISTINCT accessible names', () => {
    renderPanel();
    // Both rows used to be labelled "Interval": two controls, one accessible name,
    // indistinguishable to a screen reader and to a test (UI §5.3.7 MUST-FIX).
    expect(screen.queryAllByRole('spinbutton', { name: 'Interval' })).toHaveLength(0);
    expect(screen.getAllByRole('spinbutton', { name: 'Fetch every' })).toHaveLength(1);
    expect(screen.getAllByRole('spinbutton', { name: 'Refresh every' })).toHaveLength(1);
    // The range twins carry the same names (NumberSlider aria-labels them).
    expect(screen.getAllByRole('slider', { name: 'Fetch every' })).toHaveLength(1);
    expect(screen.getAllByRole('slider', { name: 'Refresh every' })).toHaveLength(1);
    // Ids are unchanged — the deep-link/id-based assertions above still resolve.
    expect(document.getElementById('settings-auto-fetch-interval')).not.toBeNull();
    expect(document.getElementById('settings-health-refresh-interval')).not.toBeNull();
  });

  it('AI enable without consent defers to onRequestEnableAi (no direct patch)', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('checkbox', { name: /Enable AI features/ }));
    expect(props.onRequestEnableAi).toHaveBeenCalledTimes(1);
    expect(props.onChange).not.toHaveBeenCalledWith({ aiEnabled: true });
  });

  it('AI enable with prior consent patches directly; disabling always patches', () => {
    const { props } = renderPanel({ aiConsented: true });
    fireEvent.click(screen.getByRole('checkbox', { name: /Enable AI features/ }));
    expect(props.onChange).toHaveBeenCalledWith({ aiEnabled: true });
    expect(props.onRequestEnableAi).not.toHaveBeenCalled();
    const off = renderPanel({ aiEnabled: true, aiConsented: true });
    fireEvent.click(off.getAllByRole('checkbox', { name: /Enable AI features/ })[1]);
    expect(off.props.onChange).toHaveBeenCalledWith({ aiEnabled: false });
  });

  // P68g §2.3 acceptance 13: the second radio is "Resolve automatically" — the old
  // "Auto-resolve, then review" promised a review step that does not happen.
  it('autonomy radios are disabled until AI is active, then patch the choice', () => {
    renderPanel();
    expect(screen.getByRole('radio', { name: /Propose & review/ })).toBeDisabled();
    expect(screen.queryByRole('radio', { name: /Auto-resolve, then review/ })).toBeNull();
    const active = renderPanel({ aiEnabled: true, aiConsented: true });
    const auto = active.getAllByRole('radio', { name: 'Resolve automatically' })[1];
    expect(auto).toBeEnabled();
    fireEvent.click(auto);
    expect(active.props.onChange).toHaveBeenCalledWith({ aiConflictAutonomy: 'autoResolve' });
  });

  // The autonomy consequence must be readable BEFORE the choice, so BOTH hints
  // render whatever is selected — and the autoResolve one says "no review step".
  it('both autonomy hints render, and each radio points at its own', () => {
    renderPanel({ aiEnabled: true, aiConsented: true });
    expect(
      screen.getByText(/Each result opens as a proposal\. Nothing is written to your files/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/written to your files and staged for you, with no review step/),
    ).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Propose & review' })).toHaveAttribute(
      'aria-describedby',
      'ai-autonomy-propose-hint',
    );
    expect(screen.getByRole('radio', { name: 'Resolve automatically' })).toHaveAttribute(
      'aria-describedby',
      'ai-autonomy-auto-hint',
    );
  });

  // P68g §1: the AI-runs section is wired through the container, inert until AI is
  // active, and patches one field at a time.
  it('AI runs: the section is inert until AI is active, then patches through', () => {
    renderPanel();
    expect(screen.getByRole('button', { name: /Repository access/ })).toBeDisabled();
    expect(
      screen.getByText('Turn on “Enable AI features” above to change these.'),
    ).toBeInTheDocument();

    const active = renderPanel({ aiEnabled: true, aiConsented: true });
    const access = active.getAllByRole('button', { name: /Repository access/ })[1];
    expect(access).toBeEnabled();
    fireEvent.click(access);
    expect(active.props.onChange).toHaveBeenCalledWith({ aiConflictTools: 'none' });
    fireEvent.click(active.getAllByRole('checkbox', { name: 'Stream AI output' })[1]);
    expect(active.props.onChange).toHaveBeenCalledWith({ aiStreamLog: false });
  });

  it('AI availability line: probing, installed detail, and not-found warning', () => {
    renderPanel();
    expect(screen.getByText(/Checking for the Claude Code CLI/)).toBeInTheDocument();
    renderPanel({
      aiAvailability: { installed: true, loggedIn: true, version: '1.0.0', detail: 'claude 1.0.0' },
    });
    expect(screen.getByText('claude 1.0.0')).toBeInTheDocument();
    renderPanel({
      aiAvailability: { installed: false, loggedIn: false, version: null, detail: 'not found' },
    });
    expect(screen.getByText(/Claude Code CLI not found on PATH/)).toBeInTheDocument();
  });

  it('MCP enable without consent defers; with consent starts directly; off stops', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('checkbox', { name: /Enable MCP server/ }));
    expect(props.onRequestEnableMcp).toHaveBeenCalledTimes(1);
    expect(props.onSetMcpEnabled).not.toHaveBeenCalled();
    const consented = renderPanel({ mcpConsented: true });
    fireEvent.click(consented.getAllByRole('checkbox', { name: /Enable MCP server/ })[1]);
    expect(consented.props.onSetMcpEnabled).toHaveBeenCalledWith(true);
    const running = renderPanel({ mcpStatus: RUNNING, mcpConsented: true });
    fireEvent.click(running.getAllByRole('checkbox', { name: /Enable MCP server/ })[2]);
    expect(running.props.onSetMcpEnabled).toHaveBeenCalledWith(false);
  });

  it('MCP write toggle: disabled while stopped; consent-gated when enabling', () => {
    renderPanel();
    expect(
      screen.getByRole('checkbox', { name: /Allow AI to modify repositories/ }),
    ).toBeDisabled();
    const running = renderPanel({ mcpStatus: RUNNING, mcpConsented: true });
    const writeBox = running.getAllByRole('checkbox', { name: /Allow AI to modify/ })[1];
    fireEvent.click(writeBox);
    expect(running.props.onRequestEnableMcpWrite).toHaveBeenCalledTimes(1);
    expect(running.props.onSetMcpAllowWrite).not.toHaveBeenCalled();
  });

  it('MCP running status line shows port/tool-count; stopped shows Stopped', () => {
    renderPanel();
    expect(screen.getByText('Stopped.')).toBeInTheDocument();
    renderPanel({ mcpStatus: RUNNING });
    expect(screen.getByText(/Running on port 8765 · 14 tools/)).toBeInTheDocument();
    expect(screen.getByDisplayValue('http://127.0.0.1:8765/mcp')).toBeInTheDocument();
    expect(screen.getByDisplayValue('tok-123')).toBeInTheDocument();
  });

  it('MCP registration: Add fires onRegisterMcp; repo scope needs an open repo', async () => {
    let settle!: () => void;
    const onRegisterMcp = vi.fn(
      () => new Promise<void>((resolve) => { settle = resolve; }),
    );
    renderPanel({ mcpStatus: RUNNING, repoPath: null, onRegisterMcp });
    const adds = screen.getAllByRole('button', { name: 'Add' });
    expect(adds).toHaveLength(2);
    expect(adds[1]).toBeDisabled(); // 'This repository' without an open repo
    fireEvent.click(adds[0]);
    expect(onRegisterMcp).toHaveBeenCalledWith('user');
    expect(screen.getByRole('button', { name: 'Adding…' })).toBeDisabled();
    settle();
    await vi.waitFor(() =>
      expect(screen.queryByRole('button', { name: 'Adding…' })).not.toBeInTheDocument(),
    );
  });

  it('"Show welcome tour" fires onShowOnboarding; Updates row shows the version', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('button', { name: 'Show welcome tour' }));
    expect(props.onShowOnboarding).toHaveBeenCalledTimes(1);
    expect(screen.getByText('1.2.3')).toBeInTheDocument();
  });
});
