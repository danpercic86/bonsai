// P59a: repo-scoped "Run git hooks" toggle. Bound to `bonsai.runHooks` in the
// repo's LOCAL git config via the EXISTING getConfig / setConfig commands (no
// new command). Unset ⇒ ON (git's default); only an explicit `false` disables.
// Independent of the Git-config section's Local|Global level selector — hooks
// are always a per-repo (Local) concern.

import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../ipc';
import { errorMessage } from '../utils/errors';

export interface SettingsHooksToggleProps {
  /** Open repo id (== workdir path). */
  repoId: string;
}

const RUN_HOOKS_KEY = 'bonsai.runHooks';

export function SettingsHooksToggle({ repoId }: SettingsHooksToggleProps) {
  const [enabled, setEnabled] = useState(true);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqId = useRef(0);

  const load = useCallback(async () => {
    const id = ++reqId.current;
    setLoading(true);
    setError(null);
    try {
      const view = await ipc.getConfig(repoId, 'local');
      if (id !== reqId.current) return;
      const entry = view.advanced.find((a) => a.name.toLowerCase() === RUN_HOOKS_KEY.toLowerCase());
      // Unset ⇒ ON (git default); only an explicit `false` disables.
      setEnabled(entry === undefined || entry.value.trim().toLowerCase() !== 'false');
    } catch (e) {
      if (id !== reqId.current) return;
      setError(errorMessage(e));
    } finally {
      if (id === reqId.current) setLoading(false);
    }
  }, [repoId]);

  // Reads on repo-change (contract §A0 A-D3).
  useEffect(() => {
    void load();
  }, [load]);

  const toggle = useCallback(
    async (next: boolean) => {
      setBusy(true);
      setError(null);
      setEnabled(next); // optimistic — reconciled by the reload below
      try {
        await ipc.setConfig(repoId, 'local', RUN_HOOKS_KEY, next ? 'true' : 'false');
        await load();
      } catch (e) {
        setError(errorMessage(e));
        await load();
      } finally {
        setBusy(false);
      }
    },
    [repoId, load],
  );

  return (
    <div className="settings-config-group">
      <h4 className="settings-config-subtitle">Hooks</h4>
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={enabled}
          disabled={loading || busy}
          onChange={(e) => void toggle(e.target.checked)}
        />
        <span>Run git hooks for this repository</span>
      </label>
      <p className="settings-config-hint">
        When off, commits run with <span className="mono">--no-verify</span> and{' '}
        <span className="mono">bonsai.runHooks=false</span> is written to this repo. Unset means
        hooks run (git’s default).
      </p>
      {error !== null && <p className="settings-config-error">{error}</p>}
    </div>
  );
}
