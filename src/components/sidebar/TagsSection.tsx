// P77 §8: the sidebar Tags section, extracted from Sidebar.tsx (already over the
// ~500-line soft limit) so the tag-sync overlay — per-row badges, the collapsed
// rollup, remote-only ghost rows and the offline "couldn't reach" line — lives in
// one focused file. Presentational: all IPC + git logic stays in the container;
// this only renders the precomputed TagSyncReport.
import { useMemo, useState } from 'react';
import type { TagSyncEntry, TagSyncReport } from '../../ipc';
import { relativeDate } from '../../graph/draw';
import { buildPathTree } from '../../utils/pathTree';
import { Tree } from '../Tree';
import { ListFilterInput } from '../ListFilterInput';
import { filterByName, filterItems, filterTree } from '../repoWorkspace/listFilter';
import { SectionHeader } from './SectionHeader';
import { TagSyncBadge } from './TagSyncBadge';
import { SectionRollupBadge } from './SectionRollupBadge';
import { useSidebarTreeItem } from './useSidebarTreeItem';

/** P50d: show a section's inline filter box only once the list is long enough. */
const FILTER_MIN_ROWS = 6;

/** P77 lifecycle of the live ls-remote reconciliation. */
export type TagSyncState = 'idle' | 'checking' | 'ready' | 'unavailable';

function TagRow({
  name,
  displayName,
  sync,
  remote,
  ghost,
  onContextMenu,
  treeKey,
  level = 2,
}: {
  name: string;
  displayName?: string;
  /** P77 sync verdict for this tag (undefined until state==='ready'). */
  sync?: TagSyncEntry;
  /** The remote the verdict is against, for badge tooltips. */
  remote: string | null;
  /** A remote-only ghost row (dimmed — not in the local repo yet). */
  ghost?: boolean;
  onContextMenu(name: string, clientX: number, clientY: number): void;
  /** P-a11y §D.2: treeitem key (`tag:<name>`) + aria-level. */
  treeKey: string;
  level?: number;
}) {
  const item = useSidebarTreeItem({
    treeKey,
    level,
    kind: 'leaf',
    menuIsPrimary: true,
    openMenuAt: (x, y) => onContextMenu(name, x, y),
  });
  return (
    <li
      {...item}
      role="treeitem"
      className={ghost ? 'branch-row branch-row-readonly branch-row-ghost' : 'branch-row branch-row-readonly'}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(name, e.clientX, e.clientY);
      }}
    >
      {/* §2.6/§8: the ghost row's dimming comes from the row-level opacity
          (.branch-row-ghost), not an inverted (brighter) glyph muting. */}
      <span className="branch-glyph">{'#'}</span>
      <span className="branch-name branch-name-muted" title={name}>
        {displayName ?? name}
      </span>
      {sync !== undefined && remote !== null && (
        <TagSyncBadge
          status={sync.status}
          tag={name}
          remote={remote}
          localOid={sync.localOid}
          remoteOid={sync.remoteOid}
        />
      )}
    </li>
  );
}

export function TagsSection({
  tags,
  treeMode,
  onTagContextMenu,
  tagSyncReport,
  tagSyncState,
  tagSyncRemote,
  tagSyncCheckedAt,
  onExpand,
}: {
  tags: string[];
  treeMode: boolean;
  onTagContextMenu(name: string, clientX: number, clientY: number): void;
  tagSyncReport: TagSyncReport | null;
  tagSyncState: TagSyncState;
  /** The remote the check targets, available even when no report was obtained
   *  (cold-start-offline), so the §2.3 offline line can name it. */
  tagSyncRemote: string | null;
  tagSyncCheckedAt: number | null;
  /** Fired when the section transitions collapsed → expanded (sync trigger §6). */
  onExpand(): void;
}) {
  // P11a: Tags start collapsed (least-used, can be long). Local/ephemeral state.
  const [collapsed, setCollapsed] = useState(true);
  const [tagFilter, setTagFilter] = useState('');

  const ready = tagSyncState === 'ready' && tagSyncReport !== null;
  const remote = tagSyncReport?.remote ?? null;
  // For the offline line: prefer the report's echoed remote, else the resolved
  // default the check attempted (report is null on cold-start-offline).
  const offlineRemote = remote ?? tagSyncRemote;

  // Per-name verdict for badges + menu; only populated when the check is ready so
  // rows stay badge-less while checking / unavailable (§2.2/§2.3).
  const byName = useMemo(() => {
    const m = new Map<string, TagSyncEntry>();
    if (ready) for (const e of tagSyncReport!.entries) m.set(e.name, e);
    return m;
  }, [ready, tagSyncReport]);
  const remoteOnly = useMemo(
    () => (ready ? tagSyncReport!.entries.filter((e) => e.status === 'remote-only') : []),
    [ready, tagSyncReport],
  );
  // Rollup counts TRUE divergences only — unpushed/remote-only don't inflate it.
  const divergedCount = useMemo(
    () =>
      ready
        ? tagSyncReport!.entries.filter(
            (e) => e.status === 'stale' || e.status === 'deleted-on-remote',
          ).length
        : 0,
    [ready, tagSyncReport],
  );

  // P50d — the filter box shows only when expanded AND the list is long enough.
  const showFilter = !collapsed && tags.length >= FILTER_MIN_ROWS;
  const query = showFilter ? tagFilter : '';
  const filtering = query.trim() !== '';
  const tagsFiltered = filterByName(tags, query);
  const tagTree = useMemo(
    () => (treeMode ? buildPathTree(tags, (t) => t) : []),
    [treeMode, tags],
  );
  const tagTreeFiltered = filterTree(tagTree, query, (t) => t);
  const remoteOnlyFiltered = filterItems(remoteOnly, query, (e) => e.name);
  const noMatch =
    filtering && tagsFiltered.length === 0 && remoteOnlyFiltered.length === 0;

  const toggle = () => {
    // §6: collapsed → expanded fires the sync trigger. Fire it from the event
    // handler (not inside the setCollapsed updater, which runs during render and
    // must stay pure — calling a parent setState there warns "Cannot update a
    // component while rendering a different component").
    if (collapsed) onExpand();
    setCollapsed((c) => !c);
  };

  const rollup = (
    <SectionRollupBadge
      count={divergedCount}
      busy={tagSyncState === 'checking'}
      label={'checking…'}
      ariaLabel={`${divergedCount} tags out of sync${remote !== null ? ` on ${remote}` : ''}`}
      title={`${divergedCount} tags differ from ${remote ?? 'the remote'}. Expand to resolve.`}
    />
  );

  const empty = tags.length === 0 && remoteOnlyFiltered.length === 0;

  return (
    <section className="sidebar-section">
      <SectionHeader label="Tags" collapsed={collapsed} onToggle={toggle} extra={rollup} />
      {!collapsed && (
        <>
          {showFilter && (
            <ListFilterInput
              value={tagFilter}
              onChange={setTagFilter}
              ariaLabel="Filter tags"
              count={filtering ? tagsFiltered.length + remoteOnlyFiltered.length : undefined}
            />
          )}
          {/* §2.3: offline degrade — informational, never an error banner. Shown
              on every unavailable check (incl. cold-start-offline, where no
              report was ever obtained), naming the targeted remote when known. */}
          {tagSyncState === 'unavailable' && (
            <p
              className="branch-muted"
              title={
                tagSyncCheckedAt !== null
                  ? `Last checked ${relativeDate(tagSyncCheckedAt, Math.floor(Date.now() / 1000))}`
                  : undefined
              }
            >
              {offlineRemote !== null
                ? `Couldn't reach ${offlineRemote} — showing local tags only.`
                : `Couldn't reach the remote — showing local tags only.`}
            </p>
          )}
          {empty ? (
            <p className="branch-muted">No tags</p>
          ) : noMatch ? (
            <p className="branch-muted">{`No tags match '${tagFilter.trim()}'`}</p>
          ) : (
            <>
              {treeMode ? (
                tagTreeFiltered.length > 0 && (
                  <Tree
                    key={filtering ? 'tags-filter' : 'tags'}
                    asGroup
                    nodes={tagTreeFiltered}
                    leafKey={(l) => l.item}
                    defaultCollapsed={!filtering}
                    initiallyExpanded={[]}
                    renderLeaf={(l, level) => (
                      <TagRow
                        name={l.item}
                        displayName={l.name}
                        sync={byName.get(l.item)}
                        remote={remote}
                        onContextMenu={onTagContextMenu}
                        treeKey={`tag:${l.item}`}
                        level={level}
                      />
                    )}
                  />
                )
              ) : (
                tagsFiltered.length > 0 && (
                  <ul className="branch-list" role="group">
                    {tagsFiltered.map((tag) => (
                      <TagRow
                        key={tag}
                        name={tag}
                        sync={byName.get(tag)}
                        remote={remote}
                        onContextMenu={onTagContextMenu}
                        treeKey={`tag:${tag}`}
                      />
                    ))}
                  </ul>
                )
              )}
              {/* §2.6: remote-only tags absent locally → ghost rows, appended
                  after the local list in both flat and tree modes. */}
              {remoteOnlyFiltered.length > 0 && (
                <ul className="branch-list" role="group">
                  {remoteOnlyFiltered.map((e) => (
                    <TagRow
                      key={`remote-only:${e.name}`}
                      name={e.name}
                      sync={e}
                      remote={remote}
                      ghost
                      onContextMenu={onTagContextMenu}
                      treeKey={`tag:${e.name}`}
                    />
                  ))}
                </ul>
              )}
            </>
          )}
        </>
      )}
    </section>
  );
}
