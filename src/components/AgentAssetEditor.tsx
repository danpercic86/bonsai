// P26c §7.2: the create/edit form for one agent asset (skill / subagent / slash
// command). Reads the asset on open (edit mode) or seeds a per-kind template
// (create mode), renders the kind's known frontmatter fields as single-line
// inputs + a body textarea, and round-trips a save/delete through ipc. Complex
// (multi-line YAML) assets open READ-ONLY with Save disabled — the editor never
// rewrites frontmatter it can't safely serialize. Rust owns all logic; this
// component only renders + calls ipc + confirms.

import { useEffect, useRef, useState } from 'react';
import { ipc } from '../ipc';
import type {
  AgentAsset,
  AgentAssetInput,
  AgentAssetInventory,
  AgentAssetKind,
  FrontmatterField,
} from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage, isAppError } from '../utils/errors';
import { ConfirmDialog } from './ConfirmDialog';

export interface AgentAssetEditorProps {
  repoId: string;
  kind: AgentAssetKind;
  /** null => create mode; a name => edit that existing asset. */
  name: string | null;
  onSaved(inventory: AgentAssetInventory): void;
  onClose(): void;
}

/** Known frontmatter keys per kind, in the order they render (contract §3.1). */
const KNOWN_KEYS: Record<AgentAssetKind, string[]> = {
  skill: ['name', 'description', 'argument-hint', 'allowed-tools', 'model', 'disable-model-invocation'],
  agent: ['name', 'description', 'tools', 'model'],
  command: ['description', 'argument-hint', 'allowed-tools', 'model', 'disable-model-invocation'],
};

/** Required keys per kind (contract §3.1). Only agents require any. */
const REQUIRED_KEYS: Record<AgentAssetKind, string[]> = {
  skill: [],
  agent: ['name', 'description'],
  command: [],
};

const KEY_LABELS: Record<string, string> = {
  name: 'Name',
  description: 'Description',
  'argument-hint': 'Argument hint',
  'allowed-tools': 'Allowed tools',
  model: 'Model',
  'disable-model-invocation': 'Disable model invocation',
  tools: 'Tools',
};

const KIND_NOUN: Record<AgentAssetKind, string> = {
  skill: 'skill',
  agent: 'subagent',
  command: 'command',
};

function bodyLabel(kind: AgentAssetKind): string {
  switch (kind) {
    case 'agent':
      return 'System prompt';
    case 'command':
      return 'Prompt template';
    case 'skill':
      return 'Instructions';
  }
}

/** A newline in a frontmatter value would corrupt the `---` block — replace any
 *  CR/LF with a single space (belt-and-suspenders; the inputs are single-line). */
function stripNewlines(value: string): string {
  return value.replace(/[\r\n]+/g, ' ');
}

/** True when the asset's frontmatter is complex (§4.3): multi-line YAML the flat
 *  editor can't safely round-trip. Keys off the structural `complex` flag the
 *  backend computes (authoritative — the backend also refuses to overwrite such a
 *  file); falls back to the legacy Error-message check for resilience. A merely-
 *  invalid asset (e.g. an agent missing `description`) stays editable. */
function isComplex(asset: AgentAsset): boolean {
  return (
    asset.complex ||
    asset.validation.issues.some(
      (i) => i.severity === 'error' && i.message.includes('multi-line YAML'),
    )
  );
}

interface UnknownField {
  uid: number;
  key: string;
  value: string;
}

interface Draft {
  name: string;
  /** Known-key -> current value. Keys not in KNOWN_KEYS live in `unknown`. */
  knownValues: Record<string, string>;
  /** Preserved unknown-but-present frontmatter keys (P26c preserves on save). */
  unknown: UnknownField[];
  body: string;
}

/** Per-kind create-mode template (contract §7.3). Frontmatter `name` mirrors the
 *  identity input for skill/agent; the body is a short, valid starter. */
function templateDraft(kind: AgentAssetKind): Draft {
  switch (kind) {
    case 'skill':
      return {
        name: '',
        knownValues: { name: '', description: '' },
        unknown: [],
        body: 'Describe what this skill does and when to use it.\n',
      };
    case 'agent':
      return {
        name: '',
        knownValues: { name: '', description: '', tools: '', model: 'inherit' },
        unknown: [],
        body: 'You are a specialized assistant. Describe the role and expected behavior here.\n',
      };
    case 'command':
      return {
        name: '',
        knownValues: { description: '', 'argument-hint': '' },
        unknown: [],
        body: 'Use $ARGUMENTS to describe what this command should do.\n',
      };
  }
}

/** Split a loaded asset's frontmatter into known values + preserved unknowns. */
function draftFromAsset(asset: AgentAsset, uid: () => number): Draft {
  const knownKeys = KNOWN_KEYS[asset.kind];
  const knownValues: Record<string, string> = {};
  const unknown: UnknownField[] = [];
  for (const field of asset.frontmatter) {
    if (knownKeys.includes(field.key) && !(field.key in knownValues)) {
      knownValues[field.key] = field.value;
    } else {
      unknown.push({ uid: uid(), key: field.key, value: field.value });
    }
  }
  return { name: asset.name, knownValues, unknown, body: asset.body };
}

export function AgentAssetEditor({ repoId, kind, name, onSaved, onClose }: AgentAssetEditorProps) {
  const pushToast = usePushToast();
  const creating = name === null;

  const nextUidRef = useRef(0);
  const uid = (): number => (nextUidRef.current += 1);

  // The last identity value auto-mirrored into the frontmatter `name` field, so a
  // user who overrides `name` manually keeps it while the identity keeps syncing.
  const autoNameRef = useRef('');

  const [draft, setDraft] = useState<Draft | null>(creating ? templateDraft(kind) : null);
  const [loadedAsset, setLoadedAsset] = useState<AgentAsset | null>(null);
  const [loading, setLoading] = useState(!creating);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteBusy, setDeleteBusy] = useState(false);

  const readOnly = loadedAsset !== null && isComplex(loadedAsset);

  // Load the existing asset (edit mode). Request id guards a stale resolve.
  const loadIdRef = useRef(0);
  useEffect(() => {
    if (creating || name === null) return;
    const id = (loadIdRef.current += 1);
    setLoading(true);
    setLoadError(null);
    void (async () => {
      try {
        const asset = await ipc.readAgentAsset(repoId, kind, name);
        if (loadIdRef.current !== id) return;
        setLoadedAsset(asset);
        setDraft(draftFromAsset(asset, uid));
      } catch (e) {
        if (loadIdRef.current !== id) return;
        setLoadError(errorMessage(e));
      } finally {
        if (loadIdRef.current === id) setLoading(false);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoId, kind, name, creating]);

  // Esc closes the editor first (capture + stopPropagation) so it does not also
  // close the parent AI-assets panel via App's global Esc handler.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      onClose();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [onClose]);

  const setIdentityName = (value: string): void => {
    setDraft((cur) => {
      if (cur === null) return cur;
      const next: Draft = { ...cur, name: value };
      // Mirror into frontmatter `name` (skill/agent) while the user hasn't
      // manually diverged it from the previous auto value.
      if (
        KNOWN_KEYS[kind].includes('name') &&
        (cur.knownValues.name ?? '') === autoNameRef.current
      ) {
        next.knownValues = { ...cur.knownValues, name: value };
        autoNameRef.current = value;
      }
      return next;
    });
  };

  const setKnownValue = (key: string, value: string): void => {
    setDraft((cur) =>
      cur === null ? cur : { ...cur, knownValues: { ...cur.knownValues, [key]: value } },
    );
  };

  const setUnknownKey = (target: UnknownField, key: string): void => {
    setDraft((cur) =>
      cur === null
        ? cur
        : { ...cur, unknown: cur.unknown.map((u) => (u.uid === target.uid ? { ...u, key } : u)) },
    );
  };

  const setUnknownValue = (target: UnknownField, value: string): void => {
    setDraft((cur) =>
      cur === null
        ? cur
        : { ...cur, unknown: cur.unknown.map((u) => (u.uid === target.uid ? { ...u, value } : u)) },
    );
  };

  const setBody = (value: string): void => {
    setDraft((cur) => (cur === null ? cur : { ...cur, body: value }));
  };

  const buildInput = (d: Draft): AgentAssetInput => {
    const frontmatter: FrontmatterField[] = [];
    // Known fields in table order; drop empty-value known fields (§7.2).
    for (const key of KNOWN_KEYS[kind]) {
      const raw = d.knownValues[key] ?? '';
      if (raw.trim() === '') continue;
      frontmatter.push({ key, value: stripNewlines(raw) });
    }
    // Preserve unknown fields as-is (the newline guard applies to key + value,
    // so neither can inject a line break that corrupts the `---` block).
    for (const u of d.unknown) {
      if (u.key.trim() === '') continue;
      frontmatter.push({ key: stripNewlines(u.key), value: stripNewlines(u.value) });
    }
    return { kind, name: d.name.trim(), frontmatter, body: d.body };
  };

  const save = async (): Promise<void> => {
    if (draft === null || readOnly) return;
    const input = buildInput(draft);
    setSaving(true);
    setFormError(null);
    try {
      const inv = await ipc.saveAgentAsset(repoId, input);
      pushToast('success', `Saved ${KIND_NOUN[kind]} '${input.name}'`);
      // The write always succeeds; surface any returned validation issues (§7.2).
      const saved = inv.assets.find((a) => a.kind === kind && a.name === input.name);
      if (saved !== undefined && !saved.validation.valid) {
        const count = saved.validation.issues.length;
        const first = saved.validation.issues[0]?.message ?? '';
        pushToast('info', `Saved with ${count} issue${count === 1 ? '' : 's'}: ${first}`);
      }
      onSaved(inv);
      onClose();
    } catch (e) {
      // invalidName surfaces inline; other errors toast (mirror ProfileManager).
      if (isAppError(e) && e.kind === 'invalidName') {
        setFormError(e.message);
      } else {
        pushToast('error', errorMessage(e));
      }
    } finally {
      setSaving(false);
    }
  };

  const confirmDelete = async (): Promise<void> => {
    if (name === null) return;
    setDeleteBusy(true);
    try {
      const inv = await ipc.deleteAgentAsset(repoId, kind, name);
      pushToast('success', `Deleted ${KIND_NOUN[kind]} '${name}'`);
      onSaved(inv);
      setDeleteOpen(false);
      onClose();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setDeleteBusy(false);
    }
  };

  const knownKeys = KNOWN_KEYS[kind];
  const requiredKeys = REQUIRED_KEYS[kind];
  const title = creating ? `New ${KIND_NOUN[kind]}` : `Edit ${KIND_NOUN[kind]} “${name}”`;

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dialog-card agent-editor-card" role="dialog" aria-label={title}>
        <div className="shortcut-header">
          <h2 className="dialog-title shortcut-title">{title}</h2>
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

        {loadError !== null && (
          <div className="error-banner" role="alert">
            {loadError}
          </div>
        )}

        {loading ? (
          <p className="settings-ai-status">Loading…</p>
        ) : draft === null ? null : (
          <>
            {readOnly && (
              <div className="asset-readonly-banner" role="alert">
                This asset has complex YAML frontmatter Bonsai can’t safely edit yet — edit it in your
                editor.
              </div>
            )}

            {!creating && loadedAsset !== null && loadedAsset.validation.issues.length > 0 && (
              <ul className="asset-issue-list">
                {loadedAsset.validation.issues.map((issue, i) => (
                  <li
                    key={i}
                    className={
                      issue.severity === 'error'
                        ? 'asset-issue asset-issue-error'
                        : 'asset-issue asset-issue-warning'
                    }
                  >
                    {issue.message}
                  </li>
                ))}
              </ul>
            )}

            <label className="dialog-label">
              Name
              <input
                type="text"
                className="dialog-input"
                value={draft.name}
                disabled={!creating}
                placeholder={kind === 'skill' ? 'my-skill' : 'my-name'}
                onChange={(e) => setIdentityName(e.target.value)}
              />
              {!creating && (
                <span className="asset-field-hint">
                  Renaming an asset means creating a new one — edit the name by creating a fresh asset.
                </span>
              )}
            </label>

            <div className="asset-editor-fields">
              {knownKeys.map((key) => (
                <label className="dialog-label" key={key}>
                  {KEY_LABELS[key] ?? key}
                  {requiredKeys.includes(key) ? ' (required)' : ' (optional)'}
                  <input
                    type="text"
                    className="dialog-input"
                    value={draft.knownValues[key] ?? ''}
                    disabled={readOnly}
                    onChange={(e) => setKnownValue(key, e.target.value)}
                  />
                </label>
              ))}
            </div>

            {draft.unknown.length > 0 && (
              <div className="asset-editor-unknown">
                <span className="settings-control-label">Other frontmatter (preserved)</span>
                {draft.unknown.map((u) => (
                  <div className="asset-unknown-row" key={u.uid}>
                    <input
                      type="text"
                      className="dialog-input asset-unknown-key"
                      value={u.key}
                      disabled={readOnly}
                      onChange={(e) => setUnknownKey(u, e.target.value)}
                    />
                    <input
                      type="text"
                      className="dialog-input asset-unknown-value"
                      value={u.value}
                      disabled={readOnly}
                      onChange={(e) => setUnknownValue(u, e.target.value)}
                    />
                  </div>
                ))}
              </div>
            )}

            <label className="dialog-label">
              {bodyLabel(kind)}
              <textarea
                className="dialog-input dialog-textarea asset-editor-body"
                rows={14}
                value={draft.body}
                disabled={readOnly}
                onChange={(e) => setBody(e.target.value)}
              />
            </label>

            {formError !== null && <p className="dialog-error">{formError}</p>}

            <div className="dialog-buttons agent-editor-buttons">
              {!creating && (
                <button
                  type="button"
                  className="btn-danger agent-editor-delete"
                  disabled={saving || deleteBusy}
                  onClick={() => setDeleteOpen(true)}
                >
                  Delete
                </button>
              )}
              <button type="button" className="btn-secondary" disabled={saving} onClick={onClose}>
                Cancel
              </button>
              <button
                type="button"
                className="btn-primary"
                disabled={saving || readOnly}
                title={readOnly ? 'Complex frontmatter is read-only' : undefined}
                onClick={() => void save()}
              >
                {saving ? 'Saving…' : 'Save'}
              </button>
            </div>
          </>
        )}
      </div>

      <ConfirmDialog
        open={deleteOpen}
        title={`Delete ${KIND_NOUN[kind]}?`}
        confirmLabel="Delete"
        busy={deleteBusy}
        onConfirm={() => void confirmDelete()}
        onCancel={() => setDeleteOpen(false)}
      >
        {kind === 'skill' ? (
          <div>
            Delete skill <span className="mono">{name}</span>? This permanently removes the entire{' '}
            <span className="mono">.claude/skills/{name}/</span> directory and every file inside it
            (SKILL.md plus any supporting scripts, templates, or references).
          </div>
        ) : (
          <div>
            Delete {KIND_NOUN[kind]} <span className="mono">{name}</span>? This permanently removes{' '}
            <span className="mono">
              {kind === 'agent' ? `.claude/agents/${name}.md` : `.claude/commands/${name}.md`}
            </span>
            .
          </div>
        )}
      </ConfirmDialog>
    </div>
  );
}
