// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { JobStatusChangedPayload, McpStatus, RepoChangedPayload, TagAutoSyncEvent } from '../types';

/** Listener registries: the mock's stand-in for the Tauri event system. */
export const repoChangedListeners = new Set<(p: RepoChangedPayload) => void>();
export const jobStatusListeners = new Set<(p: JobStatusChangedPayload) => void>();
// P85 A3: the fire-and-forget fetch tag auto-sync's completion event.
export const tagAutoSyncListeners = new Set<(e: TagAutoSyncEvent) => void>();
// Embedded MCP server (P16). In-memory module state — no real socket; the
// harness only verifies the Settings UI wiring. Fake but plausible port/token.
export const MOCK_MCP_PORT = 8765;
export const MOCK_MCP_TOKEN = 'mock-token-abc123';

export const mockMcp: {
  enabled: boolean;
  allowWrite: boolean;
  activeRepo: string | null;
  listeners: Set<(s: McpStatus) => void>;
} = {
  enabled: false,
  allowWrite: false,
  activeRepo: null,
  listeners: new Set(),
};

export function mcpStatusOf(): McpStatus {
  const toolCount = mockMcp.allowWrite ? 34 : 14;
  if (!mockMcp.enabled) {
    return {
      enabled: false,
      allowWrite: false,
      port: null,
      url: null,
      token: null,
      toolCount,
    };
  }
  const url = `http://127.0.0.1:${MOCK_MCP_PORT}/mcp`;
  return {
    enabled: true,
    allowWrite: mockMcp.allowWrite,
    port: MOCK_MCP_PORT,
    url,
    token: MOCK_MCP_TOKEN,
    toolCount,
  };
}
