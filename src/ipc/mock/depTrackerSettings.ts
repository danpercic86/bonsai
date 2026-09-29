/**
 * P125 §2.2 — the `depTracker` settings slice in the mock IPC layer: tolerant
 * parsing and the clamp mirror.
 *
 * Its own module (the `aiRunSettings.ts` precedent) rather than an addition to
 * `persistence.ts`, which is at the file-size limit. `readUiSettings` calls
 * {@link sanitizeDepTrackerSettings} (a hand-edited or pre-P125 localStorage blob
 * reads back as OFF / 12 h, mirroring the Rust `#[serde(default)]` struct), and
 * `setUiSettings` calls {@link clampDepTracker} on a whole-struct patch
 * (mirroring `apply_patch` → `clamp_dep_tracker`).
 */
import { DEFAULT_UI_SETTINGS } from '../../settings/defaults';
import { DEP_TRACKER_INTERVAL_MAX, DEP_TRACKER_INTERVAL_MIN } from '../../settings/ranges';
import type { DepTrackerSettings } from '../types/depTracker';

/** Mirrors Rust `clamp_dep_tracker`: `intervalHours ∈ 1..=168`, whole hours. */
export function clampDepTracker(d: DepTrackerSettings): DepTrackerSettings {
  const hours = Number.isFinite(d.intervalHours)
    ? Math.round(d.intervalHours)
    : DEFAULT_UI_SETTINGS.depTracker.intervalHours;
  return {
    enabled: d.enabled,
    intervalHours: Math.min(DEP_TRACKER_INTERVAL_MAX, Math.max(DEP_TRACKER_INTERVAL_MIN, hours)),
  };
}

/** Per-field tolerant parse of a persisted `depTracker` object. Never throws and
 *  never mints a partial struct: each field falls back to the production default
 *  independently, then the interval is clamped. Pure. */
export function sanitizeDepTrackerSettings(raw: unknown): DepTrackerSettings {
  const d = (typeof raw === 'object' && raw !== null ? raw : {}) as Partial<DepTrackerSettings>;
  const fallback = DEFAULT_UI_SETTINGS.depTracker;
  return clampDepTracker({
    enabled: typeof d.enabled === 'boolean' ? d.enabled : fallback.enabled,
    intervalHours: typeof d.intervalHours === 'number' ? d.intervalHours : fallback.intervalHours,
  });
}
