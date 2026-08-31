/**
 * P68 §8.3 — the ten streaming AI-run settings in the mock IPC layer: defaults,
 * tolerant parsing and the clamp mirror.
 *
 * Its own module (not appended to `persistence.ts`, which is already ~360 lines)
 * because it is a self-contained numeric guard: `readUiSettings` calls
 * `parseAiRunSettings` (defend a hand-edited localStorage blob, mirroring the Rust
 * `load_from` → `clamp_ai_settings`) and `setUiSettings` calls `clampAiRunSettings`
 * on the merged patch (mirroring `apply_patch` → `clamp_ai_settings`).
 *
 * The ranges live in `src/settings/ranges.ts` beside the P11/P30/P51 ones, so the
 * numbers are stated once on the frontend and mirror `src-tauri/src/settings.rs`.
 */
import {
  AI_BULK_MAX_BYTES_MAX,
  AI_BULK_MAX_BYTES_MIN,
  AI_DOCK_HEIGHT_MAX,
  AI_DOCK_HEIGHT_MIN,
  AI_HARD_CAP_MAX,
  AI_HARD_CAP_MIN,
  AI_IDLE_TIMEOUT_MAX,
  AI_IDLE_TIMEOUT_MIN,
  AI_MAX_BUDGET_USD_MAX,
  AI_MAX_TURNS_MAX,
  AI_MAX_TURNS_MIN,
} from '../../settings/ranges';
import type { AiConflictTools, UiSettings } from '../types';

/** Just the P68 slice of `UiSettings`, so the helpers below stay total. */
export type AiRunSettings = Pick<
  UiSettings,
  | 'aiIdleTimeoutSecs'
  | 'aiHardCapSecs'
  | 'aiMaxTurns'
  | 'aiStreamLog'
  | 'aiIncludePartialMessages'
  | 'aiConflictTools'
  | 'aiBulkMaxBytes'
  | 'aiMaxBudgetUsd'
  | 'aiDockHeight'
  | 'aiDockCollapsed'
>;

/** Mirrors the Rust field defaults (`settings::Settings`, §8.3). The two LOCKED
 *  user decisions are here: `aiHardCapSecs: 0` = unbounded (Cancel is the stop
 *  mechanism) and `aiMaxBudgetUsd: 0` = the `--max-budget-usd` flag is omitted. */
export const DEFAULT_AI_RUN_SETTINGS: AiRunSettings = {
  aiIdleTimeoutSecs: 300,
  aiHardCapSecs: 0,
  aiMaxTurns: 6,
  aiStreamLog: true,
  aiIncludePartialMessages: false,
  aiConflictTools: 'readOnly',
  aiBulkMaxBytes: 400_000,
  aiMaxBudgetUsd: 0,
  aiDockHeight: 180,
  aiDockCollapsed: false,
};

/** An integer knob (Rust `u32`): whole number, then clamped. */
function clampInt(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.round(value)));
}

/** Mirrors Rust `clamp_ai_settings` (settings.rs). `0` is a SENTINEL for the idle
 *  timeout (watchdog off), the hard cap (unbounded) and the budget (no flag), so
 *  those three are "0 or in range" — a plain clamp would silently turn a
 *  deliberate 0 into a minimum. Pure. */
export function clampAiRunSettings(a: AiRunSettings): AiRunSettings {
  const budget =
    !Number.isFinite(a.aiMaxBudgetUsd) || a.aiMaxBudgetUsd < 0
      ? 0
      : Math.min(AI_MAX_BUDGET_USD_MAX, a.aiMaxBudgetUsd);
  return {
    aiIdleTimeoutSecs:
      a.aiIdleTimeoutSecs === 0
        ? 0
        : clampInt(a.aiIdleTimeoutSecs, AI_IDLE_TIMEOUT_MIN, AI_IDLE_TIMEOUT_MAX),
    aiHardCapSecs:
      a.aiHardCapSecs === 0 ? 0 : clampInt(a.aiHardCapSecs, AI_HARD_CAP_MIN, AI_HARD_CAP_MAX),
    aiMaxTurns: clampInt(a.aiMaxTurns, AI_MAX_TURNS_MIN, AI_MAX_TURNS_MAX),
    aiStreamLog: a.aiStreamLog,
    aiIncludePartialMessages: a.aiIncludePartialMessages,
    aiConflictTools: a.aiConflictTools,
    aiBulkMaxBytes: clampInt(a.aiBulkMaxBytes, AI_BULK_MAX_BYTES_MIN, AI_BULK_MAX_BYTES_MAX),
    aiMaxBudgetUsd: budget,
    aiDockHeight: clampInt(a.aiDockHeight, AI_DOCK_HEIGHT_MIN, AI_DOCK_HEIGHT_MAX),
    aiDockCollapsed: a.aiDockCollapsed,
  };
}

function num(raw: unknown, fallback: number): number {
  return typeof raw === 'number' && Number.isFinite(raw) ? raw : fallback;
}

function bool(raw: unknown, fallback: boolean): boolean {
  return typeof raw === 'boolean' ? raw : fallback;
}

/** Tolerant per-field parse (mirrors the Rust per-field `#[serde(default)]` plus
 *  `clamp_ai_settings`): a pre-P68 blob — or one where a single key is corrupt —
 *  falls back to that field's default and never throws. `aiConflictTools` is
 *  validated against the two-variant union: anything else ⇒ `readOnly`, and there
 *  is deliberately no write option to accept (D10). Pure. */
export function parseAiRunSettings(parsed: Partial<UiSettings>): AiRunSettings {
  const d = DEFAULT_AI_RUN_SETTINGS;
  const tools: AiConflictTools = parsed.aiConflictTools === 'none' ? 'none' : 'readOnly';
  return clampAiRunSettings({
    aiIdleTimeoutSecs: num(parsed.aiIdleTimeoutSecs, d.aiIdleTimeoutSecs),
    aiHardCapSecs: num(parsed.aiHardCapSecs, d.aiHardCapSecs),
    aiMaxTurns: num(parsed.aiMaxTurns, d.aiMaxTurns),
    aiStreamLog: bool(parsed.aiStreamLog, d.aiStreamLog),
    aiIncludePartialMessages: bool(parsed.aiIncludePartialMessages, d.aiIncludePartialMessages),
    aiConflictTools: tools,
    aiBulkMaxBytes: num(parsed.aiBulkMaxBytes, d.aiBulkMaxBytes),
    aiMaxBudgetUsd: num(parsed.aiMaxBudgetUsd, d.aiMaxBudgetUsd),
    aiDockHeight: num(parsed.aiDockHeight, d.aiDockHeight),
    aiDockCollapsed: bool(parsed.aiDockCollapsed, d.aiDockCollapsed),
  });
}
