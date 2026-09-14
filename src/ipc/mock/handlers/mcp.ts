// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { mcpStatusOf, mockMcp } from '../events';
import { delay, query as urlParam } from '../repoState';
import type { AppError, McpStatus, Unsubscribe } from '../../types';

/**
 * P113 §14 — `?mcpFail=enable|register|write|all`: the trigger for outcome rows
 * 11-14, which had none. Each message is the backend's own text, cited:
 *
 *  * `enable`  — `AppError::Io("bind 127.0.0.1:0: {e}")`,
 *    `src-tauri/src/mcp/server.rs` `bind_listener()`. The realistic `{e}` is the
 *    Windows address-in-use error, which is also the only failure a user can
 *    cause by hand (something else already holds the persisted port).
 *  * `register` — `AppError::AiUnavailable("Claude Code CLI not found: {e}")`,
 *    `crates/bonsai-core/src/ai/mod.rs` `register_with_claude()`. By far the
 *    likeliest register failure: no `claude` on PATH.
 *  * `write`   — `AppError::Other("cannot resolve app config dir: {e}")`,
 *    `src-tauri/src/settings.rs` `settings_file()`, reached from
 *    `mcp::set_allow_write` before the server is bounced.
 *
 * None of these is invented: an invented refusal would make the note render text
 * the backend can never send, which is the failure mode §14 exists to prevent.
 */
function mcpFailure(op: 'enable' | 'register' | 'write'): AppError | null {
  const knob = urlParam('mcpFail');
  if (knob !== op && knob !== 'all') return null;
  if (op === 'enable') {
    return {
      kind: 'io',
      message:
        'bind 127.0.0.1:0: Only one usage of each socket address (protocol/network address/port) is normally permitted. (os error 10048)',
    };
  }
  if (op === 'register') {
    return {
      kind: 'aiUnavailable',
      message: 'Claude Code CLI not found: program not found',
    };
  }
  return { kind: 'other', message: 'cannot resolve app config dir: unknown path' };
}

export const mcpHandlers = {
  async getMcpStatus(): Promise<McpStatus> {
    await delay(100);
    return mcpStatusOf();
  },

  async setMcpEnabled(enabled: boolean): Promise<McpStatus> {
    await delay(150);
    const fail = mcpFailure('enable');
    if (fail !== null) throw fail;
    mockMcp.enabled = enabled;
    // Disabling drops the running server's write gate too (a stopped server has
    // no live tools); the setting itself persists via UI settings.
    if (!enabled) mockMcp.allowWrite = false;
    const status = mcpStatusOf();
    // Notify any subscriber, like the backend's `mcp-server-changed` emit.
    for (const cb of mockMcp.listeners) cb(status);
    return status;
  },

  // P16c: flip the write-gate. When the server is running this mirrors the
  // backend BOUNCE (toolCount 14 <-> 34) and re-emits the status; when stopped
  // the flag is remembered so the next enable reflects it.
  async setMcpAllowWrite(allowWrite: boolean): Promise<McpStatus> {
    await delay(150);
    const fail = mcpFailure('write');
    if (fail !== null) throw fail;
    mockMcp.allowWrite = allowWrite;
    const status = mcpStatusOf();
    for (const cb of mockMcp.listeners) cb(status);
    return status;
  },

  async onMcpServerChanged(cb: (s: McpStatus) => void): Promise<Unsubscribe> {
    mockMcp.listeners.add(cb);
    return () => {
      mockMcp.listeners.delete(cb);
    };
  },

  // P16: the harness has no real `claude` CLI, so registration is a no-op that
  // resolves after a short delay (App shows a success toast).
  async registerMcpWithClaude(
    _scope: 'user' | 'local',
    _repoPath: string | null,
  ): Promise<void> {
    await delay(150);
    const fail = mcpFailure('register');
    if (fail !== null) throw fail;
  },

  // P24: AI-asset inventory + drift. Drift is recomputed per call so the
  // optional `canonical` override is demonstrable in the harness.
} satisfies Partial<IpcApi>;
