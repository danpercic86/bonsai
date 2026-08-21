/** Write-target level (P40). System is never a write target. */
export type ConfigLevelArg = 'local' | 'global';
/** Where a value actually lives (read result). */
export type ConfigLevelName = 'local' | 'global' | 'system' | 'other';
export type ConfigValueKind = 'text' | 'bool' | 'enum';

/** A curated key with effective value + the value set at the target level (P40
 *  §4.2). Mirrors the Rust `CuratedEntry` (camelCase). `targetValue == null` +
 *  `effectiveValue != null` => the value is inherited from `effectiveLevel`. */
export interface CuratedConfigEntry {
  key: string;
  kind: ConfigValueKind;
  enumValues: string[];
  effectiveValue: string | null;
  effectiveLevel: ConfigLevelName | null;
  targetValue: string | null;
}

/** An arbitrary section.key entry at the target level (P40 Advanced list). */
export interface ConfigEntry {
  name: string;
  value: string;
  level: ConfigLevelName;
}

/** Result of getConfig for one target level (P40 §4.2). */
export interface ConfigView {
  targetLevel: ConfigLevelArg;
  curated: CuratedConfigEntry[];
  advanced: ConfigEntry[];
}

/** P82: curated identity-profile color palette. Mirrors Rust `ProfileColor`
 *  (serde camelCase). Maps to a theme-aware CSS token (see P82-ui.md). */
export type ProfileColor =
  | 'neutral'
  | 'slate'
  | 'blue'
  | 'teal'
  | 'green'
  | 'amber'
  | 'orange'
  | 'purple'
  | 'pink';

/** One named identity profile (P44). `id` is a stable crypto.randomUUID(). */
export interface IdentityProfile {
  id: string;
  label: string;
  userName: string;
  userEmail: string;
  /** Optional user.signingkey; null/empty ⇒ not written on apply. */
  signingKey: string | null;
  /** P82: display color. Optional on the wire (absent ⇒ 'neutral' for pre-P82
   *  persisted profiles); readers treat undefined as neutral. */
  color?: ProfileColor;
}
