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
import { IdentityColorSwatch } from './IdentityColorSwatch';
import { autoDistinctColors, nextFreeHue } from './identityProfileColor';
import type { ProfileColor } from '../ipc';
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
function IdentityMenuHeader({
  identity,
  matchedColor,
}: {
  identity: EffectiveIdentity;
  /** P82 (UI §3.2): the matched profile's display color, or null when the
   *  effective identity matches no saved profile (then no swatch). */
  matchedColor: ProfileColor | null;
}) {
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
            {matchedColor !== null && <IdentityColorSwatch color={matchedColor} />}
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
  // P82 (UI §6): the render-time display colors (auto-distinct fallback for
  // pre-P82 color-less profiles), aligned by index with `profiles`.
  const displayColors = autoDistinctColors(profiles);
  const matchedIndex = matched === null ? -1 : profiles.findIndex((p) => p.id === matched.id);
  const matchedColor = matchedIndex >= 0 ? displayColors[matchedIndex] : null;

  const menuOpen = anchor !== null;
  const open = menuOpen || confirm !== null;

  // P103. The lift MUST happen inside the discrete event that opens or closes
  // the menu, never only from the effect below. React assigns an update the lane
  // of the event that scheduled it: a `setState` called from a passive effect
  // gets the DEFAULT (interruptible) lane, so in a production build App's
  // re-render — and with it the re-subscription of `useAppShortcuts`' window
  // keydown listener — can be deferred past the NEXT keypress. The measured
  // symptom: Esc closed this menu, the effect ran with `open=false`, and the
  // very next `Ctrl+,` was still swallowed by a listener closed over the stale
  // `menuOpen=true`; App re-rendered only afterwards. (StrictMode's extra work
  // in dev usually flushes it in time, which is why dev was merely flaky.)
  // Lifting from the handler puts the update on the discrete lane, which React
  // flushes before the next event is dispatched. Mirrored refs are what make the
  // next value computable synchronously — `ContextMenu` calls `onClose()` in the
  // same tick as `onSelect()`, so a plain `false` in `closeMenu` would briefly
  // unsuppress the shortcuts underneath the confirm dialog.
  const anchorRef = useRef<{ x: number; y: number } | null>(null);
  const confirmRef = useRef<IdentityProfile | null>(null);
  const liftedRef = useRef(false);
  // Latest callback without making the lift depend on its identity (the unmount
  // hand-back must not re-run when the parent re-creates the prop).
  const onMenuOpenChangeRef = useRef(onMenuOpenChange);
  onMenuOpenChangeRef.current = onMenuOpenChange;

  /** Idempotent — the reconcile effect and the handlers both call it. */
  const lift = useCallback((next: boolean) => {
    if (liftedRef.current === next) return;
    liftedRef.current = next;
    onMenuOpenChangeRef.current(next);
  }, []);

  const syncLift = useCallback(() => {
    lift(anchorRef.current !== null || confirmRef.current !== null);
  }, [lift]);

  const applyAnchor = useCallback(
    (next: { x: number; y: number } | null) => {
      anchorRef.current = next;
      setAnchor(next);
      syncLift();
    },
    [syncLift],
  );

  const applyConfirm = useCallback(
    (next: IdentityProfile | null) => {
      confirmRef.current = next;
      setConfirm(next);
      syncLift();
    },
    [syncLift],
  );

  // Safety net only: reconciles the lift with the rendered state (a path that
  // changed `open` without going through the helpers above), and hands the
  // suppression back if this control unmounts while open — otherwise App's
  // global shortcuts would stay dead for the rest of the session.
  useEffect(() => {
    lift(open);
  }, [open, lift]);
  useEffect(
    () => () => {
      if (liftedRef.current) {
        liftedRef.current = false;
        onMenuOpenChangeRef.current(false);
      }
    },
    [],
  );

  const closeMenu = useCallback(() => {
    if (applyingRef.current !== null) return;
    applyAnchor(null);
  }, [applyAnchor]);

  const toggle = () => {
    if (menuOpen) {
      closeMenu();
      return;
    }
    const rect = triggerRef.current?.getBoundingClientRect();
    if (rect === undefined) return;
    applyAnchor({ x: rect.right, y: rect.bottom + 2 });
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
        applyAnchor(null);
        applyConfirm(null);
      }
    },
    [repoId, pushToast, applyAnchor, applyConfirm],
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
      applyConfirm(profile);
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
      // P82 (UI §6): a new profile gets the next free hue, not neutral.
      color: nextFreeHue(profiles),
    };
    onProfilesChange([...profiles, draft]);
    onOpenSettingsAt('identities', null, draft.id);
  };

  const goTo = (category: SettingsCategoryId, focus: 'identity' | null = null) => {
    onOpenSettingsAt(category, focus);
  };

  const items: ContextMenuItem[] = profiles.map((p, i) => ({
    label: applyingId === p.id ? `${profileDisplayName(p)} — Applying…` : profileDisplayName(p),
    detail: `${p.userName} · ${p.userEmail}`,
    // P82 (UI §3.2): reuse the existing icon slot for the leading swatch.
    icon: <IdentityColorSwatch color={displayColors[i]} size="sm" />,
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
          header={<IdentityMenuHeader identity={identity} matchedColor={matchedColor} />}
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
          onCancel={() => applyConfirm(null)}
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
