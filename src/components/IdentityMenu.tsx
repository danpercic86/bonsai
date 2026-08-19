// P69i — UI §4 / ui-reference §12.6: the header identity control.
//
// The daily question "which identity does this repository commit as?" is today
// invisible until a commit FAILS. This puts it in the toolbar's account slot and
// makes switching a two-click action, which is what let the Settings "Identity
// profiles" Apply flow stop being the only route.
//
// Built on `ContextMenu` rather than a hand-rolled popover: that primitive
// already owns keyboard navigation, viewport clamping and every dismiss path
// (Esc, outside pointerdown, scroll, resize, blur). Anchored with the house
// idiom — `rect.right / rect.bottom + 2` — whose existing right-edge clamp flips
// the menu leftward, which is exactly what a far-right trigger needs.
//
// Open state is LIFTED via `onMenuOpenChange` (the `TabStrip.tsx:35-37`
// precedent) because App early-returns its global shortcuts and its Esc handler
// while a menu is open.

import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../ipc';
import type { IdentityProfile } from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage } from '../utils/errors';
import {
  invalidateEffectiveIdentity,
  useEffectiveIdentity,
  type EffectiveIdentity,
} from '../hooks/useEffectiveIdentity';
import { ConfirmDialog } from './ConfirmDialog';
import { ContextMenu, type ContextMenuItem } from './ContextMenu';
import { IdentityAvatar } from './IdentityAvatar';
import {
  hasUsableIdentity,
  identitySourceLine,
  identityTriggerCopy,
  matchProfile,
  profileDisplayName,
} from './identityCopy';
import type { SettingsCategoryId } from './settings/types';

export interface IdentityMenuProps {
  /** Rendered only when a repo is open — `getConfig` needs a repo id (§4.1). */
  repoId: string;
  profiles: IdentityProfile[];
  /** Whole-array replace, exactly as `SettingsProfilesSection` uses it — the
   *  channel `Save “…” as an identity…` writes its draft through (§4.3 item 2). */
  onProfilesChange(next: IdentityProfile[]): void;
  onOpenSettingsAt(
    category: SettingsCategoryId,
    focus?: 'identity' | null,
    focusProfileId?: string | null,
  ): void;
  /** Lifted so App keeps suppressing global shortcuts while this is open. */
  onMenuOpenChange(open: boolean): void;
}

/** UI §4.3's non-interactive header block. */
function IdentityMenuHeader({ identity }: { identity: EffectiveIdentity }) {
  const unset = identity.error !== null || !hasUsableIdentity(identity);
  return (
    <div className="identity-menu-header">
      <p className="identity-menu-eyebrow">{'Committing as'}</p>
      {identity.loading ? (
        <p className="identity-menu-line">{'Reading commit identity…'}</p>
      ) : unset ? (
        <>
          <p className="identity-menu-name">{'No commit identity set'}</p>
          <p className="identity-menu-line">
            {identity.error !== null
              ? identitySourceLine(identity)
              : 'Commits will fail until you set a name and email.'}
          </p>
        </>
      ) : (
        <>
          <p className="identity-menu-name" title={identity.name ?? ''}>
            {identity.name}
          </p>
          <p className="identity-menu-line" title={identity.email ?? ''}>
            {identity.email}
          </p>
          <p className="identity-menu-source">{identitySourceLine(identity)}</p>
        </>
      )}
    </div>
  );
}

export function IdentityMenu({
  repoId,
  profiles,
  onProfilesChange,
  onOpenSettingsAt,
  onMenuOpenChange,
}: IdentityMenuProps) {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null);
  const [confirm, setConfirm] = useState<IdentityProfile | null>(null);
  const [applyingId, setApplyingId] = useState<string | null>(null);
  // Read synchronously by `closeMenu`: `ContextMenu` calls `onClose()` right
  // after `onSelect()`, and UI §4.5 wants the menu to STAY open (aria-busy)
  // until the write settles. A ref is the only thing that has the new value by
  // the time that same-tick close arrives.
  const applyingRef = useRef<string | null>(null);
  const pushToast = usePushToast();

  const identity = useEffectiveIdentity(repoId);
  const matched = matchProfile(identity, profiles);
  const copy = identityTriggerCopy(identity, matched);

  const menuOpen = anchor !== null;
  const open = menuOpen || confirm !== null;
  useEffect(() => {
    onMenuOpenChange(open);
    // Unmount (or transition) while open must hand the suppression back, or
    // App's global shortcuts stay dead for the rest of the session.
    return () => {
      if (open) onMenuOpenChange(false);
    };
  }, [open, onMenuOpenChange]);

  const closeMenu = useCallback(() => {
    if (applyingRef.current !== null) return;
    setAnchor(null);
  }, []);

  const toggle = () => {
    if (menuOpen) {
      closeMenu();
      return;
    }
    const rect = triggerRef.current?.getBoundingClientRect();
    if (rect === undefined) return;
    setAnchor({ x: rect.right, y: rect.bottom + 2 });
  };

  const runApply = useCallback(
    async (profile: IdentityProfile, fromMenu: boolean) => {
      if (fromMenu) applyingRef.current = profile.id;
      setApplyingId(profile.id);
      try {
        // The profile's CURRENT in-memory fields, never an id: App's `profiles`
        // state updates synchronously on edit while only the persist is
        // debounced, so an id lookup could write the pre-edit values.
        await ipc.applyIdentityProfile(
          repoId,
          profile.userName,
          profile.userEmail,
          profile.signingKey,
        );
        // `setConfig` deliberately does not emit `repo-changed`, so the shared
        // store is told explicitly (contract §5.1).
        invalidateEffectiveIdentity(repoId);
        pushToast(
          'success',
          `Now committing as ${profileDisplayName(profile)} in this repository.`,
          `identity:${repoId}`,
        );
      } catch (e) {
        pushToast('error', `Couldn’t switch identity. ${errorMessage(e)}`, `identity:${repoId}`);
      } finally {
        applyingRef.current = null;
        setApplyingId(null);
        setAnchor(null);
        setConfirm(null);
      }
    },
    [repoId, pushToast],
  );

  /**
   * UI §4.5, the confirmation rule — and only this rule:
   *   - this profile is the one already in effect ⇒ no-op close;
   *   - the repo has a differing LOCAL identity   ⇒ confirm first;
   *   - anything else (it inherits global, or has none) ⇒ write immediately,
   *     because writing into an empty slot destroys nothing.
   *
   * The first clause is deliberately NOT restricted to `source === 'local'`,
   * which is the narrow reading of §4.5's table. §4.3 item 1 calls the checked
   * row "a no-op close", the tick the user is clicking is computed from the
   * EFFECTIVE identity, and "clicking the thing that is already ticked writes to
   * .git/config" is indefensible whatever the table says. Ruling by the
   * orchestrator, P69i review — do not "fix" this back.
   *
   * "Local" means EITHER half is local: `user.name` global + `user.email` local
   * is an ordinary per-repo override, and applying would overwrite that email.
   */
  const select = (profile: IdentityProfile) => {
    // A row stays enabled while its own write is in flight (the menu is held
    // open on purpose), so without this a second click re-enters and fires a
    // duplicate write whose `finally` races the first on `applyingRef`.
    if (applyingId !== null) return;
    const hasLocal =
      hasUsableIdentity(identity) &&
      (identity.source === 'local' || identity.emailSource === 'local');
    if (matchProfile(identity, [profile]) !== null) return;
    if (hasLocal) {
      setConfirm(profile);
      return;
    }
    void runApply(profile, true);
  };

  /**
   * §4.3 item 2. The label promises an action, so it performs one: the effective
   * identity is appended as a real saved profile and Settings opens on its card
   * with the Label field focused. Landing on an unrelated list and making the
   * user retype what is already on screen would make the row a lie.
   */
  const saveEffectiveAsProfile = () => {
    const draft: IdentityProfile = {
      id: crypto.randomUUID(),
      label: '',
      userName: identity.name ?? '',
      userEmail: identity.email ?? '',
      signingKey: null,
    };
    onProfilesChange([...profiles, draft]);
    onOpenSettingsAt('identities', null, draft.id);
  };

  const goTo = (category: SettingsCategoryId, focus: 'identity' | null = null) => {
    onOpenSettingsAt(category, focus);
  };

  const items: ContextMenuItem[] = profiles.map((p) => ({
    label: applyingId === p.id ? `${profileDisplayName(p)} — Applying…` : profileDisplayName(p),
    detail: `${p.userName} · ${p.userEmail}`,
    // The FIRST match only. Two profiles holding the same name+email would
    // otherwise both report aria-checked="true" (invalid for menuitemradio) and
    // disagree with Settings' `in use` pill, which lights exactly one.
    checked: matched !== null && matched.id === p.id,
    disabled: applyingId !== null && applyingId !== p.id,
    onSelect: () => select(p),
  }));

  // §4.3 items 2–4: exactly one of these ever renders, and the list is therefore
  // never empty.
  const showSaveAs = copy.state === 'unmatched';
  const showSet = copy.state === 'unset' || copy.state === 'unreadable';
  if (showSaveAs) {
    items.push({
      label: `Save “${identity.name ?? ''}” as an identity…`,
      onSelect: saveEffectiveAsProfile,
    });
  } else if (showSet) {
    items.push({ label: 'Set an identity…', onSelect: () => goTo('git-config', 'identity') });
  } else if (profiles.length === 0) {
    items.push({ label: 'Add an identity…', onSelect: () => goTo('identities') });
  }
  items.push({ label: 'Manage identities…', onSelect: () => goTo('identities') });

  const confirmOld = `${identity.name ?? ''} <${identity.email ?? ''}>`;
  const confirmNew =
    confirm === null ? '' : `${confirm.userName} <${confirm.userEmail}>`;

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className="btn-icon identity-trigger"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        aria-busy={copy.busy || applyingId !== null ? true : undefined}
        aria-label={copy.ariaLabel}
        title={copy.title === '' ? undefined : copy.title}
        onClick={toggle}
      >
        <IdentityAvatar identity={identity} matchedProfile={matched} />
      </button>
      {anchor !== null && (
        <ContextMenu
          x={anchor.x}
          y={anchor.y}
          items={items}
          busy={applyingId !== null}
          header={<IdentityMenuHeader identity={identity} />}
          onClose={closeMenu}
        />
      )}
      {confirm !== null && (
        <ConfirmDialog
          open
          title="Change this repository’s identity?"
          confirmLabel="Change identity"
          confirmVariant="primary"
          busy={applyingId !== null}
          onConfirm={() => void runApply(confirm, false)}
          onCancel={() => setConfirm(null)}
        >
          <p className="dialog-body">
            {`This repository commits as ${confirmOld}, set in its own Git config. Using ${profileDisplayName(confirm)} replaces that with ${confirmNew}.`}
          </p>
          <p className="dialog-body-detail">
            {'Commits you have already made are not changed. You can switch back at any time.'}
          </p>
        </ConfirmDialog>
      )}
    </>
  );
}
