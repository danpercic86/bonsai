// P73 §2: extracted from Sidebar.tsx (over the ~500-line soft limit) so the
// submodule row — badge copy, busy state, context-menu hand-off — lives in one
// small file. First component in src/components/sidebar/.
import type { SubmoduleInfo } from '../../ipc';
import type { SubmoduleBusy } from '../repoWorkspace/types';
import { SUBMODULE_BADGE } from './submoduleBadges';
import { useSidebarTreeItem } from './useSidebarTreeItem';

export function SubmoduleRow({
  sub,
  submoduleBusy,
  onContextMenu,
  treeKey,
  level = 2,
}: {
  sub: SubmoduleInfo;
  /** P73 §6.1: the row with an op in flight + the participle its badge shows. */
  submoduleBusy: SubmoduleBusy | null;
  onContextMenu(name: string, clientX: number, clientY: number): void;
  /** P-a11y §D.2: treeitem key (`submodule:<name>`) + aria-level. */
  treeKey?: string;
  level?: number;
}) {
  const badge = SUBMODULE_BADGE[sub.status];
  const busy = submoduleBusy?.name === sub.name ? submoduleBusy.label : null;
  const item = useSidebarTreeItem({
    treeKey: treeKey ?? `submodule:${sub.name}`,
    level,
    kind: 'leaf',
    menuIsPrimary: true,
    openMenuAt: (x, y) => onContextMenu(sub.name, x, y),
  });
  return (
    <li
      {...item}
      role="treeitem"
      className="branch-row"
      aria-busy={busy !== null ? true : undefined}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(sub.name, e.clientX, e.clientY);
      }}
    >
      <span className="branch-glyph">{'⊡'}</span>
      <span className="branch-name" title={sub.path}>
        {sub.name}
      </span>
      {busy !== null ? (
        // The participle IS the whole message — no title to add.
        <span className="branch-badge submodule-badge-busy">
          <span>{busy}</span>
        </span>
      ) : (
        <span className={`branch-badge ${badge.intent}`} title={badge.title}>
          {badge.glyph !== null && (
            <span className="submodule-badge-glyph" aria-hidden="true">
              {badge.glyph}
            </span>
          )}
          <span>{badge.label}</span>
        </span>
      )}
    </li>
  );
}
