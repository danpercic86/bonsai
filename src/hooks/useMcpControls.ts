// P16 / P16c: the embedded MCP server's runtime state and every control that
// mutates it — start/stop, the `claude mcp add` registration, the write gate,
// and the two deferring consent dialogs. Extracted verbatim from App so the
// container only wires SettingsPanel and the dialogs to it.
//
// P113 §17.3 (call sites 11-14): every control here renders a SETTINGS row, so
// none of its outcomes may be a toast — Settings draws inside `.dialog-overlay`
// (z-index 100) and `.toast-stack` is 90, so a toast raised from here is
// unclickable, not merely dim. This hook therefore owns a `useOutcomeNotes`
// instance and returns the note map plus the section's one announcement;
// `SettingsMcpSection` renders both.
//
// It lives in `src/hooks/` and is wired from `App.tsx`, which is exactly why the
// path-scoped lint could never see it: it took `pushToast` as a PARAMETER and
// imported no toast module. **That parameter is now deleted** — after the sweep
// it had no caller left, and a hook that cannot raise a toast cannot regress
// into one. This is the strongest available guard and it costs nothing (§13.1).
import { useCallback, useEffect, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import { ipc } from '../ipc';
import type { McpStatus, UiSettingsPatch } from '../ipc';
import type { SettingsOutcome } from '../components/settings/SettingsOutcomeNote';
import {
  MCP_ALLOW_WRITE_SLOT,
  MCP_ENABLED_SLOT,
  MCP_REGISTER_SLOT,
} from '../components/settings/mcpOutcomeSlots';
import { useOutcomeNotes } from '../components/settings/useOutcomeNotes';
import { errorMessage } from '../utils/errors';

/** Both register slots, module-level so the effect below has a stable dep. */
const REGISTER_SLOTS: readonly string[] = [MCP_REGISTER_SLOT.user, MCP_REGISTER_SLOT.local];

export interface UseMcpControls {
  mcpStatus: McpStatus | null;
  mcpConsentOpen: boolean;
  setMcpConsentOpen: Dispatch<SetStateAction<boolean>>;
  mcpWriteConsentOpen: boolean;
  setMcpWriteConsentOpen: Dispatch<SetStateAction<boolean>>;
  handleSetMcpEnabled: (enabled: boolean) => void;
  handleRegisterMcp: (scope: 'user' | 'local') => Promise<void>;
  handleConfirmMcpConsent: () => void;
  handleSetMcpAllowWrite: (allowWrite: boolean) => void;
  handleConfirmMcpWriteConsent: () => void;
  /** P113 §17.3: slot key (`mcpOutcomeSlots`) → its newest outcome. */
  mcpOutcomes: ReadonlyMap<string, SettingsOutcome>;
  /** The AI-access section's ONE announcement. Rendered by `SettingsMcpSection`
   *  — the live element must live IN the section for the per-section count to
   *  mean anything (§17.3, AC6). */
  mcpAnnounce: string;
  /** §7 ("Also clears on: unmount") — back the four notes out to their mount
   *  state. This instance is owned HERE, per §17.3, but `App` mounts this hook
   *  for the app's whole lifetime while the notes' host section unmounts on a
   *  category change or a Settings close; without this the user would reopen
   *  Settings tomorrow onto yesterday's `Could not register: …`. Called by
   *  `AiCategory` — the container that renders `SettingsMcpSection` — on mount
   *  AND on unmount: unmount is the §7 rule, and mount covers the one case
   *  unmount cannot, a `report` that lands after the section is already gone. */
  resetMcpOutcomes: () => void;
}

export function useMcpControls(
  activeRepo: string | null,
  handleSettingsChange: (patch: UiSettingsPatch) => void,
): UseMcpControls {
  const { notes, announce, begin, report, discardNotes, reset } = useOutcomeNotes();
  // P16: embedded MCP server. `mcpStatus` is the live runtime state (from the
  // backend, kept fresh via `mcp-server-changed`); the one-time consent gates
  // (`mcpConsented` / `mcpWriteConsented`) are persisted settings and live in
  // useUiSettings — these two flags only track the deferring dialogs.
  const [mcpStatus, setMcpStatus] = useState<McpStatus | null>(null);
  const [mcpConsentOpen, setMcpConsentOpen] = useState(false);
  const [mcpWriteConsentOpen, setMcpWriteConsentOpen] = useState(false);

  // P16: load the embedded-MCP status once and stay live via `mcp-server-changed`.
  useEffect(() => {
    let unsub: (() => void) | null = null;
    let cancelled = false;
    ipc.getMcpStatus().then(
      (s) => {
        if (!cancelled) setMcpStatus(s);
      },
      () => {
        // Non-fatal — the Settings section renders a stopped placeholder.
      },
    );
    ipc.onMcpServerChanged((s) => setMcpStatus(s)).then(
      (u) => {
        if (cancelled) u();
        else unsub = u;
      },
      () => {},
    );
    return () => {
      cancelled = true;
      if (unsub !== null) unsub();
    };
  }, []);

  // §7 — the two register rows render behind `running` (`SettingsMcpSection`),
  // so a server that is not running takes their home away while their note KEYS
  // survive in the map and would resurrect on the next enable, after something
  // demonstrably HAS happened.
  //
  // This is the ONE place that observes `mcpStatus` being not-running, and it
  // has to be: the status arrives from TWO directions. `setMcpEnabled`'s resolve is one; the
  // `mcp-server-changed` subscription above is the other, and Rust emits it with
  // `stopped_status()` from THREE sites: `stop()` (`src-tauri/src/mcp.rs:398-404`),
  // `start_or_signal_stopped`'s error arm (`:213-218`) — a failed restart during a
  // write-gate bounce, where `handleSetMcpEnabled` never ran at all — and
  // `set_allow_write`'s already-stopped arm (`:270-279`), where flipping the write
  // gate on a server that is not running re-emits the stopped status with nothing
  // to bounce. Clearing inside the command continuation covered only the first;
  // this effect covers all three, because it keys on the status, not the caller.
  //
  // `discardNotes`, not `begin`: those two setStates (the event's `setMcpStatus`
  // and the rejection's `report`) can land in ONE React batch, so a `begin` here
  // would run after the commit and blank the ALLOW_WRITE announcement that just
  // explained the stop.
  //
  // The condition mirrors the adapter's `mcpEnabled: mcpStatus?.enabled ?? false`
  // and hence the section's `running`: a null status is "not running", and the rows
  // mount if and only if it is `true`, so a not-running status always means
  // their slots have no home.
  //
  // Keyed on the status OBJECT, not on a derived `running` boolean: a boolean
  // dep only fires on a TRANSITION, which would make the clear depend on this
  // hook having first observed `running === true` — an assumption about another
  // component's history. Keyed on the status it depends on nothing: every
  // not-running observation clears, and the repeats are free because
  // `discardNotes` bails when neither key is present (so the mount pass does not
  // even re-render).
  useEffect(() => {
    if (mcpStatus?.enabled === true) return;
    discardNotes(REGISTER_SLOTS);
  }, [mcpStatus, discardNotes]);

  // P16: start/stop the embedded MCP server; keep `mcpStatus` in sync (the
  // `mcp-server-changed` subscription also updates it, but this is immediate).
  const handleSetMcpEnabled = useCallback(
    (enabled: boolean) => {
      begin(MCP_ENABLED_SLOT);
      ipc.setMcpEnabled(enabled).then(
        // The stale-register-note clear does NOT live here: `mcpStatus` reaches
        // "stopped" from two directions and this continuation is only one of
        // them. See the effect below.
        (s) => setMcpStatus(s),
        (e) =>
          report(
            MCP_ENABLED_SLOT,
            'error',
            `Could not ${enabled ? 'start' : 'stop'} MCP server: ${errorMessage(e)}`,
          ),
      );
    },
    [begin, report],
  );

  // P16: run `claude mcp add` for the running server at the chosen scope. Returns
  // the promise so SettingsPanel can clear its in-flight state when it settles.
  const handleRegisterMcp = useCallback(
    (scope: 'user' | 'local'): Promise<void> => {
      // Scope-keyed: the two register rows are independent operations and must
      // not report into each other's slot.
      const slot = MCP_REGISTER_SLOT[scope];
      begin(slot);
      return ipc.registerMcpWithClaude(scope, activeRepo).then(
        () => report(slot, 'success', `Registered bonsai with Claude Code (${scope})`),
        (e) => {
          report(slot, 'error', `Could not register: ${errorMessage(e)}`);
        },
      );
    },
    [activeRepo, begin, report],
  );

  // Enabling the MCP server the first time records consent, then starts it.
  const handleConfirmMcpConsent = useCallback(() => {
    setMcpConsentOpen(false);
    handleSettingsChange({ mcpConsented: true });
    handleSetMcpEnabled(true);
  }, [handleSettingsChange, handleSetMcpEnabled]);

  // P16c: flip the write-gate; the running server BOUNCES (stop+restart on the
  // same token/port), so `mcpStatus` updates both from this resolve and the
  // `mcp-server-changed` re-emit.
  const handleSetMcpAllowWrite = useCallback(
    (allowWrite: boolean) => {
      begin(MCP_ALLOW_WRITE_SLOT);
      ipc.setMcpAllowWrite(allowWrite).then(
        (s) => setMcpStatus(s),
        (e) =>
          report(
            MCP_ALLOW_WRITE_SLOT,
            'error',
            `Could not ${allowWrite ? 'enable' : 'disable'} MCP write access: ${errorMessage(e)}`,
          ),
      );
    },
    [begin, report],
  );

  // First enabling write records the stronger write consent, then flips the gate.
  const handleConfirmMcpWriteConsent = useCallback(() => {
    setMcpWriteConsentOpen(false);
    handleSettingsChange({ mcpWriteConsented: true });
    handleSetMcpAllowWrite(true);
  }, [handleSettingsChange, handleSetMcpAllowWrite]);

  return {
    mcpStatus,
    mcpConsentOpen,
    setMcpConsentOpen,
    mcpWriteConsentOpen,
    setMcpWriteConsentOpen,
    handleSetMcpEnabled,
    handleRegisterMcp,
    handleConfirmMcpConsent,
    handleSetMcpAllowWrite,
    handleConfirmMcpWriteConsent,
    mcpOutcomes: notes,
    mcpAnnounce: announce,
    resetMcpOutcomes: reset,
  };
}
