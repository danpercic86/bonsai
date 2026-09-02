/**
 * P91 Amendment A26 (`docs/contracts/P91-raw-args-privacy.md`) — raw-mode `args`
 * is an **allow-list, not a blanket include**.
 *
 * The one rule: `raw` mode widens **identifier** fidelity — repo path, file
 * paths, ref names, remote URLs, full SHAs — and **never** content fidelity.
 * Free text (commit messages, search strings, prompts) and credentials are
 * outside *both* modes, permanently.
 *
 * The table itself lives in `rawArgPolicy.json` so it is data, diffable and
 * reviewable on its own. **An unlisted command is DENY** — it behaves exactly
 * like strict mode (`argsHash` + `argsShape`, no values), so a command added
 * tomorrow cannot leak by omission.
 *
 * The writer (`src-tauri/src/obs/raw_args.rs`) enforces an *independent*
 * shape+vocabulary invariant and deliberately does NOT read this table: a shared
 * table cannot defend against a wrong row or against producer code that ignores
 * it, which is precisely the bug this amendment fixes.
 */
import type { IpcApi } from '../ipc/types';
import rawArgPolicyJson from './rawArgPolicy.json';

/** §B.3 — a raw `args` string longer than this is elided (also enforced by the writer). */
export const RAW_ARG_MAX_STR = 512;

/**
 * Compile-time forcing function (AC5's static half): every key of the JSON must
 * be a real `IpcApi` method. A key that is not resolves to `never` here, and the
 * JSON's array value is not assignable to `never` — so a typo or a renamed
 * command breaks the build instead of silently becoming dead policy.
 */
type PolicyTable = {
  [K in keyof typeof rawArgPolicyJson]: K extends keyof IpcApi
    ? readonly (string | null)[]
    : never;
};

/** The per-command positional allow-list. Entry `i` names positional argument `i`. */
export const RAW_ARG_POLICY: PolicyTable = rawArgPolicyJson;

/** Untyped view for `cmd`-keyed lookup (the proxy only ever has a `string`). */
const POLICY_LOOKUP: Readonly<Record<string, readonly (string | null)[] | undefined>> =
  RAW_ARG_POLICY;

/**
 * Credential vocabulary. Mirrors Rust `scrub.rs::is_sensitive_key` plus the
 * explicit extras of §B.3.
 */
const SENSITIVE_PARAM =
  /token|secret|password|passphrase|auth|credential|apikey|api_key|privatekey|sshkey|\bpat\b/i;

/**
 * Free-text vocabulary — applied to raw `args` keys **only**. It must never be
 * folded into the general scrubber: `ErrorPayload.message` is a legitimate,
 * already-scrubbed field and collapsing it would blind every error record.
 */
const FREE_TEXT_PARAM =
  /message|msg|query|search|text|body|prompt|descri|note|content|comment|title|subject|summary|patch|diff|blurb|input|reason/i;

/** True when the parameter name is credential-shaped and may never be recorded. */
export function isSensitiveParam(name: string): boolean {
  return SENSITIVE_PARAM.test(name);
}

/** True when the parameter name suggests user-authored prose. */
export function isFreeTextParam(name: string): boolean {
  return FREE_TEXT_PARAM.test(name);
}

export interface RawArgsResult {
  /** Name-keyed, allow-listed scalars. Omitted entirely when nothing survived. */
  args?: Record<string, unknown>;
  /** Count of positions the policy or the value checks removed. */
  omitted: number;
}

/** §B.4 — objects, arrays and functions are categorically ineligible. */
function isAllowedScalar(v: unknown): boolean {
  if (v === null) return true;
  const t = typeof v;
  if (t === 'boolean') return true;
  if (t === 'number') return Number.isFinite(v as number);
  if (t !== 'string') return false;
  const s = v as string;
  return s.length <= RAW_ARG_MAX_STR && !s.includes('\n') && !s.includes('\r');
}

/**
 * Builds the raw-mode `args` object for one call. Default deny: an unlisted
 * command yields no `args` at all.
 */
export function buildRawArgs(cmd: string, args: readonly unknown[]): RawArgsResult {
  const row = POLICY_LOOKUP[cmd];
  if (!row) return { omitted: args.length };
  const out: Record<string, unknown> = {};
  let kept = 0;
  for (let i = 0; i < args.length; i += 1) {
    const name = row[i];
    if (typeof name !== 'string') continue;
    if (isFreeTextParam(name) || isSensitiveParam(name)) continue;
    const value = args[i];
    if (!isAllowedScalar(value)) continue;
    out[name] = value;
    kept += 1;
  }
  const omitted = args.length - kept;
  return kept > 0 ? { args: out, omitted } : { omitted };
}
