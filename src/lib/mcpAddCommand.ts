// P16: builds the copy-to-clipboard `claude mcp add` line for registering
// Bonsai's embedded MCP server. The CLI's `-H, --header` option is VARIADIC — it
// greedily consumes every following non-option argument — so the server name and
// URL must come BEFORE `--header`, which must be LAST. Placing the URL after
// `--header` (the old shape) made the CLI swallow it as a second header value and
// fail with `missing required argument 'commandOrUrl'`.

export type McpScope = 'user' | 'local';

/** Correctly-ordered `claude mcp add` line: name + URL first, variadic `--header`
 *  LAST. A `local`-scoped command is prefixed with `cd "<repoPath>";` so it
 *  targets the open repo regardless of the terminal's working directory (local
 *  scope resolves by cwd). The `;` separator (not `&&`) is used because the
 *  target machine's default shell is Windows PowerShell 5.1, which rejects `&&`;
 *  `;` chains statements in both PowerShell (5.1 + 7) and POSIX shells. */
export function buildClaudeAddCommand(opts: {
  url: string;
  token: string;
  scope: McpScope;
  repoPath?: string | null;
}): string {
  const base =
    `claude mcp add --transport http --scope ${opts.scope} ` +
    `bonsai ${opts.url} --header "Authorization: Bearer ${opts.token}"`;
  return opts.scope === 'local' && opts.repoPath ? `cd "${opts.repoPath}"; ${base}` : base;
}
