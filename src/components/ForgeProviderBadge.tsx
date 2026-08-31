// P79 (UI §0): a hueless 2-letter provider monogram, matching the house
// letter-badge precedent. The provider is named by the LETTERS (and the
// accessible name / tooltip), never a color — so it reads the same in both
// themes and in forced-colours mode. Presentational; no IPC.
import type { ForgeKind } from '../ipc';

interface ProviderDisplay {
  badge: string;
  label: string;
}

/** Full `Record<ForgeKind, …>` so a new `ForgeKind` is a compile error until its
 *  badge + label are supplied. */
const PROVIDER_DISPLAY: Record<ForgeKind, ProviderDisplay> = {
  gitHub: { badge: 'GH', label: 'GitHub' },
  gitLab: { badge: 'GL', label: 'GitLab' },
  bitbucket: { badge: 'BB', label: 'Bitbucket' },
  azureDevOps: { badge: 'AZ', label: 'Azure DevOps' },
  unknown: { badge: '??', label: 'Unknown forge' },
};

/** The human label for a `ForgeKind` (accessible name / add-form option text). */
export function forgeKindLabel(kind: ForgeKind): string {
  return PROVIDER_DISPLAY[kind].label;
}

export function ForgeProviderBadge({ kind }: { kind: ForgeKind }) {
  const { badge, label } = PROVIDER_DISPLAY[kind];
  return (
    <span className="forge-provider-badge" title={label} aria-label={label}>
      {badge}
    </span>
  );
}
