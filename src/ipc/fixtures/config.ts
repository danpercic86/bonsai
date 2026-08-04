// P40 mock git-config store + view builder (contract §6.3).
//
// A per-repo, per-level store keyed by dotted name. The Global level seeds a
// WORKING identity so the harness default commits succeed; `?fixture=noconfig`
// (handled in mock.ts) starts with identity dropped so the commit-error /
// Set-identity flow is demoable. `buildConfigView` mirrors the Rust
// `read_config`: effective = local overrides global (system omitted here),
// curated from the same CURATED_KEYS list, advanced = target-level non-curated.

import type {
  AppError,
  ConfigEntry,
  ConfigLevelArg,
  ConfigLevelName,
  ConfigValueKind,
  ConfigView,
  CuratedConfigEntry,
} from '../types';

/** Curated key definition mirroring the Rust `CURATED_KEYS` (contract §4.1). */
interface CuratedKeyDef {
  key: string;
  kind: ConfigValueKind;
  enumValues: string[];
}

export const CURATED_KEYS: CuratedKeyDef[] = [
  { key: 'user.name', kind: 'text', enumValues: [] },
  { key: 'user.email', kind: 'text', enumValues: [] },
  { key: 'core.autocrlf', kind: 'enum', enumValues: ['true', 'false', 'input'] },
  { key: 'init.defaultBranch', kind: 'text', enumValues: [] },
  { key: 'pull.ff', kind: 'enum', enumValues: ['true', 'false', 'only'] },
  { key: 'pull.rebase', kind: 'enum', enumValues: ['true', 'false', 'merges', 'interactive'] },
];

/** Two flat maps keyed by dotted name -> value, one per level. */
export interface MockConfigStore {
  local: Record<string, string>;
  global: Record<string, string>;
}

/**
 * Fresh store. `withIdentity` false (the `?fixture=noconfig` case) drops the
 * seeded global identity so a mock commit that consults the store fails with
 * `configMissing` until the user sets an identity in Settings.
 */
export function makeMockConfigStore(withIdentity = true): MockConfigStore {
  const global: Record<string, string> = { 'init.defaultBranch': 'main' };
  if (withIdentity) {
    global['user.name'] = 'Mock Fixture User';
    global['user.email'] = 'fixture@bonsai.dev';
  }
  return { local: {}, global };
}

function isCurated(name: string): boolean {
  return CURATED_KEYS.some((c) => c.key.toLowerCase() === name.toLowerCase());
}

/** Effective value for `key`: local wins over global (system omitted). */
function effective(store: MockConfigStore, key: string): { value: string; level: ConfigLevelName } | null {
  if (key in store.local) return { value: store.local[key], level: 'local' };
  if (key in store.global) return { value: store.global[key], level: 'global' };
  return null;
}

/** Whether the store has a usable (non-empty) identity for commit gating. */
export function hasIdentity(store: MockConfigStore): boolean {
  const name = effective(store, 'user.name');
  const email = effective(store, 'user.email');
  return Boolean(name && name.value.trim() && email && email.value.trim());
}

/** Builds a `ConfigView` for one target level from the store (mirrors §4.4). */
export function buildConfigView(store: MockConfigStore, level: ConfigLevelArg): ConfigView {
  const target = store[level];
  const curated: CuratedConfigEntry[] = CURATED_KEYS.map((ck) => {
    const eff = effective(store, ck.key);
    const targetValue = ck.key in target ? target[ck.key] : null;
    return {
      key: ck.key,
      kind: ck.kind,
      enumValues: [...ck.enumValues],
      effectiveValue: eff ? eff.value : null,
      effectiveLevel: eff ? eff.level : null,
      targetValue,
    };
  });

  const advanced: ConfigEntry[] = Object.keys(target)
    .filter((name) => !isCurated(name))
    .sort()
    .map((name) => ({ name, value: target[name], level: level as ConfigLevelName }));

  return { targetLevel: level, curated, advanced };
}

/** Throws an `AppError`-shaped `invalidName` mirroring the Rust §4.5 key shape. */
export function validateKeyOrThrow(key: string): void {
  const trimmed = key.trim();
  const fail = (message: string): never => {
    const err: AppError = { kind: 'invalidName', message };
    throw err;
  };
  if (trimmed === '') fail('config key must not be empty');
  const parts = trimmed.split('.');
  if (parts.length < 2) fail('config key must be section.key');
  const section = parts[0];
  if (section === '' || !/^[A-Za-z0-9-]+$/.test(section)) fail('invalid section name');
  const variable = parts[parts.length - 1];
  if (!/^[A-Za-z][A-Za-z0-9-]*$/.test(variable)) fail('invalid key name');
  for (const sub of parts.slice(1, parts.length - 1)) {
    if (sub === '' || /\s/.test(sub)) fail('invalid subsection');
  }
}

/** Throws `invalidName` if `value` is out of a curated Enum key's set (§4.5). */
export function validateEnumOrThrow(key: string, value: string): void {
  const ck = CURATED_KEYS.find((c) => c.key.toLowerCase() === key.trim().toLowerCase());
  if (ck && ck.kind === 'enum' && !ck.enumValues.includes(value.trim())) {
    const err: AppError = {
      kind: 'invalidName',
      message: `value for ${ck.key} must be one of: ${ck.enumValues.join(', ')}`,
    };
    throw err;
  }
}
