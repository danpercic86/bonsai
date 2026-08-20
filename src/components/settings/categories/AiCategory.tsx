// P69f §1.1 — the "AI" category page: assistance, run limits, and the embedded
// MCP server. Three leaf sections, each keeping its existing props (§2.3); this
// page only reads the context and hands them down.
//
// P69j re-skinned all three onto the canonical row, so each now renders its own
// catalog groups — Assistance / Runs + Limits + Bulk resolve / AI access. The
// AI-runs `<fieldset disabled>` (UI §5.4) lives inside its own section because it
// spans three of those groups and nothing above it may dim.

import { SettingsAiSection } from '../../SettingsAiSection';
import { SettingsAiRunSection } from '../../SettingsAiRunSection';
import { SettingsMcpSection } from '../../SettingsMcpSection';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';

export function AiCategory() {
  const {
    aiEnabled,
    aiConflictAutonomy,
    aiActive,
    aiAvailability,
    aiRun,
    mcpStatus,
    mcpEnabled,
    mcpAllowWrite,
    mcpRegistering,
    repoPath,
  } = useSettingsValues();
  const { change, setAiEnabled, setMcpEnabled, setMcpAllowWrite, registerMcp } =
    useSettingsActions();

  return (
    <>
      {/* --- Assistance (P13 §8.1, P68g §2.3) --- */}
      <SettingsAiSection
        aiEnabled={aiEnabled}
        aiConflictAutonomy={aiConflictAutonomy}
        aiActive={aiActive}
        aiAvailability={aiAvailability}
        onToggleEnabled={setAiEnabled}
        onChange={change}
      />

      {/* --- Runs / Limits / Bulk resolve (P68g §1): the eight knobs that had
              no UI at all, inside one disabled-when-off fieldset --- */}
      <SettingsAiRunSection aiRun={aiRun} aiActive={aiActive} onChange={change} />

      {/* --- AI access (MCP server) (P16 §10.5) --- */}
      <SettingsMcpSection
        mcpStatus={mcpStatus}
        mcpEnabled={mcpEnabled}
        mcpAllowWrite={mcpAllowWrite}
        repoPath={repoPath}
        mcpRegistering={mcpRegistering}
        onToggleEnabled={setMcpEnabled}
        onToggleAllowWrite={setMcpAllowWrite}
        onRegister={registerMcp}
      />
    </>
  );
}
