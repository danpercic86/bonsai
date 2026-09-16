/**
 * P113 §17.3 — call sites 11-14, the four MCP outcomes the phase-1 sweep missed.
 *
 * They were missed because the sweep searched two DIRECTORIES: `useMcpControls`
 * lives in `src/hooks/`, took `pushToast` as a parameter, and rendered Settings
 * rows anyway. The parameter is gone now, so this suite pins the replacement:
 * the hook writes notes, the section renders them plus its ONE announcer, and
 * the two register rows are keyed by SCOPE so neither can show the other's
 * result.
 */
import { describe, expect, it, vi } from 'vitest';
import { act, render, renderHook, screen, waitFor } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import type { McpStatus } from '../../ipc';
import { appErr } from '../../test/actionHookKit';
import { useMcpControls } from '../../hooks/useMcpControls';
import { SettingsMcpSection } from '../SettingsMcpSection';
import {
  MCP_ALLOW_WRITE_SLOT,
  MCP_ENABLED_SLOT,
  MCP_REGISTER_SLOT,
} from './mcpOutcomeSlots';
import type { SettingsOutcome } from './SettingsOutcomeNote';

const RUNNING: McpStatus = {
  enabled: true,
  allowWrite: false,
  port: 8765,
  url: 'http://127.0.0.1:8765/mcp',
  token: 'tok-123',
  toolCount: 14,
};

/** Byte-for-byte what the backend pushes when the server goes down:
 *  `stopped_status()`, `src-tauri/src/mcp.rs:143`. Written out rather than
 *  spread from `RUNNING` so the fixture documents what Rust actually sends —
 *  `port`/`url`/`token` all drop to null alongside `enabled`. What removes the
 *  register rows is `enabled`, though: `running = mcpEnabled && mcpStatus !== null`
 *  (`SettingsMcpSection.tsx:140`) is the render gate (`:208`), and the adapter
 *  derives `mcpEnabled` from `status.enabled` (`useSettingsPanelAdapter.ts:413`).
 *  The nulled url/token only make
 *  `ready` (`:139`) false, and `ready` gates `disabled` on the Add buttons
 *  (`:226`) — it never unmounts a row. */
const STOPPED: McpStatus = {
  enabled: false,
  allowWrite: false,
  port: null,
  url: null,
  token: null,
  toolCount: 14,
};

function renderSection(
  outcomes: ReadonlyMap<string, SettingsOutcome> = new Map(),
  announce = '',
  over: Partial<React.ComponentProps<typeof SettingsMcpSection>> = {},
) {
  return render(
    <SettingsMcpSection
      mcpStatus={RUNNING}
      mcpEnabled
      mcpAllowWrite={false}
      repoPath="/repo/fixture"
      mcpRegistering={null}
      onToggleEnabled={vi.fn()}
      onToggleAllowWrite={vi.fn()}
      onRegister={vi.fn()}
      outcomes={outcomes}
      announce={announce}
      {...over}
    />,
  );
}

const noteFor = (slot: string) => document.querySelector(`[data-outcome-note="${slot}"]`);

describe('SettingsMcpSection — the four slots (AC5, AC6, AC8)', () => {
  it('mounts all four notes empty when there is no outcome (AC5)', () => {
    renderSection();
    for (const slot of [
      MCP_ENABLED_SLOT,
      MCP_ALLOW_WRITE_SLOT,
      MCP_REGISTER_SLOT.user,
      MCP_REGISTER_SLOT.local,
    ]) {
      const el = noteFor(slot);
      expect(el, slot).not.toBeNull();
      expect(el?.textContent).toBe('');
      // `:empty` is what collapses the chrome, so the element must have NO child
      // nodes at all — a stray whitespace text node would silently defeat it.
      expect(el?.childNodes.length).toBe(0);
    }
  });

  it('has exactly ONE live region, and no note carries a role or aria-live (AC6)', () => {
    const { container } = renderSection(new Map([[MCP_ENABLED_SLOT, { tone: 'error', text: 'x' }]]));
    expect(
      container.querySelectorAll('[aria-live],[role="status"],[role="alert"]'),
    ).toHaveLength(1);
    for (const el of container.querySelectorAll('[data-outcome-note]')) {
      expect(el.hasAttribute('aria-live')).toBe(false);
      expect(el.hasAttribute('role')).toBe(false);
    }
  });

  it('renders the announcement in that one region', () => {
    renderSection(new Map(), 'Registered bonsai with Claude Code (user)');
    expect(screen.getByRole('status')).toHaveTextContent(
      'Registered bonsai with Claude Code (user)',
    );
  });

  it('composes aria-describedby and every id resolves (AC8)', () => {
    const { container } = renderSection();
    const described = [
      ...container.querySelectorAll<HTMLElement>('[aria-describedby]'),
    ];
    expect(described.length).toBeGreaterThanOrEqual(4);
    for (const el of described) {
      const ids = (el.getAttribute('aria-describedby') ?? '').split(' ').filter((i) => i !== '');
      expect(ids.length, el.outerHTML.slice(0, 80)).toBeGreaterThanOrEqual(2);
      // `getElementById`, not a `#id` selector: the catalog ids contain DOTS
      // (`ai.mcp-enabled-help`), which are legal in an id and in
      // `aria-describedby` but are class syntax in CSS.
      for (const id of ids) expect(document.getElementById(id), id).not.toBeNull();
    }
  });

  it("composes the write row's conditional STATE note only while it renders", () => {
    const { rerender } = renderSection();
    const readDescribed = () =>
      document.getElementById('ai.mcp-allow-write-input')?.getAttribute('aria-describedby') ?? '';
    expect(readDescribed()).toContain('mcp-allow-write-note');
    expect(readDescribed()).toContain('mcp-allow-write-outcome');

    rerender(
      <SettingsMcpSection
        mcpStatus={null}
        mcpEnabled={false}
        mcpAllowWrite={false}
        repoPath={null}
        mcpRegistering={null}
        onToggleEnabled={vi.fn()}
        onToggleAllowWrite={vi.fn()}
        onRegister={vi.fn()}
        outcomes={new Map()}
        announce=""
      />,
    );
    // The state note is gone, so its id must not dangle; the OUTCOME note is
    // unconditional and stays — that is the always-mounted shape.
    expect(readDescribed()).not.toContain('mcp-allow-write-note');
    expect(readDescribed()).toContain('mcp-allow-write-outcome');
    expect(noteFor(MCP_ALLOW_WRITE_SLOT)).not.toBeNull();
  });
});

describe('useMcpControls — outcomes replace the deleted pushToast (AC1, AC15)', () => {
  it('reports a failed start into the ENABLED slot and the announcer', async () => {
    vi.spyOn(mockIpc, 'setMcpEnabled').mockRejectedValue(appErr('io', 'bind 127.0.0.1:0: taken'));
    const { result } = renderHook(() => useMcpControls(null, vi.fn()));

    act(() => result.current.handleSetMcpEnabled(true));

    await waitFor(() => {
      expect(result.current.mcpOutcomes.get(MCP_ENABLED_SLOT)).toEqual({
        tone: 'error',
        text: 'Could not start MCP server: bind 127.0.0.1:0: taken',
      });
    });
    expect(result.current.mcpAnnounce).toBe('Could not start MCP server: bind 127.0.0.1:0: taken');
  });

  it('keys register outcomes by SCOPE, so one row never shows the other result', async () => {
    vi.spyOn(mockIpc, 'registerMcpWithClaude').mockImplementation(async (scope) => {
      if (scope === 'local') throw appErr('aiUnavailable', 'Claude Code CLI not found');
    });
    const { result } = renderHook(() => useMcpControls('/repo', vi.fn()));

    await act(async () => {
      await result.current.handleRegisterMcp('user');
    });
    await act(async () => {
      await result.current.handleRegisterMcp('local');
    });

    expect(result.current.mcpOutcomes.get(MCP_REGISTER_SLOT.user)).toEqual({
      tone: 'success',
      text: 'Registered bonsai with Claude Code (user)',
    });
    expect(result.current.mcpOutcomes.get(MCP_REGISTER_SLOT.local)).toEqual({
      tone: 'error',
      text: 'Could not register: Claude Code CLI not found',
    });
  });

  it('clears the note at the operation START, so a repeat announces again (AC15)', async () => {
    vi.spyOn(mockIpc, 'setMcpAllowWrite').mockRejectedValue(appErr('other', 'nope'));
    const { result } = renderHook(() => useMcpControls(null, vi.fn()));
    const seen: string[] = [];

    act(() => result.current.handleSetMcpAllowWrite(true));
    await waitFor(() => expect(result.current.mcpAnnounce).not.toBe(''));
    seen.push(result.current.mcpAnnounce);

    act(() => result.current.handleSetMcpAllowWrite(true));
    // The `begin()` at the start is a commit of its own, so the announcer really
    // goes text -> '' -> text. Asserting the FINAL text would prove nothing: the
    // bug this guards is an ABSENT change.
    expect(result.current.mcpAnnounce).toBe('');
    await waitFor(() => expect(result.current.mcpAnnounce).not.toBe(''));
    seen.push(result.current.mcpAnnounce);

    expect(seen).toEqual([
      'Could not enable MCP write access: nope',
      'Could not enable MCP write access: nope',
    ]);
  });

  it('drops a stale register note when the server is stopped and restarted', async () => {
    vi.spyOn(mockIpc, 'registerMcpWithClaude').mockRejectedValue(appErr('other', 'gone'));
    vi.spyOn(mockIpc, 'setMcpEnabled').mockResolvedValue({ ...RUNNING, enabled: false });
    const { result } = renderHook(() => useMcpControls('/repo', vi.fn()));

    await act(async () => {
      await result.current.handleRegisterMcp('user');
    });
    expect(result.current.mcpOutcomes.has(MCP_REGISTER_SLOT.user)).toBe(true);

    // Stopping unmounts the register rows; the KEY would otherwise survive and
    // the note would reappear on the next enable, after something HAS happened.
    act(() => result.current.handleSetMcpEnabled(false));
    await waitFor(() => {
      expect(result.current.mcpOutcomes.has(MCP_REGISTER_SLOT.user)).toBe(false);
    });
  });
  it('drops a stale register note when the server reports stopped on its OWN (event path)', async () => {
    // `mcpStatus` reaches "stopped" through TWO paths and only one of them runs
    // `handleSetMcpEnabled`. Rust emits `mcp-server-changed` with
    // `stopped_status()` from `stop()` (`src-tauri/src/mcp.rs:398-402`) AND from
    // `start_or_signal_stopped`'s ERROR arm (`:213-218`) — a failed restart
    // during a write-gate bounce, where the only command the user issued was
    // `set_mcp_allow_write`. The register rows unmount either way
    // (`SettingsMcpSection` renders them behind `running`), so a clear that
    // lives in the command continuation alone lets the note resurrect on the
    // next enable. This case NEVER calls `handleSetMcpEnabled` — that is the
    // whole point, and it is why the sibling case above stayed green while this
    // path was unhandled.
    vi.spyOn(mockIpc, 'registerMcpWithClaude').mockRejectedValue(appErr('other', 'gone'));
    let emit: ((s: McpStatus) => void) | null = null;
    vi.spyOn(mockIpc, 'onMcpServerChanged').mockImplementation(async (cb) => {
      emit = cb;
      return () => {};
    });
    const { result } = renderHook(() => useMcpControls('/repo', vi.fn()));
    await waitFor(() => expect(emit).not.toBeNull());

    await act(async () => {
      await result.current.handleRegisterMcp('user');
    });
    expect(result.current.mcpOutcomes.get(MCP_REGISTER_SLOT.user)).toEqual({
      tone: 'error',
      text: 'Could not register: gone',
    });

    act(() => emit?.(STOPPED));

    expect(result.current.mcpOutcomes.has(MCP_REGISTER_SLOT.user)).toBe(false);
  });

  it('resetMcpOutcomes() empties every note AND the announcer (§7)', async () => {
    // §7's "Also clears on: unmount". `AiCategory` calls this on mount and on
    // unmount (`SettingsPanel.test.tsx` pins the wiring); this case pins what
    // the call actually does, so a `reset` that quietly became a no-op fails
    // here instead of passing everywhere.
    vi.spyOn(mockIpc, 'registerMcpWithClaude').mockRejectedValue(appErr('other', 'gone'));
    const { result } = renderHook(() => useMcpControls('/repo', vi.fn()));

    await act(async () => {
      await result.current.handleRegisterMcp('user');
    });
    expect(result.current.mcpOutcomes.size).toBe(1);
    expect(result.current.mcpAnnounce).toBe('Could not register: gone');

    act(() => result.current.resetMcpOutcomes());

    expect(result.current.mcpOutcomes.size).toBe(0);
    expect(result.current.mcpAnnounce).toBe('');
  });

  it('a reset also clears a note that landed while the page was AWAY', async () => {
    // The one case an unmount-only reset cannot catch, and the reason
    // `AiCategory` resets on MOUNT too: press Add, close Settings before the run
    // settles, and the failure reports into a map that outlives the surface.
    // `resetMcpOutcomes` stands in for both lifecycle calls here — that the
    // MOUNT one exists is pinned in `SettingsPanel.test.tsx`.
    let rejectRun: (e: unknown) => void = () => {};
    const pending = new Promise<void>((_resolve, reject) => {
      rejectRun = reject;
    });
    vi.spyOn(mockIpc, 'registerMcpWithClaude').mockReturnValue(pending);
    const { result } = renderHook(() => useMcpControls('/repo', vi.fn()));

    const run = result.current.handleRegisterMcp('user');
    // The section goes away while the run is in flight (the unmount reset).
    act(() => result.current.resetMcpOutcomes());
    await act(async () => {
      rejectRun(appErr('other', 'gone'));
      await run;
    });

    // The late `report` wrote into the surviving map, so the unmount reset
    // demonstrably could NOT have caught this one.
    expect(result.current.mcpOutcomes.get(MCP_REGISTER_SLOT.user)).toEqual({
      tone: 'error',
      text: 'Could not register: gone',
    });

    // Reopening Settings mounts the page again — that pass is what makes §7's
    // "reopening Settings is a clean page" true even for this ordering.
    act(() => result.current.resetMcpOutcomes());
    expect(result.current.mcpOutcomes.size).toBe(0);
    expect(result.current.mcpAnnounce).toBe('');
  });
});
