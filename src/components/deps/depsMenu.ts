// P125-ui §5.3 — the row context menu's items. Copying is local-only, so it is
// offered for private rows too; there is no registry item for local/private
// rows (they have no URL — §7.6).
import type { ContextMenuItem } from '../ContextMenu';
import type { DepRow } from '../../ipc/types/depTracker';
import {
  ADVISORY_HOST,
  MENU_COPY_NAME,
  MENU_COPY_NAME_VERSION,
  menuOpenOn,
  menuShowAllAdvisories,
  openAdvisory,
} from './depsCopy';
import { hostLabel } from './depsView';

/** At most this many `Open advisory {id}` items before `Show all {n} advisories`. */
export const MENU_ADVISORY_CAP = 5;

export interface DepsMenuHandlers {
  openUrl(url: string, host: string): void;
  expand(): void;
  copy(text: string): void;
}

export function depsMenuItems(row: DepRow, h: DepsMenuHandlers): ContextMenuItem[] {
  const items: ContextMenuItem[] = [];
  const kind = row.availability.kind;
  const url = row.registryUrl;
  if (url !== null && kind !== 'local' && kind !== 'private') {
    const host = hostLabel(row.ecosystem);
    items.push({ label: menuOpenOn(host), onSelect: () => h.openUrl(url, host) });
  }
  for (const a of row.vulns.slice(0, MENU_ADVISORY_CAP)) {
    items.push({ label: openAdvisory(a.id), onSelect: () => h.openUrl(a.url, ADVISORY_HOST) });
  }
  if (row.vulns.length > MENU_ADVISORY_CAP) {
    items.push({ label: menuShowAllAdvisories(row.vulns.length), onSelect: h.expand });
  }
  if (items.length > 0) items.push({ label: '', separator: true });
  items.push({ label: MENU_COPY_NAME, onSelect: () => h.copy(row.name) });
  items.push({
    label: MENU_COPY_NAME_VERSION,
    onSelect: () => h.copy(`${row.name}@${row.current ?? row.declared ?? ''}`),
  });
  return items;
}
