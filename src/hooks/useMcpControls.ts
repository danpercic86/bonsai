// P16 / P16c: the embedded MCP server's runtime state and every control that
// mutates it — start/stop, the `claude mcp add` registration, the write gate,
// and the two deferring consent dialogs. Extracted verbatim from App so the
// container only wires SettingsPanel and the dialogs to it.
import { useCallback, useEffect, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import { ipc } from '../ipc';
import type { McpStatus, UiSettingsPatch } from '../ipc';
import type { PushToast } from '../ToastContext';
import { errorMessage } from '../utils/errors';

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
}

export function useMcpControls(
  pushToast: PushToast,
  activeRepo: string | null,
  handleSettingsChange: (patch: UiSettingsPatch) => void,
): UseMcpControls {
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

  // P16: start/stop the embedded MCP server; keep `mcpStatus` in sync (the
  // `mcp-server-changed` subscription also updates it, but this is immediate).
  const handleSetMcpEnabled = useCallback(
    (enabled: boolean) => {
      ipc.setMcpEnabled(enabled).then(
        (s) => setMcpStatus(s),
        (e) => pushToast('error', `Could not ${enabled ? 'start' : 'stop'} MCP server: ${errorMessage(e)}`),
      );
    },
    [pushToast],
  );

  // P16: run `claude mcp add` for the running server at the chosen scope. Returns
  // the promise so SettingsPanel can clear its in-flight state when it settles.
  const handleRegisterMcp = useCallback(
    (scope: 'user' | 'local'): Promise<void> =>
      ipc.registerMcpWithClaude(scope, activeRepo).then(
        () => pushToast('success', `Registered bonsai with Claude Code (${scope})`),
        (e) => {
          pushToast('error', `Could not register: ${errorMessage(e)}`);
        },
      ),
    [pushToast, activeRepo],
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
      ipc.setMcpAllowWrite(allowWrite).then(
        (s) => setMcpStatus(s),
        (e) =>
          pushToast(
            'error',
            `Could not ${allowWrite ? 'enable' : 'disable'} MCP write access: ${errorMessage(e)}`,
          ),
      );
    },
    [pushToast],
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
  };
}
