/** Embedded MCP server status for the Settings panel (P16). Mirrors the Rust
 *  `McpStatus`. `enabled` is the live runtime state; `port`/`url`/`token` are
 *  populated only while running. */
export interface McpStatus {
  /** Server running? */
  enabled: boolean;
  /** Write tools registered? Reflects the running server's live gate (P16c). */
  allowWrite: boolean;
  /** Bound port when running, else `null`. */
  port: number | null;
  /** e.g. "http://127.0.0.1:8765/mcp"; `null` when stopped. */
  url: string | null;
  /** Persisted bearer token; `null` when stopped. */
  token: string | null;
  /** 14 (read-only) or 34 (write enabled). */
  toolCount: number;
}

/** Persisted multi-tab session: open tabs (in display order) + the active tab.
 *  `repoId`s are canonical workdir path strings. */
export interface SessionState {
  openRepos: string[];
  activeRepo: string | null;
}
