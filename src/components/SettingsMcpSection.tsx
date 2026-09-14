// P16 §10.5: presentational "AI access (MCP server)" section, extracted from
// SettingsPanel to keep that container under the file-size soft limit. All state,
// consent gating, and start/stop logic stay in SettingsPanel — this component only
// renders the enable/write toggles, the running status + URL/token rows, and the
// two Register-with-Claude-Code scopes (Globally / This repository), each with an
// Add (Tauri `register_mcp_with_claude`) and a Copy action.
//
// P69j: re-skinned onto the canonical row (UI §5.1) inside the AI category's
// "AI access" group. The two checkboxes are switches (a CSS skin over the SAME
// native checkboxes, so the consent gating and every `getByRole('checkbox')`
// query are untouched) and the value+Copy pairs are stacked rows.
// **Every string in this file is frozen (UI §8) — layout moved, words did not.**

import type { McpStatus } from '../ipc';
import { buildClaudeAddCommand, type McpScope } from '../lib/mcpAddCommand';
import {
  MCP_ALLOW_WRITE_SLOT,
  MCP_ENABLED_SLOT,
  MCP_REGISTER_SLOT,
} from './settings/mcpOutcomeSlots';
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsOutcomeNote, type SettingsOutcome } from './settings/SettingsOutcomeNote';
import { SettingsRow } from './settings/SettingsRow';
import { SettingsSwitchRow } from './settings/SettingsSwitchRow';
import { settingsRowHelpId, settingsRowLabelId } from './settings/settingsCatalog';
import type { SettingsRowId } from './settings/types';

/** Best-effort clipboard copy (harness + native). Silent on failure — the
 *  values are also visible for manual selection. */
function copyText(text: string): void {
  void navigator.clipboard?.writeText(text).catch(() => {});
}

const ENABLED = MCP_ENABLED_SLOT;
const ALLOW_WRITE = MCP_ALLOW_WRITE_SLOT;
const URL_ROW = 'ai.mcp-server-url';
const TOKEN_ROW = 'ai.mcp-token';
const REGISTER = MCP_REGISTER_SLOT;

/** P113 §9 - the outcome elements' DOM ids, composed onto each control's
 *  `aria-describedby`. Literals because each row exists exactly once here. */
const ENABLED_OUTCOME_ID = 'mcp-enabled-outcome';
const ALLOW_WRITE_OUTCOME_ID = 'mcp-allow-write-outcome';
/** §17.3 - the write row's STATE note is conditional (`mcpEnabled ? ... :
 *  undefined`), so it needs an id of its own for the composition below; the
 *  OUTCOME note beside it is unconditional, which is the always-mounted shape
 *  `:empty` collapses. */
const ALLOW_WRITE_NOTE_ID = 'mcp-allow-write-note';
const registerOutcomeId = (scope: McpScope): string => `mcp-register-${scope}-outcome`;

export interface SettingsMcpSectionProps {
  /** Live runtime status (null until first loaded). */
  mcpStatus: McpStatus | null;
  /** Derived: whether the server is currently enabled/running. */
  mcpEnabled: boolean;
  /** Derived: whether the write-gate is on. */
  mcpAllowWrite: boolean;
  /** Path of the open repo, or null — gates the `local`-scope registration row. */
  repoPath: string | null;
  /** In-flight registration scope; disables that scope's Add button while pending. */
  mcpRegistering: McpScope | null;
  /** Enable/disable the server (consent handled upstream). */
  onToggleEnabled(checked: boolean): void;
  /** Flip the write-gate (write-consent handled upstream). */
  onToggleAllowWrite(checked: boolean): void;
  /** Run `claude mcp add` for the given scope. */
  onRegister(scope: McpScope): void;
  /** P113 §17.3 - slot key (`mcpOutcomeSlots`) to its newest outcome, owned by
   *  `useMcpControls`. This section raises NO toasts: it renders inside
   *  `.dialog-overlay` (z-index 100) while `.toast-stack` is 90, so a toast from
   *  here is unclickable, not merely dim (ui-reference §12.14). */
  outcomes: ReadonlyMap<string, SettingsOutcome>;
  /** The section's ONE announcement, written by the same `report()` call that
   *  wrote the note. Rendered here, not upstream, so AC6's per-section
   *  live-region count of 1 is a property of this component. */
  announce: string;
}

/**
 * A read-only value + its Copy button, in one stacked row (UI §5.1).
 *
 * `copyLabel` is passed IN rather than derived from the catalog: the label there
 * is title-cased for a row heading (`Bearer token` → `Copy Bearer token`), and
 * four buttons whose whole accessible name is `Copy` are indistinguishable to a
 * screen-reader user who lands on one out of context — a search result can show
 * this row entirely alone. The VISIBLE word stays `Copy` (every string in this
 * file is frozen, UI §8) and each name starts with it, so WCAG 2.5.3 holds.
 */
function ValueRow({
  id,
  controlId,
  value,
  copyLabel,
}: {
  id: SettingsRowId;
  controlId: string;
  value: string;
  copyLabel: string;
}) {
  return (
    <SettingsRow id={id} controlId={controlId} stacked>
      <div className="settings-value-copy">
        <input
          id={controlId}
          className="settings-text settings-mcp-field"
          type="text"
          readOnly
          value={value}
          onFocus={(e) => e.target.select()}
        />
        <button
          type="button"
          className="btn-secondary"
          aria-label={copyLabel}
          onClick={() => copyText(value)}
        >
          Copy
        </button>
      </div>
    </SettingsRow>
  );
}

export function SettingsMcpSection({
  mcpStatus,
  mcpEnabled,
  mcpAllowWrite,
  repoPath,
  mcpRegistering,
  onToggleEnabled,
  onToggleAllowWrite,
  onRegister,
  outcomes,
  announce,
}: SettingsMcpSectionProps) {
  // `mcpStatus.url`/`token` are non-null while running; fall back defensively.
  const url = mcpStatus?.url ?? '';
  const token = mcpStatus?.token ?? '';
  const ready = url !== '' && token !== '';
  const running = mcpEnabled && mcpStatus !== null;

  return (
    <SettingsGroup id="ai-access" title="AI access">
      <p className="settings-group-lead">
        Run a local MCP server on 127.0.0.1 so an external AI client (e.g. Claude Code) can work
        with the repositories you have open in Bonsai. Access requires the token below. The server
        is read-only unless you allow write access.
      </p>

      <SettingsSwitchRow
        id={ENABLED}
        checked={mcpEnabled}
        /* §9: composed, never replaced - the catalog help stays, the outcome id
           is appended and is always present (an id resolving to an empty element
           reads as nothing, while mutating the attribute can itself re-announce). */
        describedBy={`${settingsRowHelpId(ENABLED)} ${ENABLED_OUTCOME_ID}`}
        hint={
          <SettingsOutcomeNote
            slot={ENABLED}
            id={ENABLED_OUTCOME_ID}
            outcome={outcomes.get(ENABLED) ?? null}
          />
        }
        onChange={onToggleEnabled}
      />

      <SettingsSwitchRow
        id={ALLOW_WRITE}
        checked={mcpAllowWrite}
        disabled={!mcpEnabled}
        /* The state note is conditional, so its id joins only while it renders;
           the outcome id is unconditional. Order is help then state then outcome. */
        describedBy={[
          settingsRowHelpId(ALLOW_WRITE),
          ...(mcpEnabled ? [ALLOW_WRITE_NOTE_ID] : []),
          ALLOW_WRITE_OUTCOME_ID,
        ].join(' ')}
        hint={
          <>
            {mcpEnabled ? (
              <p className="settings-row-note" id={ALLOW_WRITE_NOTE_ID}>
                Adds staging, commit, merge, and conflict-resolution tools. Changing this restarts
                the server and drops any active connection; the client reconnects automatically.
              </p>
            ) : undefined}
            {/* The SECOND tenant of the help slot (§6.1): the state note above is
                never replaced, and this one is mounted even while the row is
                disabled - `:empty` gives it zero height (AC11). */}
            <SettingsOutcomeNote
              slot={ALLOW_WRITE}
              id={ALLOW_WRITE_OUTCOME_ID}
              outcome={outcomes.get(ALLOW_WRITE) ?? null}
            />
          </>
        }
        onChange={onToggleAllowWrite}
      />

      {running ? (
        <p className="settings-ai-status settings-ai-status-ok">
          Running on port {mcpStatus.port} · {mcpStatus.toolCount} tools{' '}
          {mcpStatus.allowWrite ? '(read + write)' : '(read-only)'}
        </p>
      ) : (
        <p className="settings-ai-status">Stopped.</p>
      )}

      {running && (
        <>
          <ValueRow
            id={URL_ROW}
            controlId="settings-mcp-url"
            value={url}
            copyLabel="Copy server URL"
          />
          <ValueRow
            id={TOKEN_ROW}
            controlId="settings-mcp-token"
            value={token}
            copyLabel="Copy bearer token"
          />

          {(['user', 'local'] as const).map((scope) => {
            const rowId = REGISTER[scope];
            const outcomeId = registerOutcomeId(scope);
            const disabled = !ready || (scope === 'local' && repoPath === null);
            return (
              <SettingsRow
                id={rowId}
                key={scope}
                hint={
                  <SettingsOutcomeNote
                    slot={rowId}
                    id={outcomeId}
                    outcome={outcomes.get(rowId) ?? null}
                  />
                }
              >
                <div className="settings-value-copy">
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    /* The row LABEL is this button's accessible name: "Add" alone
                       does not say what is being added, and two rows would then
                       offer two identically-named buttons (UI §7.1). */
                    aria-labelledby={settingsRowLabelId(rowId)}
                    /* §9: the row's catalog help plus this scope's own outcome.
                       Scope-keyed, so the global row never describes itself with
                       the repository row's result. */
                    aria-describedby={`${settingsRowHelpId(rowId)} ${outcomeId}`}
                    disabled={disabled || mcpRegistering !== null}
                    onClick={() => onRegister(scope)}
                  >
                    {mcpRegistering === scope ? 'Adding…' : 'Add'}
                  </button>
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    /* Same reason as `ValueRow`'s Copy: `Copy` alone does not
                       say WHICH command, and both scopes offer one. Visible
                       text unchanged (UI §8), name starts with it (2.5.3). */
                    aria-label={
                      scope === 'user'
                        ? 'Copy command to register globally'
                        : 'Copy command to register for this repository'
                    }
                    disabled={disabled}
                    onClick={() =>
                      copyText(buildClaudeAddCommand({ url, token, scope, repoPath }))
                    }
                  >
                    Copy
                  </button>
                </div>
              </SettingsRow>
            );
          })}
        </>
      )}

      {/* P113 §8/§17.3 - the section's ONE live region, always mounted so its
          text always arrives in a LATER commit than its mount (the condition for
          being announced at all). Every `[data-outcome-note]` above is
          description-only: no `aria-live`, no `role`. AI-access count: 1. */}
      <p className="sr-only" role="status" aria-live="polite">
        {announce}
      </p>
    </SettingsGroup>
  );
}
