/**
 * P119 §5.1 — the mock's mirrors of the core target funnel (`TargetArg` +
 * `resolve_activity_target`) and of the line funnel (`activity_line`).
 *
 * The mock is a second backend: every string it puts on the activity stream
 * crosses the same boundary as the real one, so it goes through the same rules.
 * Every helper here is pure and NEVER throws — the real resolver returns `None`
 * when the repo cannot be opened and the bracket still runs, so an unknown repo
 * id yields `null` here, never a `noRepo` rejection before `started`.
 */
import { repos } from './repoState';

/** MIRRORS `bonsai_core::git::activity::MAX_ACTIVITY_LINE_CHARS`. */
export const MAX_ACTIVITY_LINE_CHARS = 2000;

/** MIRRORS `strip_control_chars`: drops C0/C1 controls (so `\n`, `\r`, `\t`,
 *  U+007F), the bidi overrides/isolates (U+200E/200F, U+202A-202E,
 *  U+2066-2069) and the zero-width chars (U+200B-200D, U+FEFF). A code-point
 *  filter, not a regex: that is the exact shape of the Rust, and a control-char
 *  class in a regex literal is a lint error. `[...s]` iterates code points,
 *  matching Rust's `chars()`. */
export function stripActivityControls(raw: string): string {
  return [...raw]
    .filter((ch) => {
      const cp = ch.codePointAt(0) ?? 0;
      if (cp <= 0x1f || (cp >= 0x7f && cp <= 0x9f)) return false; // C0 / C1
      if (cp >= 0x200b && cp <= 0x200f) return false; // ZWSP/ZWNJ/ZWJ + LRM/RLM
      if (cp >= 0x202a && cp <= 0x202e) return false; // bidi embeddings/overrides
      if (cp >= 0x2066 && cp <= 0x2069) return false; // bidi isolates
      return cp !== 0xfeff; // BOM
    })
    .join('');
}

/** MIRRORS `truncate_chars`: an over-long string ends in `…` and is exactly
 *  `cap` CHARS long. */
export function truncateActivityChars(text: string, cap: number): string {
  if (cap <= 0) return '';
  const chars = [...text];
  if (chars.length <= cap) return text;
  return `${chars.slice(0, cap - 1).join('')}…`;
}

/** MIRRORS `activity_line`: every stdout/stderr line on the wire. */
export function mockActivityLine(raw: string): string {
  return truncateActivityChars(stripActivityControls(raw), MAX_ACTIVITY_LINE_CHARS);
}

/** MIRRORS the §2.8 reason line in `ActivityEmitter::finish`: line breaks become
 *  spaces FIRST (the control strip would otherwise glue the words together),
 *  then the ordinary line funnel (other controls stripped, 2000-char cap). */
export function mockReasonLine(message: string): string {
  return mockActivityLine(message.replace(/\r\n/g, ' ').replace(/[\r\n]/g, ' '));
}

// ------------------------------------------------ TargetArg mirrors (§2.2/§2.3)

/** `ActivityTarget::commit`: the first 7 chars iff the input is ≥7 ASCII hex
 *  digits (a real oid), else null — `HEAD~2` / `abc` are never a target. */
export function mockCommitTarget(oid: string): string | null {
  return oid.length >= 7 && /^[0-9a-f]+$/i.test(oid) ? oid.slice(0, 7) : null;
}

/** `ActivityTarget::stash`: `stash@{N}`. */
export function mockStashTarget(index: number): string {
  return `stash@{${index}}`;
}

/** `TargetArg::PathLeaf` (`path_leaf`): the last path component under either
 *  separator, trailing separators ignored; empty → null.
 *
 *  SECURITY mirror: URL-shaped input is REFUSED, not trimmed — null for any
 *  `://` anywhere, or an `@` in the leaf (scp-like `git@host`,
 *  `/a/user:tok@host`). A clone URL can carry `user:token@host`, and the "leaf"
 *  of `https://user:tok@host` is exactly the credential. Callers pass a
 *  filesystem path (clone/init: the destination). */
export function mockPathLeaf(path: string): string | null {
  if (path.includes('://')) return null;
  const leaf = path.replace(/[/\\]+$/, '').split(/[/\\]/).pop() ?? '';
  return leaf === '' || leaf.includes('@') ? null : leaf;
}

/** `ActivityTarget::any_ref`: strips ONE of `refs/heads/`, `refs/remotes/`,
 *  `refs/tags/`. */
export function mockRefTarget(ref: string): string {
  for (const prefix of ['refs/heads/', 'refs/remotes/', 'refs/tags/']) {
    if (ref.startsWith(prefix)) return ref.slice(prefix.length);
  }
  return ref;
}

/** `ActivityTarget::tag`: strips `refs/tags/`. */
export function mockTagTarget(tag: string): string {
  return tag.startsWith('refs/tags/') ? tag.slice('refs/tags/'.length) : tag;
}

// ------------------------------------------- resolver mirrors (H / R / C rows)

/** `H` rows: the checked-out branch; null when detached, unborn, or the repo is
 *  not open (never throws). */
export function mockHeadTarget(repoId: string): string | null {
  const state = repos.get(repoId);
  if (state === undefined || state.kind === 'detached' || state.kind === 'unborn') return null;
  return state.headBranch === '' ? null : state.headBranch;
}

/** `R` rows: the branch of the in-progress rebase (`head_name`), else null. */
export function mockRebaseTarget(repoId: string): string | null {
  const op = repos.get(repoId)?.opState;
  return op?.kind === 'rebase' ? op.headName : null;
}

/** `C` rows: HEAD's short oid (the bisect midpoint being judged), else null. */
export function mockHeadCommitTarget(repoId: string): string | null {
  const state = repos.get(repoId);
  return state === undefined ? null : mockCommitTarget(state.headOid);
}
