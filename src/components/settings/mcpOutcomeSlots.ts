// P113 §17.3 — the outcome-slot keys for the "AI access (MCP server)" section.
//
// Their own module because TWO files need the same keys and neither may own
// them: `useMcpControls` (src/hooks) writes the notes, `SettingsMcpSection`
// (src/components) renders them. A key that drifted between the two would put a
// row's outcome in another row's slot with no type error to catch it.
//
// The keys ARE the catalog row ids, so `[data-outcome-note]` and
// `[data-setting-id]` agree and a harness can hit-test a slot by row.

import type { McpScope } from '../../lib/mcpAddCommand';
import type { SettingsRowId } from './types';

/** Start/stop the embedded server. */
export const MCP_ENABLED_SLOT: SettingsRowId = 'ai.mcp-enabled';

/** The write-gate. */
export const MCP_ALLOW_WRITE_SLOT: SettingsRowId = 'ai.mcp-allow-write';

/** `claude mcp add`, **keyed by scope**. There are two register rows, so a
 *  single shared key would let the global row's outcome appear under the
 *  repository row (and vice versa) — the two operations are independent and can
 *  succeed and fail separately. */
export const MCP_REGISTER_SLOT: Readonly<Record<McpScope, SettingsRowId>> = {
  user: 'ai.mcp-register-global',
  local: 'ai.mcp-register-repo',
};
