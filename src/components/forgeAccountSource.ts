// P80 (UI §0.1): the single source of truth for the `AccountSource` display
// vocabulary. A pure map (no JSX) so the same strings back the PR-panel switcher
// caption, its tooltip, and any Settings hint — the microcopy cannot drift.
import type { AccountSource } from '../ipc';

/** Short in-header caption for why this account is active. `null` for `single`
 *  (nothing to disambiguate) and `none` (the connect view shows instead). */
export function accountSourceCaption(source: AccountSource): string | null {
  switch (source) {
    case 'override':
      return 'Pinned to this repo';
    case 'ownerMatch':
      return 'Matched by owner';
    case 'hostDefault':
      return 'Host default';
    case 'single':
    case 'none':
      return null;
    default:
      return null;
  }
}

/** The long-form tooltip for the caption. `null` where no caption is shown. */
export function accountSourceTooltip(source: AccountSource): string | null {
  switch (source) {
    case 'override':
      return 'Pinned to this repository. Other repositories on this host use the default.';
    case 'ownerMatch':
      return "Chosen because its username matches this repository's owner.";
    case 'hostDefault':
      return 'The default account for this host.';
    case 'single':
    case 'none':
      return null;
    default:
      return null;
  }
}
