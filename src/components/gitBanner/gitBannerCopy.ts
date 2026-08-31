// P70 UI §5: every string the notice bar shows, branched by OS family and by
// variant. Pure data + pure functions, zero DOM — so the copy is unit-tested
// without rendering anything, and the component stays a layout concern.
//
// NOTE the division of labour with the Rust message: `GitAvailability.detail`
// is a single-paragraph error payload written to be pasted into a bug report,
// and it appears ONLY inside the technical-details block. The banner's prose is
// the structured copy below.

import type { GitAvailability, GitBinSource } from '../../ipc';
import { osFamily, type OsFamily } from '../../utils/platform';

/** UI §11.3: the `?os=` override, mock-gated so it can never affect a real
 *  build. Deliberately lives HERE and not in `utils/platform.ts`, which stays
 *  free of harness code. */
export function resolveOsFamily(): OsFamily {
  if (import.meta.env.VITE_MOCK_IPC !== '1') return osFamily;
  try {
    const q = new URLSearchParams(window.location.search).get('os');
    if (q === 'windows' || q === 'mac' || q === 'linux') return q;
    return osFamily;
  } catch {
    return osFamily;
  }
}

/** UI §3: keyed only on fields the backend ships. `notFound` = the ladder was
 *  exhausted; `unrunnable` = a candidate resolved but could not be run. */
export type BannerVariant = 'notFound' | 'unrunnable';

export function bannerVariant(status: GitAvailability | null): BannerVariant {
  return status !== null && status.path !== null ? 'unrunnable' : 'notFound';
}

/** UI §5.5: how a rung is named to a human. */
export function sourceLabel(source: GitBinSource): string {
  switch (source) {
    case 'override':
      return 'BONSAI_GIT_BIN';
    case 'path':
      return 'PATH';
    case 'registry':
      return 'Windows registry';
    case 'wellKnown':
      return 'standard install folder';
    case 'fallback':
    default:
      // `default` (not an exhaustive-switch fallthrough) because the value
      // arrives over IPC: a future Rust rung must degrade to a truthful label
      // rather than render `undefined`.
      return 'not found';
  }
}

export const BANNER_TITLE: Record<BannerVariant, string> = {
  notFound: 'Git is not available',
  unrunnable: "Git couldn't be started",
};

export const BANNER_EXPLANATION: Record<BannerVariant, string> = {
  notFound:
    "Bonsai couldn't find a runnable git program on this computer. Your saved credentials are " +
    'fine — Bonsai never got as far as checking them.',
  unrunnable:
    "Bonsai found a git program but couldn't run it. Your saved credentials are fine — Bonsai " +
    'never got as far as checking them.',
};

const NOT_FOUND_REMEDY: Record<OsFamily, string> = {
  windows:
    'Quit Bonsai and reopen it from the Start menu — an in-app update can leave Bonsai running ' +
    'without your full PATH.',
  mac:
    'Quit Bonsai and reopen it from Applications — an in-app update can leave Bonsai running ' +
    'without your full PATH.',
  linux:
    'Quit Bonsai and reopen it from your application menu — an in-app update can leave Bonsai ' +
    'running with an incomplete PATH.',
};

/** UI §5.1 / §5.2: the ONE remedy shown in the collapsed bar. */
export function bannerRemedy(
  variant: BannerVariant,
  os: OsFamily,
  source: GitBinSource | null,
): string {
  if (variant === 'notFound') return NOT_FOUND_REMEDY[os];
  if (source === 'override') {
    return "BONSAI_GIT_BIN points at a program Bonsai can't run. Correct it or clear it, then restart Bonsai.";
  }
  return os === 'windows'
    ? 'Reinstall Git for Windows, then choose Re-check.'
    : 'Reinstall Git, then choose Re-check.';
}

const INSTALL_REMEDY: Record<OsFamily, string> = {
  windows: 'Install Git for Windows from git-scm.com, then choose Re-check.',
  mac: 'Install Git — run xcode-select --install, or brew install git — then choose Re-check.',
  linux:
    'Install Git with your package manager (for example, sudo apt install git), then choose Re-check.',
};

const ENV_REMEDY: Record<OsFamily, string> = {
  windows: 'Set BONSAI_GIT_BIN to the full path of git.exe, then restart Bonsai.',
  mac: 'Set BONSAI_GIT_BIN to the full path of the git binary, then restart Bonsai.',
  linux: 'Set BONSAI_GIT_BIN to the full path of the git binary, then restart Bonsai.',
};

/** UI §5.3. The BONSAI_GIT_BIN row is suppressed when it is already the
 *  headline remedy (Variant B with an `override` source). */
export function otherRemedies(
  variant: BannerVariant,
  os: OsFamily,
  source: GitBinSource | null,
): string[] {
  const rows = [INSTALL_REMEDY[os]];
  if (!(variant === 'unrunnable' && source === 'override')) rows.push(ENV_REMEDY[os]);
  return rows;
}

/** UI §5.4 — revised for ratified decision 5. The SSH clause is load-bearing:
 *  ssh-agent authentication runs entirely inside libgit2 and is unaffected, so
 *  claiming "fetch, pull and push don't work" would be false for a large class
 *  of users. */
export const CAPABILITY_ROWS = [
  {
    tone: 'works' as const,
    leader: 'Still works:',
    text:
      "the commit graph, file status, staging, committing, branches, tags and diffs — these don't " +
      'use the git program. Remotes you connect to over SSH also keep working.',
  },
  {
    tone: 'broken' as const,
    leader: "Doesn't work:",
    text:
      'commit search, commit signing, Git hooks, and signing in to HTTPS remotes — Bonsai needs ' +
      'Git to read the credential helper that holds those saved logins.',
  },
];

/** UI §5.7: the ONE resolved copy set for the current status. Everything the
 *  surface says — visible or announced — is read off this object, so the
 *  announcement structurally cannot diverge from the rendered text. */
export interface BannerCopy {
  variant: BannerVariant;
  title: string;
  explanation: string;
  /** The single headline remedy shown in the collapsed bar. */
  remedy: string;
  /** Variant B only: the candidate we resolved but could not run. Rendered as
   *  the `Tried:` row and DELIBERATELY never announced (§5.7 — a 250-char path
   *  read aloud is hostile; it stays in the technical block). */
  triedPath: string | null;
  source: GitBinSource | null;
  os: OsFamily;
}

export function bannerCopy(status: GitAvailability | null, os: OsFamily): BannerCopy {
  const variant = bannerVariant(status);
  const source = status?.source ?? null;
  return {
    variant,
    title: BANNER_TITLE[variant],
    explanation: BANNER_EXPLANATION[variant],
    remedy: bannerRemedy(variant, os, source),
    triedPath: variant === 'unrunnable' ? (status?.path ?? null) : null,
    source,
    os,
  };
}

/** UI §5.5: the paste-into-a-bug-report block. `null` when there is no truthful
 *  content — i.e. the latch fired before any probe landed (§6). */
export function buildTechnicalDetails(status: GitAvailability | null): string | null {
  if (status === null) return null;
  return [
    status.detail,
    `Resolved from: ${sourceLabel(status.source)}`,
    `Path: ${status.path ?? '(none)'}`,
  ].join('\n');
}

/** UI §5.7 (normative): the first-appearance announcement, COMPOSED from the
 *  same `BannerCopy` the visible bar renders — never assembled from literals at
 *  the call site. That is the whole point: a live region whose text contradicts
 *  the visible text (the pre-tightening defect: Variant B announced the Variant
 *  A diagnosis, with no remedy at all) actively misleads.
 *
 *  Carries the remedy — a screen-reader user told only that something is broken
 *  is worse off than a sighted user glancing at the bar — and never the
 *  `Tried:` path or the disclosure content. */
export function buildAnnouncement(copy: BannerCopy): string {
  return `${copy.title}. ${copy.explanation} ${copy.remedy}`;
}

/** Deliberately short: the diagnosis and remedy are unchanged and were already
 *  announced, so repeating them on every retry is noise. */
export const ANNOUNCE_STILL_UNAVAILABLE = 'Git is still not available.';

/** UI §5.7: the recovery announcement. Same `version ?? 'on this computer'`
 *  degradation as the success toast. */
export function announceAvailable(status: GitAvailability): string {
  return `Git is available. Bonsai found Git ${status.version ?? 'on this computer'}.`;
}

/** `Still not found — checked 14:03.` (local 24h, zero-padded). */
export function checkedAtLine(at: Date): string {
  const hh = String(at.getHours()).padStart(2, '0');
  const mm = String(at.getMinutes()).padStart(2, '0');
  return `Still not found — checked ${hh}:${mm}.`;
}

/** UI §5.6: the one toast fired on a user-initiated `false → true` transition. */
export function gitAvailableToastText(status: GitAvailability): string {
  return `Git is available again — Bonsai found Git ${status.version ?? 'on this computer'}.`;
}
