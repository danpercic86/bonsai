// P24d §8.1: the AI-asset inventory + drift overlay. Fetches the inventory and
// the profile store on open / repoId change / repo-changed, renders the drift
// badge, the managed instruction-file rows (with a click-through current-vs-
// canonical compare), the detected (unmanaged) rows, and hosts the
// ProfileManager. Rust owns all logic; this component only renders + calls ipc.

import { useCallback, useEffect, useRef, useState } from 'react';
import { ipc } from '../ipc';
import type {
  AiAsset,
  AiAssetInventory,
  DriftEntry,
  ProfileActivation,
  ProfileStore,
  Unsubscribe,
} from '../ipc';
import { errorMessage } from '../utils/errors';
import { ProfileManager } from './ProfileManager';
import { TextComparePane } from './ProfileActivateDialog';

export interface AiAssetsPanelProps {
  open: boolean;
  onClose(): void;
  repoId: string;
}

interface CompareState {
  label: string;
  assetPath: string;
  canonicalPath: string;
  left: string | null;
  right: string | null;
  loading: boolean;
  error: string | null;
}

/** The sync chip for one managed instruction file (§8.1). */
function syncChip(entry: DriftEntry, canonicalId: string | null) {
  if (entry.assetId === canonicalId) {
    return <span className="asset-chip asset-chip-canonical">canonical</span>;
  }
  if (!entry.exists) return <span className="asset-chip asset-chip-missing">missing</span>;
  if (entry.inSync) return <span className="asset-chip asset-chip-sync">in sync</span>;
  return <span className="asset-chip asset-chip-drifted">drifted</span>;
}

export function AiAssetsPanel({ open, onClose, repoId }: AiAssetsPanelProps) {
  const [inventory, setInventory] = useState<AiAssetInventory | null>(null);
  const [profiles, setProfiles] = useState<ProfileStore | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [compare, setCompare] = useState<CompareState | null>(null);

  // Monotonic request id: a fetch whose id no longer matches the latest issued
  // one is stale (repoId changed via Ctrl+Tab while the panel was open, or a
  // newer Refresh superseded it) and must NOT write state. Same discipline as
  // ProfileActivateDialog's cancelled flag.
  const fetchIdRef = useRef(0);

  const refresh = useCallback(async (): Promise<void> => {
    const id = (fetchIdRef.current += 1);
    setLoading(true);
    setError(null);
    try {
      const [inv, store] = await Promise.all([
        ipc.listAiAssets(repoId),
        ipc.listProfiles(repoId),
      ]);
      if (fetchIdRef.current !== id) return;
      setInventory(inv);
      setProfiles(store);
    } catch (e) {
      if (fetchIdRef.current !== id) return;
      setError(errorMessage(e));
    } finally {
      if (fetchIdRef.current === id) setLoading(false);
    }
  }, [repoId]);

  // Fetch on open + whenever the repo changes while open.
  useEffect(() => {
    if (open) void refresh();
  }, [open, refresh]);

  // Auto-refresh on the repo-changed event for THIS repo (§8.1) — a profile
  // write or an external edit re-drives the drift view for free.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];
    void (async () => {
      const off = await ipc.onRepoChanged((p) => {
        if (p.repoId === repoId) void refresh();
      });
      if (cancelled) {
        off();
        return;
      }
      unsubs.push(off);
    })();
    return () => {
      cancelled = true;
      for (const unsub of unsubs) unsub();
    };
  }, [open, repoId, refresh]);

  // Activation refreshes both slices: profiles from the returned store, then
  // the inventory so the drift chips reflect the just-written files.
  const handleActivated = useCallback(
    (activation: ProfileActivation): void => {
      setProfiles(activation.store);
      void refresh();
    },
    [refresh],
  );

  // Esc closes the compare sub-overlay first (capture + stopPropagation) so it
  // does not also close the whole panel via App's global Esc handler.
  useEffect(() => {
    if (compare === null) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      setCompare(null);
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [compare]);

  // Compare reads carry their own request id so a repoId change (or a second
  // row click) mid-read can never populate the pane with a stale file body.
  const compareIdRef = useRef(0);

  const openCompare = useCallback(
    async (asset: AiAsset, canonical: AiAsset): Promise<void> => {
      const id = (compareIdRef.current += 1);
      setCompare({
        label: asset.label,
        assetPath: asset.path,
        canonicalPath: canonical.path,
        left: null,
        right: null,
        loading: true,
        error: null,
      });
      try {
        const [cur, can] = await Promise.all([
          ipc.readAiAsset(repoId, asset.path),
          ipc.readAiAsset(repoId, canonical.path),
        ]);
        setCompare((prev) =>
          prev === null || compareIdRef.current !== id
            ? prev
            : { ...prev, left: cur.content, right: can.content, loading: false },
        );
      } catch (e) {
        setCompare((prev) =>
          prev === null || compareIdRef.current !== id
            ? prev
            : { ...prev, loading: false, error: errorMessage(e) },
        );
      }
    },
    [repoId],
  );

  if (!open) return null;

  const drift = inventory?.drift ?? null;
  const canonicalId = drift?.canonicalId ?? null;
  const canonicalAsset =
    canonicalId !== null ? inventory?.assets.find((a) => a.id === canonicalId) ?? null : null;
  const driftedCount = drift?.entries.filter((e) => e.exists && !e.inSync).length ?? 0;

  // Detected = unmanaged OR not a single-file (rules-dirs, .mcp.json, .claude/).
  const detected =
    inventory?.assets.filter((a) => !a.managed || a.kind !== 'singleFile') ?? [];

  const assetById = (id: string): AiAsset | undefined =>
    inventory?.assets.find((a) => a.id === id);

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dialog-card ai-assets-card" role="dialog" aria-label="AI Assets">
        <div className="shortcut-header">
          <h2 className="dialog-title shortcut-title">AI Assets</h2>
          <div className="asset-header-actions">
            <button
              type="button"
              className="btn-secondary settings-toggle-btn"
              disabled={loading}
              onClick={() => void refresh()}
            >
              {loading ? 'Refreshing…' : 'Refresh'}
            </button>
            <button
              type="button"
              className="btn-icon shortcut-close"
              aria-label="Close"
              title="Close"
              onClick={onClose}
            >
              {'×'}
            </button>
          </div>
        </div>

        {error !== null && (
          <div className="error-banner" role="alert">
            {error}
          </div>
        )}

        {inventory === null ? (
          <p className="settings-ai-status">Loading AI assets…</p>
        ) : (
          <>
            {/* --- Drift badge --- */}
            <div className="asset-drift-badge-row">
              {drift?.inSync ? (
                <span className="asset-badge asset-badge-ok">In sync</span>
              ) : (
                <span className="asset-badge asset-badge-warn">
                  {driftedCount} file{driftedCount === 1 ? '' : 's'} drifted
                </span>
              )}
            </div>

            {/* --- Managed instruction files --- */}
            <section className="settings-section">
              <h3 className="settings-section-title">Managed instruction files</h3>
              <p className="settings-section-desc">
                Compared against the canonical reference. Click a drifted file to see how it differs.
              </p>
              <ul className="asset-list">
                {(drift?.entries ?? []).map((entry) => {
                  const asset = assetById(entry.assetId);
                  const label = asset?.label ?? entry.assetId;
                  const path = asset?.path ?? entry.assetId;
                  const clickable =
                    entry.exists &&
                    entry.assetId !== canonicalId &&
                    canonicalAsset !== null &&
                    asset !== undefined;
                  const body = (
                    <>
                      <div className="asset-row-main">
                        <div className="asset-row-head">
                          <span className="asset-row-label">{label}</span>
                          {syncChip(entry, canonicalId)}
                        </div>
                        <span className="asset-row-path mono">{path}</span>
                      </div>
                    </>
                  );
                  return clickable ? (
                    <li key={entry.assetId}>
                      <button
                        type="button"
                        className="asset-row asset-row-clickable"
                        onClick={() => void openCompare(asset, canonicalAsset)}
                      >
                        {body}
                      </button>
                    </li>
                  ) : (
                    <li className="asset-row" key={entry.assetId}>
                      {body}
                    </li>
                  );
                })}
              </ul>
            </section>

            {/* --- Detected (not managed) --- */}
            <section className="settings-section">
              <h3 className="settings-section-title">Detected (not managed)</h3>
              <p className="settings-section-desc">
                Inventoried for context — managed in a later release.
              </p>
              {detected.length === 0 ? (
                <p className="settings-ai-status">None detected.</p>
              ) : (
                <ul className="asset-list">
                  {detected.map((asset) => (
                    <li className="asset-row" key={asset.id}>
                      <div className="asset-row-main">
                        <div className="asset-row-head">
                          <span className="asset-row-label">{asset.label}</span>
                          {!asset.exists && (
                            <span className="asset-chip asset-chip-missing">missing</span>
                          )}
                          {asset.kind === 'rulesDir' && asset.exists && (
                            <span className="asset-chip asset-chip-muted">
                              {asset.files.length} file{asset.files.length === 1 ? '' : 's'}
                            </span>
                          )}
                        </div>
                        <span className="asset-row-path mono">{asset.path}</span>
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </section>

            {/* --- Context profiles --- */}
            {profiles !== null && (
              <ProfileManager
                repoId={repoId}
                store={profiles}
                inventory={inventory}
                onStoreChange={setProfiles}
                onActivated={handleActivated}
              />
            )}
          </>
        )}
      </div>

      {/* Read-only current-vs-canonical compare for a drifted row. */}
      {compare !== null && (
        <div
          className="dialog-overlay"
          onMouseDown={(e) => {
            if (e.target === e.currentTarget) setCompare(null);
          }}
        >
          <div className="dialog-card ai-assets-card" role="dialog" aria-label={`Compare ${compare.label}`}>
            <div className="shortcut-header">
              <h2 className="dialog-title shortcut-title">{compare.label} vs canonical</h2>
              <button
                type="button"
                className="btn-icon shortcut-close"
                aria-label="Close"
                title="Close"
                onClick={() => setCompare(null)}
              >
                {'×'}
              </button>
            </div>
            {compare.error !== null ? (
              <div className="error-banner" role="alert">
                {compare.error}
              </div>
            ) : compare.loading ? (
              <p className="settings-ai-status">Loading files…</p>
            ) : (
              <TextComparePane
                leftLabel={compare.assetPath}
                rightLabel={`${compare.canonicalPath} (canonical)`}
                left={compare.left}
                right={compare.right}
              />
            )}
          </div>
        </div>
      )}
    </div>
  );
}
