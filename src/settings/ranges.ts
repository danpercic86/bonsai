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

export const DOT_RADIUS_MIN = 2;
export const DOT_RADIUS_MAX = 10;

export const AVATAR_RADIUS_MIN = 6;
export const AVATAR_RADIUS_MAX = 16;

export const ROW_HEIGHT_MIN = 24;
export const ROW_HEIGHT_MAX = 48;

export const LANE_WIDTH_MIN = 10;
export const LANE_WIDTH_MAX = 28;
