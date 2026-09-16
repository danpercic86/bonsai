// P69f §1.1 — the "AI" category page: assistance, run limits, and the embedded
// MCP server. Three leaf sections, each keeping its existing props (§2.3); this
// page only reads the context and hands them down.
//
// P69j re-skinned all three onto the canonical row, so each now renders its own
// catalog groups — Assistance / Runs + Limits + Bulk resolve / AI access. The
// AI-runs `<fieldset disabled>` (UI §5.4) lives inside its own section because it
// spans three of those groups and nothing above it may dim.

import { useEffect, useLayoutEffect } from 'react';

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
    mcpOutcomes,
    mcpAnnounce,
    repoPath,
  } = useSettingsValues();
  const { change, setAiEnabled, setMcpEnabled, setMcpAllowWrite, registerMcp, resetMcpOutcomes } =
    useSettingsActions();

  // P113 §7, "Also clears on: unmount". The four MCP notes are owned by
  // `useMcpControls`, which `App` mounts for the app's WHOLE lifetime, so the
  // note map does not die with the surface that shows it (Dev and Accounts get
  // that for free by owning theirs component-locally). This page IS that
  // surface's lifetime, and it renders `SettingsMcpSection` unconditionally, so
  // its unmount is the section's unmount.
  //
  // THREE things unmount this page, not two. The two §7 names: leaving the
  // category, and closing Settings (`SettingsPanel` returns null when closed).
  // The third is the Settings search box — `SettingsShell` swaps `<Page>` for
  // `SettingsResults` on any non-empty query (`SettingsShell.tsx:65,213`), and
  // `SettingsResults.tsx:82` remounts `AiCategory` only when AI is among the
  // matches, so typing clears the notes too. That is an ACCEPTED early clear,
  // not a §7 trigger: it is fail-safe (clears early, never stale) and an outcome
  // note is ephemeral feedback about an operation the user just performed. Do
  // NOT add code to suppress it.
  //
  // These effects live in the page, not in the section, because the section is
  // presentational ("this component only renders", its own header) and the page
  // is the container that already reads the action bag. §17.3 is untouched: it
  // rules on who OWNS the instance and where the live region renders, and
  // neither moves — the hook still owns it, the announcer still renders in the
  // section, and the per-section live-region count is still 1.
  //
  // Reset on MOUNT as well as on cleanup: cleanup is the §7 rule, and the mount
  // pass covers the one case cleanup cannot — a `report` from an operation that
  // was still in flight when the section went away (press Add, close Settings,
  // the run then fails). Both calls are no-ops on an already-clean instance, so
  // neither costs a render.
  //
  // The mount pass is a LAYOUT effect, deliberately. A passive effect runs after
  // paint, so that remount-after-a-late-`report` case would commit with the note
  // VISIBLE and the announcer holding its text for one frame — a frame of a note
  // §7 says is gone, plus a freshly inserted `role="status"` region that already
  // carries content, which some assistive tech voices. Clearing before paint
  // means neither is ever observable.
  useLayoutEffect(() => {
    resetMcpOutcomes();
  }, [resetMcpOutcomes]);

  // The unmount clear stays passive: nothing paints after it (this subtree is
  // going away), so there is nothing to beat to the screen.
  useEffect(() => resetMcpOutcomes, [resetMcpOutcomes]);

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
        outcomes={mcpOutcomes}
        announce={mcpAnnounce}
      />
    </>
  );
}
