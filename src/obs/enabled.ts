/**
 * P91 §10/§11 — the single gate every obs code path reads first.
 *
 * ONE concern: "is Dev mode on, and at what verbosity". It performs no IO, holds
 * no records and imports nothing from `src/ipc` (the IPC proxy imports *this*,
 * so the dependency must not run the other way).
 *
 * Dev mode OFF must cost exactly one boolean read: no allocation, no Proxy
 * wrapper, no trace minting (§11). That is why the config is a module-level
 * value rather than React state — the IPC `get` trap runs outside React.
 */
import type { DevSettings, LogLevel } from '../ipc/types/settings';

/** Verbosity ordering: a record is captured when its level is at or above the
 *  configured threshold. `error` is the most severe, `trace` the least. */
const LEVEL_RANK: Record<LogLevel, number> = {
  error: 0,
  warn: 1,
  info: 2,
  debug: 3,
  trace: 4,
};

export type ObsConfigListener = (dev: DevSettings | null, sessionRestarted: boolean) => void;

let config: DevSettings | null = null;
const listeners = new Set<ObsConfigListener>();

/**
 * Installs (or clears) the Dev-mode config. Called at boot and on every settings
 * change; increments 4/7 own the wiring — nothing in production calls this yet.
 *
 * §12 row 2 note: changing `level` or `includeRawNames` **restarts the Rust log
 * session**, which mints a NEW salt. Listeners are told via `sessionRestarted`
 * so the redactor can re-seed instead of hashing against a stale salt.
 */
export function configureObs(dev: DevSettings | null): void {
  const prev = config;
  config = dev && dev.enabled ? { ...dev } : null;
  const restarted =
    !!config &&
    (!prev || prev.level !== config.level || prev.includeRawNames !== config.includeRawNames);
  for (const l of listeners) l(config, restarted);
}

/** Subscribe to config changes. Returns the unsubscribe. */
export function onObsConfigChange(listener: ObsConfigListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** The hot check — one boolean read when off. */
export function obsEnabled(): boolean {
  return config !== null;
}

export function obsLevel(): LogLevel {
  return config?.level ?? 'debug';
}

/** Is `lvl` verbose enough to be captured at the current threshold? */
export function obsLevelAllows(lvl: LogLevel): boolean {
  return config !== null && LEVEL_RANK[lvl] <= LEVEL_RANK[config.level];
}

/** §10 `dev.capture-ipc` — gates ipc/event/channel (and later `span`) records. */
export function obsCaptureIpc(): boolean {
  return config !== null && config.captureIpc;
}

/** §10 `dev.capture-react` — gates render/effect/state records (increment 4). */
export function obsCaptureReact(): boolean {
  return config !== null && config.captureReact;
}

/** §10 `dev.capture-frames`; `level: 'trace'` force-enables it (§5 footnote). */
export function obsCaptureFrames(): boolean {
  return config !== null && (config.captureFrames || config.level === 'trace');
}

/** §7 — `raw` is the only mode in which argument VALUES may be logged. */
export function obsRedaction(): 'strict' | 'raw' {
  return config?.includeRawNames ? 'raw' : 'strict';
}

/** TEST ONLY — drops the config and every listener. */
export function resetObsConfigForTests(): void {
  config = null;
  listeners.clear();
}
