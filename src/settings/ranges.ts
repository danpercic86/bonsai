/**
 * Shared clamp ranges for the P11 settings knobs (contract §2.3).
 *
 * These mirror the Rust range consts in `src-tauri/src/settings.rs` exactly.
 * Both the mock IPC layer (`src/ipc/mock.ts`) and the SettingsPanel UI import
 * from here so the min/max numbers live in one place on the frontend.
 */

export const AUTO_FETCH_INTERVAL_MIN = 1;
export const AUTO_FETCH_INTERVAL_MAX = 120;

// P30: healthRefresh background job (settings.rs HEALTH_REFRESH_INTERVAL_*).
export const HEALTH_REFRESH_INTERVAL_MIN = 1;
export const HEALTH_REFRESH_INTERVAL_MAX = 240;

// P51 D7: DOT_RADIUS_MIN/MAX removed — `dotRadius` is a deleted dead field.

export const AVATAR_RADIUS_MIN = 6;
export const AVATAR_RADIUS_MAX = 16;

export const ROW_HEIGHT_MIN = 24;
export const ROW_HEIGHT_MAX = 48;

export const LANE_WIDTH_MIN = 10;
export const LANE_WIDTH_MAX = 28;

// P68 §8.3: streaming AI-run knobs (settings.rs AI_*). `0` is a documented
// SENTINEL for the idle timeout (watchdog disabled), the hard cap (unbounded) and
// the budget (flag omitted) — so these are "0 OR in range" clamps, never a plain
// clamp that would turn an intentional 0 into a minimum.
export const AI_IDLE_TIMEOUT_MIN = 30;
export const AI_IDLE_TIMEOUT_MAX = 3600;
export const AI_HARD_CAP_MIN = 60;
export const AI_HARD_CAP_MAX = 86_400;
export const AI_MAX_TURNS_MIN = 1;
export const AI_MAX_TURNS_MAX = 20;
export const AI_BULK_MAX_BYTES_MIN = 20_000;
export const AI_BULK_MAX_BYTES_MAX = 4_000_000;
export const AI_MAX_BUDGET_USD_MAX = 100;
export const AI_DOCK_HEIGHT_MIN = 120;
export const AI_DOCK_HEIGHT_MAX = 600;

/**
 * P68 OQ1: how many streaming AI runs may be live at once.
 *
 * MIRRORS `bonsai_core::ai::AI_MAX_CONCURRENT_RUNS`, which is the authoritative
 * one: the backend rejects an over-cap `aiResolveConflictStream` with an
 * `aiFailed` whose message starts "too many AI runs in progress". This copy exists
 * only so the UI can disable an entry point before making a call it knows will
 * fail — never as the sole guard (each run is a `claude` process tree with no
 * wall-clock deadline and no default spend cap). Keep the two numbers equal.
 */
export const AI_MAX_CONCURRENT_RUNS = 3;
