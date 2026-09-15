/**
 * P112 §8 / §16.10 — every string the external-tools group renders, and the two
 * text builders that interpolate one.
 *
 * Data only, so the section and the row stay rendering code: the copy is signed
 * in the contract and this is the one place it lives, which is also what lets a
 * test pin it without mounting anything. Sentence case, one space after a
 * period, curly apostrophes, and a quoted control name uses `“ ”` (§12.12).
 *
 * The ids also live here because two id VOCABULARIES meet on this surface and
 * mixing them is a silent a11y failure:
 *   * `slot` is the DOTTED row id (`general.terminal-tool`) — what
 *     `useOutcomeNotes` keys on and what `[data-outcome-note]` carries;
 *   * `id` is the DASHED element id (`general-terminal-tool-note`) — what
 *     `aria-describedby` points at.
 * The note ids are hand-written rather than `settingsRowHelpId(rowId)`, which
 * would DANGLE: all three rows deliberately carry no catalog `help` (§8), so
 * `{rowId}-help` names an element that is never rendered, and a dangling idref
 * is worse than no description (ui-reference §12.2 rule 5). This is
 * `SettingsDevLogsSection.tsx:30-33`'s shape.
 */
import type { ExternalToolKind } from '../../ipc';

export const TERMINAL_SLOT = 'general.terminal-tool';
export const EDITOR_SLOT = 'general.editor-tool';
export const RESCAN_SLOT = 'general.rescan-tools';

export const TERMINAL_NOTE_ID = 'general-terminal-tool-note';
export const TERMINAL_OUTCOME_ID = 'general-terminal-tool-outcome';
export const EDITOR_NOTE_ID = 'general-editor-tool-note';
export const EDITOR_OUTCOME_ID = 'general-editor-tool-outcome';
export const RESCAN_NOTE_ID = 'general-rescan-tools-note';
export const RESCAN_OUTCOME_ID = 'general-rescan-tools-outcome';

/** §16.4: state note FIRST, then the outcome slot — both ids always present,
 *  because both elements are always mounted (the outcome note is empty when
 *  idle). Composed once per control, never assembled at a call site. */
export const RESCAN_DESCRIBED_BY = `${RESCAN_NOTE_ID} ${RESCAN_OUTCOME_ID}`;

/** The group's own two strings. `LEAD` keeps the one fact from the deleted
 *  section note that is still true and still matters: never through a shell. */
export const LEAD =
  '“Open in terminal” and “Open in editor” use the tools below. Bonsai offers only what it finds on this computer and launches it directly, never through a shell.';
export const GROUP_NOTE = 'Not listed? Use Browse to point Bonsai at the program yourself.';

/** §5 state 7a — shown in the input only while NO scan has landed yet, because
 *  a strict `Combobox` derives its text from the options and a stored id whose
 *  label has not arrived renders as `''`, i.e. reads as UNSET. Never a synthetic
 *  "loading…" OPTION: that would be selectable and would patch a junk value. */
export const PLACEHOLDER = 'Looking for installed tools…';

export const OPT_AUTO_LABEL = 'Auto-detect';
export const OPT_AUTO_DETAIL = 'Bonsai picks the first tool it finds';
/** `detail === 'built in'` is the architect's frozen sentinel; sentence case is
 *  applied here rather than asking for a DTO change (§4.2). */
export const OPT_BUILTIN_DETAIL = 'Built in';
/** A WORD, not a hue — so "kept but not installed" survives greyscale and needs
 *  no badge (WCAG 1.4.1), and lands in the option's accessible name. */
export const OPT_MISSING_DETAIL = 'Not installed';

export const BTN_BROWSE = 'Browse…';
export const BTN_RESCAN = 'Rescan';

export const SCAN_SCANNING = 'Looking for installed tools…';
export const SCAN_NONE = 'No terminals or editors found on this computer.';
/** §16.4 R5 — the state §5 had no entry for: `listExternalTools` rejects
 *  `AppError('other')`. What happened / what it means / what to do next, with no
 *  raw backend text (the category is `other`; nothing in it is actionable). */
export const SCAN_ERR =
  'Couldn’t check for installed tools. Your current choices still work — try Rescan again.';
/** §16.4a — the pick was PERSISTED and only the re-read that would display it
 *  failed, so silence would leave a wrong value on screen with no indication. */
export const BROWSE_STALE =
  'Your choice was saved, but Bonsai couldn’t refresh this list. Press Rescan.';
export const BROWSE_ERR =
  'That file isn’t a program Bonsai can launch. Try again, or pick a detected tool.';

/** The per-kind nouns every row string interpolates. One table, so the two rows
 *  cannot drift into saying "terminal" in an editor sentence. */
interface KindWords {
  /** The action the setting configures, quoted as the user sees it in a menu. */
  action: string;
  /** Singular noun for a tool of this kind. */
  noun: string;
  /** Plural noun. */
  plural: string;
  /** Row label, used by the announce-only confirmation. */
  label: string;
}

const WORDS: Record<ExternalToolKind, KindWords> = {
  terminal: {
    action: '“Open in terminal”',
    noun: 'terminal',
    plural: 'terminals',
    label: 'Terminal',
  },
  editor: { action: '“Open in editor”', noun: 'editor', plural: 'editors', label: 'Editor' },
};

export function noteAuto(kind: ExternalToolKind): string {
  const w = WORDS[kind];
  return `${w.action} uses the first ${w.noun} Bonsai finds.`;
}

export function noteNone(kind: ExternalToolKind): string {
  const w = WORDS[kind];
  return `No ${w.plural} found on this computer. ${w.action} still tries the usual ones for this system.`;
}

export function noteBuiltIn(label: string): string {
  return `${label} is built in to this system.`;
}

/** §8: says all three things in order — what is wrong, what happens instead, and
 *  that nothing was lost. `your choice is kept` is the clause that makes the
 *  silent fallback honest rather than mysterious. */
export function noteStale(kind: ExternalToolKind, label: string): string {
  const fallback = `${WORDS[kind].action} falls back to auto-detect; your choice is kept.`;
  return `${label} isn’t installed right now. ${fallback}`;
}

/** The trailing half of `NOTE_STALE_CUSTOM`; its head is a `<span class="mono">`
 *  holding the path, so the sentence is assembled in JSX, not here. */
export function noteStaleCustomTail(kind: ExternalToolKind): string {
  const fallback = `${WORDS[kind].action} falls back to auto-detect; your choice is kept.`;
  return ` any more. ${fallback}`;
}

export function noteScanning(kind: ExternalToolKind): string {
  return `Looking for installed ${WORDS[kind].plural}…`;
}

export function browseLabel(kind: ExternalToolKind): string {
  return `Browse for ${kind === 'terminal' ? 'a terminal' : 'an editor'} program`;
}

/** §16.10 — announce-only, never rendered. It names the LABEL, never the path: a
 *  46-char absolute path read aloud is the "not actionable by voice" case. */
export function announceBrowsed(kind: ExternalToolKind, label: string): string {
  return `${WORDS[kind].label} set to ${label}.`;
}

/** The Rescan row's counts sentence, and the string a landed rescan announces.
 *  An empty scan is `SCAN_NONE` — not `0 terminals and 0 editors found.` */
export function scanCounts(terminals: number, editors: number): string {
  if (terminals === 0 && editors === 0) return SCAN_NONE;
  const t = `${terminals} ${terminals === 1 ? 'terminal' : 'terminals'}`;
  const e = `${editors} ${editors === 1 ? 'editor' : 'editors'}`;
  return `${t} and ${e} found.`;
}
