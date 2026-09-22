// P118: the Stashes section, lifted verbatim out of Sidebar.tsx to keep that
// container under the ~500-line ceiling once the busy-context providers landed.
// Markup, ordering and class names are byte-preserved; the only change is that
// the "Stash changes" button is a `SidebarActionButton`, which reads the
// in-flight flag from `SidebarBusyContext` instead of taking it as a prop.
//
// Deliberately NOT `memo`'d: it was inline in the (unmemoised) Sidebar body, so
// memoising it here would be a behaviour change smuggled into a file move. Its
// rows are memoised, which is where the render cost actually is.
import type { Dispatch, SetStateAction } from 'react';
import type { StashEntry } from '../../ipc';
import type { RevealTarget } from '../../graph/reveal';
import { StashIcon } from '../appIcons';
import { SectionHeader } from './SectionHeader';
import { SidebarActionButton } from './SidebarActionButton';
import { StashRow } from './rows';

export interface StashesSectionProps {
  stashes: StashEntry[];
  collapsed: boolean;
  setCollapsed: Dispatch<SetStateAction<boolean>>;
  /** Hidden on an unborn HEAD — there is nothing to stash against yet. */
  headUnborn: boolean;
  onCreateStash(): void;
  onContextMenu(index: number, oid: string, clientX: number, clientY: number): void;
  onReveal?: (t: RevealTarget) => void;
  /** Unix SECONDS, ticked by the container — see `SidebarProps.now`. */
  now: number;
}

export function StashesSection({
  stashes,
  collapsed,
  setCollapsed,
  headUnborn,
  onCreateStash,
  onContextMenu,
  onReveal,
  now,
}: StashesSectionProps) {
  return (
    <section className="sidebar-section">
      <SectionHeader
        label="Stashes"
        collapsed={collapsed}
        onToggle={() => setCollapsed((c) => !c)}
        extra={
          !headUnborn && (
            <SidebarActionButton
              label="Stash changes"
              iconOnly
              onClick={() => {
                setCollapsed(false);
                onCreateStash();
              }}
            >
              <StashIcon />
            </SidebarActionButton>
          )
        }
      />
      {!collapsed &&
        (stashes.length === 0 ? (
          <p className="branch-muted">No stashes</p>
        ) : (
          <ul className="branch-list" role="group">
            {stashes.map((s) => (
              <StashRow
                key={s.index}
                index={s.index}
                oid={s.oid}
                message={s.message}
                ts={s.ts}
                now={now}
                onContextMenu={onContextMenu}
                onReveal={onReveal}
                treeKey={`stash:${s.index}`}
              />
            ))}
          </ul>
        ))}
    </section>
  );
}
