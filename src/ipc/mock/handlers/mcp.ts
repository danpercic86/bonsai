// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { mcpStatusOf, mockMcp } from '../events';
import { delay } from '../repoState';
import type { McpStatus, Unsubscribe } from '../../types';

export const mcpHandlers = {
  async getMcpStatus(): Promise<McpStatus> {
    await delay(100);
    return mcpStatusOf();
  },

  async setMcpEnabled(enabled: boolean): Promise<McpStatus> {
    await delay(150);
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
  },

  // P24: AI-asset inventory + drift. Drift is recomputed per call so the
  // optional `canonical` override is demonstrable in the harness.
} satisfies Partial<IpcApi>;
