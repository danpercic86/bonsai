import { createElement } from 'react';
import type { ContextMenuItem } from './ContextMenu';
import { CopyIcon, DeleteIcon, SummarizeIcon, TagIcon } from './menuIcons';
import { errorMessage } from '../utils/errors';
import type { WorkspaceMenuDeps } from './workspaceMenus';
import { checkoutMenuItems, commitActionItems } from './workspaceMenusCommit';

// P22 §7.2: the shared tag menu — used by the graph tag pill AND the sidebar
// tag rows. Delete (ConfirmDialog) + Copy + one "Push tag to <remote>" per
// configured remote (§OPEN-7: 0 → no push item; 1 → single; >1 → one each).
export function tagMenuItems(
  deps: WorkspaceMenuDeps,
  name: string,
  oid: string | null,
): ContextMenuItem[] {
  const {
    mutating,
    opActive,
    aiEligible,
    remotes,
    pushToast,
    tagSync,
    handleForceRefreshTag,
    handleFetchRemoteTag,
    handlePushTag,
    runChangelog,
    setPendingDeleteTag,
    setPendingDeleteRemoteTag,
    setPendingForceMoveTag,
  } = deps;
  const gate = mutating || opActive;
  // P77 §3: the per-name sync verdict gates the resolve items. Undefined when no
  // check has run / the remote is unavailable → the menu degrades to the
  // pre-P77 set (copy / release notes / push / delete-local).
  const entry = tagSync?.entries.find((e) => e.name === name);
  const status = entry?.status;
  const syncRemote = tagSync?.remote ?? null;
  const isRemoteOnly = status === 'remote-only';
  const oldShort = entry?.remoteOid?.slice(0, 7) ?? '';
  const newShort = entry?.localOid?.slice(0, 7) ?? '';
  // Item 7 shows only when the tag exists on the remote (in-sync/stale/remote-
  // only) — never for unpushed (nothing there) or the reserved deleted-on-remote.
  const existsOnRemote =
    status === 'in-sync' || status === 'stale' || status === 'remote-only';

  const items: ContextMenuItem[] = [];

  // Checkout is the most-primary action → FIRST. Graph tag pills pass an oid;
  // sidebar tag rows pass null (no reachable target) → no checkout (matches today).
  if (oid !== null) items.push(...checkoutMenuItems(deps, oid));

  // 1. Update to remote target (stale) — resolve-in-place, no confirm (§3).
  if (status === 'stale' && syncRemote !== null) {
    items.push({
      label: 'Update to remote target',
      icon: createElement(TagIcon),
      disabled: gate,
      onSelect: () => void handleForceRefreshTag(syncRemote, name),
    });
  }
  // 2. Create local tag (remote-only ghost row).
  if (isRemoteOnly && syncRemote !== null) {
    items.push({
      label: 'Create local tag',
      icon: createElement(TagIcon),
      disabled: gate,
      onSelect: () => void handleFetchRemoteTag(syncRemote, name),
    });
  }
  // 3. Push tag to {remote} (existing) — one per configured remote. Skipped for
  // a remote-only row (no local tag to push).
  if (!isRemoteOnly) {
    for (const r of remotes) {
      items.push({
        label: `Push tag to ${r.name}`,
        icon: createElement(TagIcon),
        disabled: gate,
        onSelect: () => void handlePushTag(r.name, name),
      });
    }
  }
  // 4. Copy tag name (existing) — always.
  items.push({
    label: 'Copy tag name',
    icon: createElement(CopyIcon),
    disabled: false,
    onSelect: () => {
      const p =
        navigator.clipboard?.writeText(name) ??
        Promise.reject(new Error('Clipboard unavailable'));
      void p
        .then(() => pushToast('success', 'Copied tag name'))
        .catch((e) => pushToast('error', `Copy failed: ${errorMessage(e)}`));
    },
  });
  // 5. Release notes since previous tag (existing) — AI-gated. Not for a ghost
  // row (no local history to summarise).
  if (!isRemoteOnly) {
    items.push({
      label: 'Release notes since previous tag',
      icon: createElement(SummarizeIcon),
      disabled: !aiEligible,
      onSelect: () =>
        runChangelog({ kind: 'sinceLastTag', target: name }, `Release notes for ${name}`),
    });
  }
  // 6. Delete tag (existing, local) — skipped for a remote-only ghost (no local
  // tag exists). Routes through the existing local confirm.
  if (!isRemoteOnly) {
    items.push({
      label: 'Delete tag',
      icon: createElement(DeleteIcon),
      disabled: gate,
      onSelect: () => setPendingDeleteTag(name),
    });
  }
  // 7. Delete tag on {remote}… (danger → confirm §4.1).
  if (existsOnRemote && syncRemote !== null) {
    items.push({
      label: `Delete tag on ${syncRemote}…`,
      icon: createElement(DeleteIcon),
      disabled: gate,
      tone: 'danger',
      onSelect: () => setPendingDeleteRemoteTag({ name, remote: syncRemote }),
    });
  }
  // 8. Force-move tag on {remote}… (stale, danger → confirm §4.2).
  if (status === 'stale' && syncRemote !== null) {
    items.push({
      label: `Force-move tag on ${syncRemote}…`,
      icon: createElement(TagIcon),
      disabled: gate,
      tone: 'danger',
      onSelect: () =>
        setPendingForceMoveTag({ name, remote: syncRemote, oldShort, newShort }),
    });
  }
  // P47 (Part A, Fork-1): a GRAPH tag pill carries its target oid (the node id),
  // so it gets the shared commit actions after the tag-specific items. Sidebar
  // tag rows pass `oid === null` and keep only the tag/sync actions above.
  if (oid !== null) items.push(...commitActionItems(deps, oid));
  return items;
}
