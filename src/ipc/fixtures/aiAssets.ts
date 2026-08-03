// AI-asset fixture data for the browser-harness mock (P24 §7, P26 §6, P24b §7).
// Static seed data only — drift math, validation, and stateful CRUD stay in
// mock.ts. `createRepoState` structuredClones these into per-repo state.

import type { AgentAsset, AiAssetInventory, ProfileStore } from '../types';

// P24 — AI-asset fixture (§7). CLAUDE.md / AGENTS.md / copilot exist; AGENTS.md
// is DRIFTED (its own normalized hash) while copilot is IN SYNC with the
// canonical `claude` (shared hash). One detected `.cursor/rules` dir with 2
// members and `.mcp.json` are `managed:false`. Hashes are opaque 40-hex
// placeholders — only their equality matters to the drift math.
const HASH_CLAUDE = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const HASH_AGENTS = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const HASH_RAW = 'cccccccccccccccccccccccccccccccccccccccc';

/** Canned file bodies served by `readAiAsset` for known paths (§7). */
export const MOCK_ASSET_CONTENT: Record<string, string> = {
  'CLAUDE.md': '# CLAUDE.md\n\nProject instructions for Claude Code.\n',
  'AGENTS.md': '# AGENTS.md\n\nProject instructions, OpenAI/Codex flavor (drifted).\n',
  '.github/copilot-instructions.md':
    '# CLAUDE.md\n\nProject instructions for Claude Code.\n',
  '.cursor/rules/style.mdc': '---\ndescription: style\n---\n\nUse tabs.\n',
  '.cursor/rules/testing.mdc': '---\ndescription: testing\n---\n\nWrite tests.\n',
  '.mcp.json': '{\n  "mcpServers": {}\n}\n',
};

function mockAssetFile(
  path: string,
  contentHash: string,
  normalizedHash: string,
): { path: string; size: number; contentHash: string; normalizedHash: string; modified: number } {
  const size = MOCK_ASSET_CONTENT[path]?.length ?? 0;
  return { path, size, contentHash, normalizedHash, modified: 1_753_900_000 };
}

/** Seed inventory (drift recomputed on every `listAiAssets`, so `drift` here is
 *  just the initial no-override view). */
export const mockInventory: AiAssetInventory = {
  assets: [
    {
      id: 'claude',
      agent: 'Claude Code',
      label: 'CLAUDE.md',
      kind: 'singleFile',
      path: 'CLAUDE.md',
      managed: true,
      exists: true,
      files: [mockAssetFile('CLAUDE.md', HASH_RAW, HASH_CLAUDE)],
    },
    {
      id: 'agents',
      agent: 'Codex/Cursor/Gemini/Zed',
      label: 'AGENTS.md',
      kind: 'singleFile',
      path: 'AGENTS.md',
      managed: true,
      exists: true,
      files: [mockAssetFile('AGENTS.md', HASH_RAW, HASH_AGENTS)],
    },
    {
      id: 'copilot',
      agent: 'GitHub Copilot',
      label: 'copilot-instructions.md',
      kind: 'singleFile',
      path: '.github/copilot-instructions.md',
      managed: true,
      exists: true,
      // Same normalized hash as claude -> IN SYNC.
      files: [mockAssetFile('.github/copilot-instructions.md', HASH_RAW, HASH_CLAUDE)],
    },
    { id: 'gemini', agent: 'Gemini CLI', label: 'GEMINI.md', kind: 'singleFile', path: 'GEMINI.md', managed: true, exists: false, files: [] },
    { id: 'windsurf', agent: 'Windsurf (legacy)', label: '.windsurfrules', kind: 'singleFile', path: '.windsurfrules', managed: true, exists: false, files: [] },
    { id: 'cursorLegacy', agent: 'Cursor (legacy)', label: '.cursorrules', kind: 'singleFile', path: '.cursorrules', managed: true, exists: false, files: [] },
    {
      id: 'cursorRules',
      agent: 'Cursor',
      label: '.cursor/rules/',
      kind: 'rulesDir',
      path: '.cursor/rules',
      managed: true,
      exists: true,
      files: [
        mockAssetFile('.cursor/rules/style.mdc', HASH_RAW, HASH_RAW),
        mockAssetFile('.cursor/rules/testing.mdc', HASH_RAW, HASH_RAW),
      ],
    },
    { id: 'windsurfRules', agent: 'Windsurf', label: '.windsurf/rules/', kind: 'rulesDir', path: '.windsurf/rules', managed: true, exists: false, files: [] },
    { id: 'copilotInstr', agent: 'GitHub Copilot', label: '.github/instructions/', kind: 'rulesDir', path: '.github/instructions', managed: false, exists: false, files: [] },
    { id: 'copilotPrompts', agent: 'GitHub Copilot', label: '.github/prompts/', kind: 'rulesDir', path: '.github/prompts', managed: false, exists: false, files: [] },
    { id: 'claudeDir', agent: 'Claude Code', label: '.claude/ (skills/agents/commands)', kind: 'config', path: '.claude', managed: false, exists: false, files: [] },
    {
      id: 'mcp',
      agent: 'MCP clients',
      label: '.mcp.json',
      kind: 'config',
      path: '.mcp.json',
      managed: false,
      exists: true,
      files: [mockAssetFile('.mcp.json', HASH_RAW, HASH_RAW)],
    },
  ],
  drift: {
    canonicalId: 'claude',
    canonicalHash: HASH_CLAUDE,
    entries: [],
    inSync: false,
  },
};

// --- P26: agent-asset (skills / subagents / slash commands) fixture ---------

/** Seed agent-asset inventory (§6): one valid asset per kind, one invalid agent
 *  (missing required `description`), and one skill flagged `complex` (multi-line
 *  frontmatter, read-only). Kinds/names are pre-sorted (skill<agent<command). */
export const mockAgentAssets: AgentAsset[] = [
  {
    kind: 'skill',
    name: 'code-review',
    path: '.claude/skills/code-review/SKILL.md',
    exists: true,
    frontmatter: [
      { key: 'name', value: 'code-review' },
      { key: 'description', value: 'Reviews a diff for correctness and style' },
    ],
    body: '\n# Code review\n\nReview the staged changes.\n',
    complex: false,
    validation: { valid: true, issues: [] },
  },
  {
    kind: 'skill',
    name: 'release-notes',
    path: '.claude/skills/release-notes/SKILL.md',
    exists: true,
    // Multi-line YAML the editor can't round-trip -> read-only Error.
    frontmatter: [{ key: 'name', value: 'release-notes' }],
    body: '\n# Release notes\n\nSummarize merged PRs.\n',
    complex: true,
    validation: {
      valid: false,
      issues: [
        {
          severity: 'error',
          message:
            "frontmatter uses multi-line YAML this editor can't safely round-trip — edit the file directly",
        },
      ],
    },
  },
  {
    kind: 'agent',
    name: 'test-runner',
    path: '.claude/agents/test-runner.md',
    exists: true,
    frontmatter: [
      { key: 'name', value: 'test-runner' },
      { key: 'description', value: 'Runs the test suite and triages failures' },
      { key: 'tools', value: 'Bash, Read' },
      { key: 'model', value: 'inherit' },
    ],
    body: '\nYou run the project test suite and report failures.\n',
    complex: false,
    validation: { valid: true, issues: [] },
  },
  {
    kind: 'agent',
    name: 'broken',
    path: '.claude/agents/broken.md',
    exists: true,
    // Missing required `description` -> invalid (but NOT complex — still editable).
    frontmatter: [{ key: 'name', value: 'broken' }],
    body: '\nAn incomplete subagent.\n',
    complex: false,
    validation: {
      valid: false,
      issues: [
        { severity: 'error', message: "agent requires frontmatter field 'description'" },
      ],
    },
  },
  {
    kind: 'command',
    name: 'changelog',
    path: '.claude/commands/changelog.md',
    exists: true,
    frontmatter: [
      { key: 'description', value: 'Draft a changelog entry' },
      { key: 'argument-hint', value: '<version>' },
    ],
    body: '\nDraft a changelog entry for $ARGUMENTS.\n',
    complex: false,
    validation: { valid: true, issues: [] },
  },
];

// --- P24b: profiles fixture -------------------------------------------------

/** Rich profile body shared by `opus-rich`'s claude + agents targets, so both
 *  land on the same normalized hash — after activation AGENTS.md flips in-sync. */
export const OPUS_RICH_BODY =
  '# Project instructions (Opus-rich)\n\n' +
  'Detailed, high-context guidance for a top-tier model. Explain rationale, ' +
  'cover edge cases, and prefer thorough answers.\n';

export const CHEAP_TERSE_BODY =
  '# Project instructions (terse)\n\nBe brief. Do the task. No preamble.\n';

/** Seed profile store (§7 + P31 §6): two profiles, with per-worktree
 *  activations seeded — `@main` runs the rich profile, the `feature-login`
 *  linked worktree runs the terse one. `activeProfile` mirrors `@main` (D4). */
export const mockProfiles: ProfileStore = {
  version: 2,
  profiles: [
    {
      name: 'opus-rich',
      description: 'Full-context instructions for a top-tier model.',
      model: 'opus',
      targets: [
        { assetId: 'claude', content: OPUS_RICH_BODY },
        { assetId: 'agents', content: OPUS_RICH_BODY },
      ],
    },
    {
      name: 'cheap-terse',
      description: 'Minimal instructions for a cheap/fast model.',
      model: 'haiku',
      targets: [{ assetId: 'claude', content: CHEAP_TERSE_BODY }],
    },
  ],
  activeProfile: 'opus-rich',
  worktreeActivations: { '@main': 'opus-rich', 'feature-login': 'cheap-terse' },
};
