// P16 / P16c — the App-level MCP wiring: the ONE place that adapts
// `useMcpControls` into the three prop surfaces App renders it through, so the
// container holds a single `mcp` binding instead of a 13-name destructure plus
// eleven threaded props.
//
// Nothing here decides anything: it is the same hook call, the same inline
// dialog-open arrows (deliberately NOT memoised — they were inline in App's JSX
// and their per-render identity is preserved), and the same `||` the modal guard
// used. `useMcpControls` still owns all MCP state, IPC and outcome notes.
import { useMcpControls } from './useMcpControls';
import type { UiSettingsPatch } from '../ipc';
import type { SettingsPanelProps } from '../components/SettingsPanel';

/** The MCP half of `SettingsPanelProps` — spread into `SettingsPanel` by App.
 *  Typed as a `Pick` so tsc fails if a prop is added, dropped or retyped. */
export type McpSettingsProps = Pick<
  SettingsPanelProps,
  | 'mcpStatus'
  | 'mcpOutcomes'
  | 'mcpAnnounce'
  | 'onResetMcpOutcomes'
  | 'mcpConsented'
  | 'onSetMcpEnabled'
  | 'onRequestEnableMcp'
  | 'mcpWriteConsented'
  | 'onSetMcpAllowWrite'
  | 'onRequestEnableMcpWrite'
  | 'onRegisterMcp'
>;

/** One consent dialog's props (`McpConsentDialog` / `McpWriteConsentDialog`
 *  share the shape). */
export interface McpConsentDialogProps {
  open: boolean;
  onConfirm(): void;
  onCancel(): void;
}

export interface McpWiring {
  /** `mcpConsentOpen || mcpWriteConsentOpen` — App's `globalModalOpen` term. */
  modalOpen: boolean;
  settingsProps: McpSettingsProps;
  consentDialog: McpConsentDialogProps;
  writeConsentDialog: McpConsentDialogProps;
}

export function useMcpWiring(
  activeRepo: string | null,
  handleSettingsChange: (patch: UiSettingsPatch) => void,
  mcpConsented: boolean,
  mcpWriteConsented: boolean,
): McpWiring {
  const {
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
    mcpOutcomes,
    mcpAnnounce,
    resetMcpOutcomes,
  } = useMcpControls(activeRepo, handleSettingsChange);

  return {
    modalOpen: mcpConsentOpen || mcpWriteConsentOpen,
    settingsProps: {
      mcpStatus,
      mcpOutcomes,
      mcpAnnounce,
      onResetMcpOutcomes: resetMcpOutcomes,
      mcpConsented,
      onSetMcpEnabled: handleSetMcpEnabled,
      onRequestEnableMcp: () => setMcpConsentOpen(true),
      mcpWriteConsented,
      onSetMcpAllowWrite: handleSetMcpAllowWrite,
      onRequestEnableMcpWrite: () => setMcpWriteConsentOpen(true),
      onRegisterMcp: handleRegisterMcp,
    },
    consentDialog: {
      open: mcpConsentOpen,
      onConfirm: handleConfirmMcpConsent,
      onCancel: () => setMcpConsentOpen(false),
    },
    writeConsentDialog: {
      open: mcpWriteConsentOpen,
      onConfirm: handleConfirmMcpWriteConsent,
      onCancel: () => setMcpWriteConsentOpen(false),
    },
  };
}
