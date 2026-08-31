/**
 * P69 §4 — AI category rows (UI §1.3 #41–#59): Assistance, Runs, Limits, Bulk
 * resolve, AI access (MCP).
 *
 * No `requires` on the Runs/Limits rows: when AI is off the fieldset is DISABLED,
 * not removed (UI §5.4), and `requires` describes absence, not disablement.
 *
 * Three rows deliberately carry no `reset` pair — `Stop a run that goes quiet`,
 * `Stop a run after a fixed time` and `Set a spend limit per run` together with
 * their number rows. Their `0` is a documented mode sentinel (§3.4), so a ↺ would
 * either disable the feature or re-enable it behind the user's back; the switch is
 * already the off-switch.
 *
 * P69j — `Enable AI features` lost its `reset` for the same family of reasons, and
 * one more. Its default is ON (`defaults.ts`), so the ↺ appears precisely when the
 * user has deliberately turned AI OFF, offering a one-click "turn it back on";
 * and `resetRow` patches `{aiEnabled}` straight through `onChange`, bypassing the
 * consent-aware `setAiEnabled` that every other path to that flag goes through.
 * The two MCP switches already carry no reset for exactly that reason.
 *
 * P69j-1 — four rows deliberately carry no `help`: `ai.repository-access`,
 * `ai.idle-timeout-enabled`, `ai.hard-cap-enabled` and `ai.budget-enabled`. Each
 * renders a STATEFUL `.settings-row-note` from its section component that says the
 * same thing about the CURRENT state, and a row gets one help line, not two. Their
 * vocabulary was folded into `keywords` so Settings search still finds them, and
 * each passes an explicit `describedBy`, so no `{rowId}-help` idref dangles.
 */
import type { SettingsIndexEntry } from '../types';
import { resetKey } from './reset';

export const AI_ENTRIES: readonly SettingsIndexEntry[] = [
  {
    id: 'ai.enabled',
    category: 'ai',
    group: 'Assistance',
    label: 'Enable AI features',
    help: 'Master switch for every Claude-powered feature. Nothing leaves your machine until you consent.',
    keywords: 'claude assistant consent',
    control: 'switch',
  },
  {
    id: 'ai.conflict-resolution',
    category: 'ai',
    group: 'Assistance',
    label: 'Conflict resolution',
    help: 'Whether an AI resolution is proposed for review or written and staged for you.',
    keywords: 'merge resolve autonomy propose',
    control: 'radiogroup',
    reset: resetKey('aiConflictAutonomy', 'Propose & review'),
  },
  {
    id: 'ai.repository-access',
    category: 'ai',
    group: 'Runs',
    label: 'Repository access',
    keywords: 'sandbox files read-only permissions grant reads anthropic',
    control: 'segmented',
    reset: resetKey('aiConflictTools', 'Read-only'),
  },
  {
    id: 'ai.stream-output',
    category: 'ai',
    group: 'Runs',
    label: 'Stream AI output',
    help: 'Show log lines in the AI activity dock as they arrive.',
    keywords: 'log dock live progress',
    control: 'switch',
    reset: resetKey('aiStreamLog', 'On'),
  },
  {
    id: 'ai.stream-partial',
    category: 'ai',
    group: 'Runs',
    label: 'Stream partial replies',
    help: 'Ask the CLI for partial message chunks. Off by default.',
    keywords: 'incremental tokens chunks',
    control: 'switch',
    reset: resetKey('aiIncludePartialMessages', 'Off'),
  },
  {
    id: 'ai.idle-timeout-enabled',
    category: 'ai',
    group: 'Limits',
    label: 'Stop a run that goes quiet',
    keywords: 'idle timeout stall hang quiet silent',
    control: 'switch',
  },
  {
    id: 'ai.idle-timeout-secs',
    category: 'ai',
    group: 'Limits',
    label: 'After',
    help: 'How long a run may stay silent before it is ended.',
    keywords: 'idle seconds timeout quiet',
    control: 'numberSlider',
  },
  {
    id: 'ai.hard-cap-enabled',
    category: 'ai',
    group: 'Limits',
    label: 'Stop a run after a fixed time',
    keywords: 'wall clock timeout duration deadline',
    control: 'switch',
  },
  {
    id: 'ai.hard-cap-secs',
    category: 'ai',
    group: 'Limits',
    label: 'Time limit',
    help: 'The wall-clock deadline for one run.',
    keywords: 'seconds duration hard cap',
    control: 'numberSlider',
  },
  {
    id: 'ai.max-turns',
    category: 'ai',
    group: 'Limits',
    label: 'Replies per run',
    help: 'How many times Claude may answer inside one run.',
    keywords: 'turns iterations',
    control: 'numberSlider',
    reset: resetKey('aiMaxTurns', '6'),
  },
  {
    id: 'ai.budget-enabled',
    category: 'ai',
    group: 'Limits',
    label: 'Set a spend limit per run',
    keywords: 'budget cost usd dollars money spend cap',
    control: 'switch',
  },
  {
    id: 'ai.budget-usd',
    category: 'ai',
    group: 'Limits',
    label: 'Spend limit',
    help: 'The most one run may spend, in US dollars.',
    keywords: 'budget usd dollars cost spend',
    control: 'numberSlider',
  },
  {
    id: 'ai.bulk-batch-size',
    category: 'ai',
    group: 'Bulk resolve',
    label: 'Batch size',
    help: 'The most text Bonsai puts into one bulk run. Larger merges are split, never truncated.',
    keywords: 'bulk resolve chunk kb',
    control: 'numberSlider',
    reset: resetKey('aiBulkMaxBytes', '400 KB'),
  },
  {
    id: 'ai.mcp-enabled',
    category: 'ai',
    group: 'AI access',
    label: 'Enable MCP server',
    help: 'Run a local server so an external AI client can work with your open repositories.',
    keywords: 'model context protocol port tools',
    control: 'switch',
  },
  {
    id: 'ai.mcp-allow-write',
    category: 'ai',
    group: 'AI access',
    label: 'Allow AI to modify repositories',
    help: 'Adds staging, commit, merge and conflict tools. Restarts the server.',
    keywords: 'write mutation grant',
    control: 'switch',
  },
  {
    id: 'ai.mcp-server-url',
    category: 'ai',
    group: 'AI access',
    label: 'Server URL',
    help: 'The address an MCP client connects to.',
    keywords: 'endpoint address port copy',
    control: 'readonly',
    requires: 'mcpRunning',
  },
  {
    id: 'ai.mcp-token',
    category: 'ai',
    group: 'AI access',
    label: 'Bearer token',
    help: 'The secret an MCP client must send.',
    keywords: 'secret auth credential copy',
    control: 'readonly',
    requires: 'mcpRunning',
  },
  {
    id: 'ai.mcp-register-global',
    category: 'ai',
    group: 'AI access',
    label: 'Register with Claude Code · Globally',
    help: 'Add Bonsai to your user-level Claude Code MCP config.',
    keywords: 'install claude code user scope',
    control: 'button',
    requires: 'mcpRunning',
  },
  {
    id: 'ai.mcp-register-repo',
    category: 'ai',
    group: 'AI access',
    label: 'Register with Claude Code · This repository',
    help: 'Add Bonsai to the open repository’s Claude Code MCP config.',
    keywords: 'install claude code local scope',
    control: 'button',
    requires: 'mcpRunning',
  },
];
