// P69f §2.2 — the consent-gated Settings toggles, and nothing else.
//
// Three of the four controls here share one shape: turning the switch ON when
// the matching one-time consent has not been given defers to App's dialog
// instead of patching, while turning it OFF always acts immediately. The fourth
// (`registerMcp`) owns the in-flight scope that disables one "Add" button while
// its run settles. They moved out of `useSettingsPanelAdapter.ts` VERBATIM when
// that file reached 499 of the ~500-line limit; the adapter calls this hook at
// the exact position its `useState` used to sit, so the hook order is unchanged.
//
// The adapter keeps everything else: the two memoised context bags, the
// whole-`UiSettings` reset snapshot, and `resetRow`.
import { useCallback, useState } from 'react';

import { type McpScope } from '../../lib/mcpAddCommand';
import type { SettingsPanelProps } from './useSettingsPanelAdapter';

/** Exactly the props the four gates read (type-only, so no runtime import of
 *  the adapter). */
export type SettingsConsentGateProps = Pick<
  SettingsPanelProps,
  | 'onChange'
  | 'aiConsented'
  | 'onRequestEnableAi'
  | 'mcpConsented'
  | 'onSetMcpEnabled'
  | 'onRequestEnableMcp'
  | 'mcpWriteConsented'
  | 'onSetMcpAllowWrite'
  | 'onRequestEnableMcpWrite'
  | 'onRegisterMcp'
>;

export interface SettingsConsentGates {
  /** The scope whose `claude mcp add` run is pending, or `null`. */
  mcpRegistering: McpScope | null;
  setAiEnabled: (checked: boolean) => void;
  setMcpEnabled: (checked: boolean) => void;
  setMcpAllowWrite: (checked: boolean) => void;
  registerMcp: (scope: McpScope) => void;
}

export function useSettingsConsentGates(
  props: SettingsConsentGateProps,
): SettingsConsentGates {
  const {
    onChange,
    aiConsented,
    onRequestEnableAi,
    mcpConsented,
    onSetMcpEnabled,
    onRequestEnableMcp,
    mcpWriteConsented,
    onSetMcpAllowWrite,
    onRequestEnableMcpWrite,
    onRegisterMcp,
  } = props;

  // In-flight scope for the "Add" registration buttons — disables a button while
  // its `claude mcp add` run is pending.
  const [mcpRegistering, setMcpRegistering] = useState<McpScope | null>(null);

  // Enabling requires one-time consent (§8.1): turning ON without consent defers
  // to App's consent dialog; turning OFF patches immediately (consent is kept).
  const setAiEnabled = useCallback(
    (checked: boolean): void => {
      if (!checked) {
        onChange({ aiEnabled: false });
        return;
      }
      if (aiConsented) onChange({ aiEnabled: true });
      else onRequestEnableAi();
    },
    [onChange, aiConsented, onRequestEnableAi],
  );

  // MCP enable toggle (P16): enabling without consent defers to App's consent
  // dialog; disabling stops immediately.
  const setMcpEnabled = useCallback(
    (checked: boolean): void => {
      if (!checked) {
        onSetMcpEnabled(false);
        return;
      }
      if (mcpConsented) onSetMcpEnabled(true);
      else onRequestEnableMcp();
    },
    [onSetMcpEnabled, mcpConsented, onRequestEnableMcp],
  );

  // MCP write-gate (P16c): only meaningful while the server runs. Turning ON
  // without the stronger write consent defers to App's write-consent dialog;
  // turning OFF flips immediately. Either direction bounces the server.
  const setMcpAllowWrite = useCallback(
    (checked: boolean): void => {
      if (!checked) {
        onSetMcpAllowWrite(false);
        return;
      }
      if (mcpWriteConsented) onSetMcpAllowWrite(true);
      else onRequestEnableMcpWrite();
    },
    [onSetMcpAllowWrite, mcpWriteConsented, onRequestEnableMcpWrite],
  );

  // Run `claude mcp add` for one scope, holding the in-flight scope so its "Add"
  // button disables until the run settles (App owns the toast).
  const registerMcp = useCallback(
    (scope: McpScope): void => {
      setMcpRegistering(scope);
      void onRegisterMcp(scope).finally(() => setMcpRegistering(null));
    },
    [onRegisterMcp],
  );

  return {
    mcpRegistering,
    setAiEnabled,
    setMcpEnabled,
    setMcpAllowWrite,
    registerMcp,
  };
}
