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
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';
import { SettingsSwitchRow } from './settings/SettingsSwitchRow';
import { settingsRowLabelId } from './settings/settingsCatalog';
import type { SettingsRowId } from './settings/types';

/** Best-effort clipboard copy (harness + native). Silent on failure — the
 *  values are also visible for manual selection. */
function copyText(text: string): void {
  void navigator.clipboard?.writeText(text).catch(() => {});
}

const ENABLED = 'ai.mcp-enabled';
const ALLOW_WRITE = 'ai.mcp-allow-write';
const URL_ROW = 'ai.mcp-server-url';
const TOKEN_ROW = 'ai.mcp-token';
const REGISTER: Readonly<Record<McpScope, SettingsRowId>> = {
  user: 'ai.mcp-register-global',
  local: 'ai.mcp-register-repo',
};

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

      <SettingsSwitchRow id={ENABLED} checked={mcpEnabled} onChange={onToggleEnabled} />

      <SettingsSwitchRow
        id={ALLOW_WRITE}
        checked={mcpAllowWrite}
        disabled={!mcpEnabled}
        hint={
          mcpEnabled ? (
            <p className="settings-row-note">
              Adds staging, commit, merge, and conflict-resolution tools. Changing this restarts the
              server and drops any active connection; the client reconnects automatically.
            </p>
          ) : undefined
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
            const disabled = !ready || (scope === 'local' && repoPath === null);
            return (
              <SettingsRow id={rowId} key={scope}>
                <div className="settings-value-copy">
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    /* The row LABEL is this button's accessible name: "Add" alone
                       does not say what is being added, and two rows would then
                       offer two identically-named buttons (UI §7.1). */
                    aria-labelledby={settingsRowLabelId(rowId)}
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
    </SettingsGroup>
  );
}
