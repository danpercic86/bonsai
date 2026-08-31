// P69h — the Git-config pane's data layer, lifted out of
// `SettingsGitConfigSection.tsx` so that file is a view again.
//
// It owns the `getConfig` read for the selected scope, the per-key drafts, the
// per-key write/unset flow, and the repo-scoped `bonsai.runHooks` toggle.
//
// **One read serves three consumers** (contract §2.4's corrected measurement).
// Opening this pane used to cost THREE `getConfig(repoId, 'local')` round-trips
// for one answer: the pane's own view, `SettingsHooksToggle`'s private read of
// `bonsai.runHooks`, and `useEffectiveIdentity`'s read of `user.*`. All three are
// answered by the same `ConfigView`, so:
//   * the hooks toggle is now presentational and reads `localView.advanced` here;
//   * every local-level load PRIMES the shared identity store
//     (`primeEffectiveIdentity`) instead of letting it fetch its own copy.
// A write to a `user.*` key at the LOCAL level therefore needs no invalidation —
// its own post-write refetch re-primes the store. A write at the GLOBAL level
// does (it changes the effective identity but not this view), and says so.
//
// `localView` is tracked separately from `view` because hooks are always a
// per-repo (Local) concern: switching the pane to Global must not make the Hooks
// row lie, and must not cost a second read to keep it honest. It is published
// under its OWN request id (`localReqId`) rather than the pane's: a local read
// superseded by a switch to Global is still the freshest LOCAL answer, while a
// local read superseded by a newer LOCAL read must not be published at all (or
// an out-of-order resolution would revert the Hooks row).

import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../../ipc';
import type { ConfigLevelArg, ConfigView } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import {
  claimEffectiveIdentity,
  failEffectiveIdentity,
  invalidateEffectiveIdentity,
  primeEffectiveIdentity,
} from '../../hooks/useEffectiveIdentity';

const RUN_HOOKS_KEY = 'bonsai.runHooks';

/** Merge a fresh server-side draft map onto the current one, PRESERVING the local draft for
 *  any key the user is actively editing — i.e. whose input has focus, or whose draft diverges
 *  from the freshly-loaded server value (an unsaved edit). Prevents a post-write refetch from
 *  clobbering a sibling field mid-keystroke (name→email identity flow); keys absent from the
 *  server (just-removed advanced entries) are dropped. */
function mergeDraftsPreservingEdits(
  prev: Record<string, string>,
  server: Record<string, string>,
): Record<string, string> {
  const activeId = (document.activeElement as HTMLElement | null)?.id ?? '';
  const merged: Record<string, string> = { ...server };
  for (const key of Object.keys(prev)) {
    const serverVal = server[key];
    if (serverVal === undefined) continue; // key gone on the server → drop draft
    const focused = activeId === `cfg-${key}` || activeId === `cfg-adv-${key}`;
    const dirty = prev[key] !== serverVal;
    if (focused || dirty) merged[key] = prev[key];
  }
  return merged;
}

/** Unset ⇒ ON (git's default); only an explicit `false` disables (P59a). */
function readRunHooks(view: ConfigView | null): boolean {
  if (view === null) return true;
  const entry = view.advanced.find((a) => a.name.toLowerCase() === RUN_HOOKS_KEY.toLowerCase());
  return entry === undefined || entry.value.trim().toLowerCase() !== 'false';
}

export interface GitConfigEditor {
  view: ConfigView | null;
  loading: boolean;
  loadError: string | null;
  /** No local view yet — the Hooks row does not know its own value. */
  hooksLoading: boolean;
  drafts: Record<string, string>;
  busyKey: string | null;
  fieldErrors: Record<string, string>;
  runHooks: boolean;
  hooksBusy: boolean;
  hooksError: string | null;
  onDraftChange(key: string, value: string): void;
  onCommit(key: string, value: string, hadTarget: boolean): void;
  removeKey(key: string): void;
  reload(): Promise<void>;
  setRunHooks(next: boolean): void;
}

export function useGitConfigEditor(repoId: string, level: ConfigLevelArg): GitConfigEditor {
  const [view, setView] = useState<ConfigView | null>(null);
  const [localView, setLocalView] = useState<ConfigView | null>(null);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  // Editable per-key drafts (curated + advanced), seeded from the fetched view.
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  // Optimistic hooks value, cleared once the write's refetch has landed.
  const [hooksDraft, setHooksDraft] = useState<boolean | null>(null);
  const [hooksBusy, setHooksBusy] = useState(false);
  const [hooksWriteError, setHooksWriteError] = useState<string | null>(null);
  /** Error from the LOCAL read specifically — the Hooks row's own truth source,
   *  which survives the pane switching to Global. */
  const [localError, setLocalError] = useState<string | null>(null);

  const reqId = useRef(0);
  /** Request id for LOCAL reads only (see the header note). */
  const localReqId = useRef(0);

  /** Publish a freshly-read LOCAL view to everything that depends on it — unless
   *  a newer LOCAL read has since started, in which case that read owns both the
   *  Hooks row and the identity claim and this one publishes nothing. */
  const adoptLocal = useCallback(
    (id: string, v: ConfigView, localId: number, claimSeq: number) => {
      if (localId !== localReqId.current) return;
      setLocalView(v);
      setLocalError(null);
      primeEffectiveIdentity(id, v, claimSeq);
    },
    [],
  );

  const load = useCallback(
    // `preserveEdits` (post-write refetch): keep the local draft for a focused/dirty field so
    // an in-flight sibling edit is not clobbered. False on mount / level change → full reset.
    async (lvl: ConfigLevelArg, preserveEdits = false) => {
      const id = ++reqId.current;
      const localId = lvl === 'local' ? ++localReqId.current : localReqId.current;
      setLoading(true);
      setLoadError(null);
      // Claim BEFORE awaiting: an identity consumer mounting in the same commit
      // (the profiles pane, the header trigger) would otherwise fire its own read
      // while this one is still in flight, and the count would be two again.
      const claimSeq = lvl === 'local' ? claimEffectiveIdentity(repoId) : 0;
      try {
        const v = await ipc.getConfig(repoId, lvl);
        if (lvl === 'local') adoptLocal(repoId, v, localId, claimSeq);
        if (id !== reqId.current) return;
        setView(v);
        const d: Record<string, string> = {};
        for (const c of v.curated) d[c.key] = c.targetValue ?? '';
        for (const a of v.advanced) d[a.name] = a.value;
        setDrafts((prev) => (preserveEdits ? mergeDraftsPreservingEdits(prev, d) : d));
        setFieldErrors({});
      } catch (e) {
        const message = errorMessage(e);
        if (lvl === 'local' && localId === localReqId.current) {
          // The Hooks row is driven by the LOCAL read, so it owns this error even
          // when the pane has since moved to Global.
          setLocalError(message);
          failEffectiveIdentity(repoId, message, claimSeq);
        }
        if (id !== reqId.current) return;
        setView(null);
        setLoadError(message);
      } finally {
        if (id === reqId.current) setLoading(false);
      }
    },
    [repoId, adoptLocal],
  );

  useEffect(() => {
    void load(level);
  }, [load, level]);

  /** Refresh the LOCAL view after a local-only write. Free while the pane is at
   *  local scope (it is the same read the pane needs anyway). */
  const refreshLocal = useCallback(async () => {
    if (level === 'local') {
      await load('local', true);
      return;
    }
    const localId = ++localReqId.current;
    const claimSeq = claimEffectiveIdentity(repoId);
    try {
      adoptLocal(repoId, await ipc.getConfig(repoId, 'local'), localId, claimSeq);
    } catch (e) {
      const message = errorMessage(e);
      if (localId === localReqId.current) setLocalError(message);
      failEffectiveIdentity(repoId, message, claimSeq);
      // Keep the previous local view: the Hooks row showing a stale-but-real
      // value beats it flipping to git's default on a transient read failure.
    }
  }, [repoId, level, load, adoptLocal]);

  // Write (or unset) a single key at the current level, then refetch.
  const write = useCallback(
    async (key: string, rawValue: string, hadTarget: boolean) => {
      const value = rawValue.trim();
      setBusyKey(key);
      setFieldErrors((m) => Object.fromEntries(Object.entries(m).filter(([k]) => k !== key)));
      try {
        if (value === '') {
          if (hadTarget) await ipc.unsetConfig(repoId, level, key);
        } else {
          await ipc.setConfig(repoId, level, key, value);
        }
        await load(level, true);
        // A `user.*` write changes the identity every surface shows, and
        // setConfig/unsetConfig emit no `repo-changed`. At local scope the
        // refetch above already re-primed the store; at global scope this view
        // is not the identity's view, so the store must be told (§5.1).
        if (level !== 'local' && /^user\./i.test(key)) invalidateEffectiveIdentity(repoId);
      } catch (e) {
        setFieldErrors((m) => ({ ...m, [key]: errorMessage(e) }));
      } finally {
        setBusyKey(null);
      }
    },
    [repoId, level, load],
  );

  const removeKey = useCallback(
    async (key: string) => {
      setBusyKey(key);
      try {
        await ipc.unsetConfig(repoId, level, key);
        await load(level, true);
        if (level !== 'local' && /^user\./i.test(key)) invalidateEffectiveIdentity(repoId);
      } catch (e) {
        setFieldErrors((m) => ({ ...m, [key]: errorMessage(e) }));
      } finally {
        setBusyKey(null);
      }
    },
    [repoId, level, load],
  );

  const setRunHooks = useCallback(
    (next: boolean) => {
      setHooksBusy(true);
      setHooksWriteError(null);
      setHooksDraft(next); // optimistic — reconciled by the refetch below
      void (async () => {
        try {
          // Always LOCAL: hooks are a per-repo concern, independent of the scope
          // switch (P59a).
          await ipc.setConfig(repoId, 'local', RUN_HOOKS_KEY, next ? 'true' : 'false');
        } catch (e) {
          setHooksWriteError(errorMessage(e));
        }
        await refreshLocal();
        setHooksDraft(null);
        setHooksBusy(false);
      })();
    },
    [repoId, refreshLocal],
  );

  const onDraftChange = useCallback(
    (k: string, v: string) => setDrafts((d) => ({ ...d, [k]: v })),
    [],
  );
  const onCommit = useCallback(
    (k: string, v: string, had: boolean) => void write(k, v, had),
    [write],
  );
  const reload = useCallback(async () => load(level, true), [load, level]);
  const removeKeyNow = useCallback((key: string) => void removeKey(key), [removeKey]);

  return {
    view,
    loading,
    loadError,
    drafts,
    busyKey,
    fieldErrors,
    // Until the first local read lands the row does not know its value, so it
    // renders git's default DISABLED rather than inviting a click that would
    // flip a setting the user cannot yet see (the toggle used to show a
    // confident ON over an unread — or unreadable — config).
    hooksLoading: localView === null,
    runHooks: hooksDraft ?? readRunHooks(localView),
    hooksBusy,
    hooksError: hooksWriteError ?? (localView === null ? localError : null),
    onDraftChange,
    onCommit,
    removeKey: removeKeyNow,
    reload,
    setRunHooks,
  };
}
