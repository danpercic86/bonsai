// P16 §10.5: presentational "AI access (MCP server)" section, extracted from
// SettingsPanel to keep that container under the file-size soft limit. All state,
// consent gating, and start/stop logic stay in SettingsPanel — this component only
// renders the enable/write toggles, the running status + URL/token rows, and the
// two Register-with-Claude-Code scopes (Globally / This repository), each with an
// Add (Tauri `register_mcp_with_claude`) and a Copy action.

import type { McpStatus } from '../ipc';
import { buildClaudeAddCommand, type McpScope } from '../lib/mcpAddCommand';

/** Best-effort clipboard copy (harness + native). Silent on failure — the
 *  values are also visible for manual selection. */
function copyText(text: string): void {
  void navigator.clipboard?.writeText(text).catch(() => {});
}

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
  return (
    <section className="settings-section">
      <h3 className="settings-section-title">AI access (MCP server)</h3>
      <p className="settings-section-desc">
        Run a local MCP server on 127.0.0.1 so an external AI client (e.g. Claude Code) can work
        with the repositories you have open in Bonsai. Access requires the token below. The server
        is read-only unless you allow write access.
      </p>
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={mcpEnabled}
          onChange={(e) => onToggleEnabled(e.target.checked)}
        />
        <span>Enable MCP server</span>
      </label>

      <label className={`settings-checkbox${mcpEnabled ? '' : ' is-disabled'}`}>
        <input
          type="checkbox"
          checked={mcpAllowWrite}
          disabled={!mcpEnabled}
          onChange={(e) => onToggleAllowWrite(e.target.checked)}
        />
        <span>Allow AI to modify repositories</span>
      </label>
      {mcpEnabled && (
        <p className="settings-section-desc">
          Adds staging, commit, merge, and conflict-resolution tools. Changing this restarts the
          server and drops any active connection; the client reconnects automatically.
        </p>
      )}

      {mcpEnabled && mcpStatus !== null ? (
        <p className="settings-ai-status settings-ai-status-ok">
          Running on port {mcpStatus.port} · {mcpStatus.toolCount} tools{' '}
          {mcpStatus.allowWrite ? '(read + write)' : '(read-only)'}
        </p>
      ) : (
        <p className="settings-ai-status">Stopped.</p>
      )}

      {mcpEnabled && mcpStatus !== null && (
        <>
          <div className="settings-control">
            <label className="settings-control-label" htmlFor="settings-mcp-url">
              Server URL
            </label>
            <div className="settings-control-inputs">
              <input
                id="settings-mcp-url"
                className="settings-number settings-mcp-field"
                type="text"
                readOnly
                value={mcpStatus.url ?? ''}
                onFocus={(e) => e.target.select()}
              />
              <button
                type="button"
                className="btn-secondary settings-toggle-btn"
                onClick={() => mcpStatus.url !== null && copyText(mcpStatus.url)}
              >
                Copy
              </button>
            </div>
          </div>

          <div className="settings-control">
            <label className="settings-control-label" htmlFor="settings-mcp-token">
              Bearer token
            </label>
            <div className="settings-control-inputs">
              <input
                id="settings-mcp-token"
                className="settings-number settings-mcp-field"
                type="text"
                readOnly
                value={mcpStatus.token ?? ''}
                onFocus={(e) => e.target.select()}
              />
              <button
                type="button"
                className="btn-secondary settings-toggle-btn"
                onClick={() => mcpStatus.token !== null && copyText(mcpStatus.token)}
              >
                Copy
              </button>
            </div>
          </div>

          {(() => {
            // `mcpStatus.url`/`token` are non-null while running (this block is
            // gated on `mcpStatus !== null` + running); fall back defensively.
            const url = mcpStatus.url ?? '';
            const token = mcpStatus.token ?? '';
            const ready = url !== '' && token !== '';
            const rows: { scope: McpScope; label: string; disabled: boolean }[] = [
              { scope: 'user', label: 'Globally', disabled: !ready },
              {
                scope: 'local',
                label: 'This repository',
                disabled: !ready || repoPath === null,
              },
            ];
            return rows.map((row) => (
              <div className="settings-row" key={row.scope}>
                <span className="settings-control-label">
                  Register with Claude Code · {row.label}
                </span>
                <div className="settings-control-inputs">
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    disabled={row.disabled || mcpRegistering !== null}
                    onClick={() => onRegister(row.scope)}
                  >
                    {mcpRegistering === row.scope ? 'Adding…' : 'Add'}
                  </button>
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    disabled={row.disabled}
                    onClick={() =>
                      copyText(buildClaudeAddCommand({ url, token, scope: row.scope, repoPath }))
                    }
                  >
                    Copy
                  </button>
                </div>
              </div>
            ));
          })()}
        </>
      )}
    </section>
  );
}
