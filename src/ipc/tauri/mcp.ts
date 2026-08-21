import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { McpStatus, Unsubscribe } from '../types';

export const mcpCommands = {

  // P16: embedded MCP server.
  setActiveRepo(repoId: string | null): Promise<void> {
    return invoke<void>('set_active_repo', { repoId });
  },

  getMcpStatus(): Promise<McpStatus> {
    return invoke<McpStatus>('get_mcp_status');
  },

  setMcpEnabled(enabled: boolean): Promise<McpStatus> {
    return invoke<McpStatus>('set_mcp_enabled', { enabled });
  },

  setMcpAllowWrite(allowWrite: boolean): Promise<McpStatus> {
    return invoke<McpStatus>('set_mcp_allow_write', { allowWrite });
  },

  onMcpServerChanged(cb: (s: McpStatus) => void): Promise<Unsubscribe> {
    return listen<McpStatus>('mcp-server-changed', (e) => cb(e.payload));
  },

  registerMcpWithClaude(scope: 'user' | 'local', repoPath: string | null): Promise<void> {
    return invoke('register_mcp_with_claude', { scope, repoPath });
  },
};
