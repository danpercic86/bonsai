// P40b §7.1: the "Git config" pane body. P69h re-skins it onto the settings
// primitives (`SettingsGroup` / `SettingsRow` / `SettingsSwitch`) and moves its
// IPC + form state into `settings/useGitConfigEditor.ts`, so this file is a view:
// Hooks, Identity, and the collapsed Advanced block (`settings/GitConfigAdvanced`).
//
// Two things deliberately stayed here:
//   * the Identity sub-section — the `configMissing` deep link scrolls to it and
//     focuses its `user.name` input, and that effect lives where the refs live
//     (its `focusedOnce` guard is untouched);
//   * nothing else. The scope (Local | Global) switch moved to the pane header
//     (UI §1.1) and reaches this file through `GitConfigScopeContext`, which
//     falls back to `local` when no provider is above — so a bare render behaves
//     exactly as before.

import { useEffect, useRef } from 'react';

import type { CuratedConfigEntry } from '../ipc';
import { SettingsHooksToggle } from './SettingsHooksToggle';
import { CuratedConfigControl } from './settings/CuratedConfigControl';
import { GitConfigAdvanced } from './settings/GitConfigAdvanced';
import { useGitConfigScope } from './settings/GitConfigScopeContext';
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';
import { settingsRowHelpId } from './settings/settingsCatalog';
import { useGitConfigEditor } from './settings/useGitConfigEditor';

export interface SettingsGitConfigSectionProps {
  /** Open repo id (== workdir path). Never null: `GitConfigCategory` renders
   *  `SettingsEmpty` for the no-repo case (UI §1.2), so the type rules out the
   *  branch rather than this file carrying a dead one. */
  repoId: string;
  /** 'identity' → scroll/focus the Identity sub-section on mount (commit-error
   *  linkage). */
  initialFocus?: 'identity' | null;
}

const IDENTITY_KEYS = ['user.name', 'user.email'];

/** Catalog row id for a curated identity key. */
const IDENTITY_ROW: Record<string, string> = {
  'user.name': 'git-config.user-name',
  'user.email': 'git-config.user-email',
};

export function SettingsGitConfigSection({ repoId, initialFocus }: SettingsGitConfigSectionProps) {
  const { level } = useGitConfigScope();
  const editor = useGitConfigEditor(repoId, level);
  const { view, drafts, busyKey, fieldErrors, onDraftChange, onCommit } = editor;

  const identityRef = useRef<HTMLElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);
  const focusedOnce = useRef(false);

  // Commit-error linkage: scroll/focus the Identity sub-section once when opened
  // with initialFocus === 'identity' and the view is ready.
  useEffect(() => {
    if (initialFocus !== 'identity' || view === null || focusedOnce.current) return;
    focusedOnce.current = true;
    identityRef.current?.scrollIntoView({ block: 'center' });
    nameInputRef.current?.focus();
  }, [initialFocus, view]);

  const curated = view?.curated ?? [];
  const behaviourKeys = curated.filter((c) => !IDENTITY_KEYS.includes(c.key));

  const renderIdentity = (entry: CuratedConfigEntry, inputRef?: React.Ref<HTMLInputElement>) => (
    <SettingsRow
      key={entry.key}
      id={IDENTITY_ROW[entry.key]}
      controlId={`cfg-${entry.key}`}
      stacked
    >
      {/* The row already owns the `<label for>`, which IS the control's
          accessible name — a second label here would append to it. */}
      <CuratedConfigControl
        entry={entry}
        draft={drafts[entry.key] ?? ''}
        busy={busyKey === entry.key}
        error={fieldErrors[entry.key]}
        inputRef={inputRef}
        labelled={false}
        describedBy={settingsRowHelpId(IDENTITY_ROW[entry.key])}
        onDraftChange={onDraftChange}
        onCommit={onCommit}
      />
    </SettingsRow>
  );
  const nameEntry = curated.find((c) => c.key === 'user.name');
  const emailEntry = curated.find((c) => c.key === 'user.email');

  return (
    <>
      {/* P59a: repo-scoped "Run git hooks" toggle (always Local, whatever the
          scope switch says — served from the same read as the form below). */}
      <SettingsHooksToggle
        enabled={editor.runHooks}
        loading={editor.hooksLoading}
        busy={editor.hooksBusy}
        error={editor.hooksError}
        onToggle={editor.setRunHooks}
      />

      {editor.loadError !== null ? (
        <p className="settings-ai-status settings-ai-status-warn" role="note">
          {editor.loadError}
        </p>
      ) : editor.loading && view === null ? (
        <p className="settings-ai-status">Loading config…</p>
      ) : (
        <>
          {/* --- Identity (stays here: the deep-link focus effect owns these refs) --- */}
          <SettingsGroup id="git-config-identity" title="Identity" innerRef={identityRef}>
            {nameEntry !== undefined && renderIdentity(nameEntry, nameInputRef)}
            {emailEntry !== undefined && renderIdentity(emailEntry)}
          </SettingsGroup>

          <GitConfigAdvanced
            repoId={repoId}
            level={level}
            behaviourKeys={behaviourKeys}
            advanced={view?.advanced ?? []}
            drafts={drafts}
            busyKey={busyKey}
            fieldErrors={fieldErrors}
            onDraftChange={onDraftChange}
            onCommit={onCommit}
            onRemove={editor.removeKey}
            onReload={editor.reload}
          />
        </>
      )}
    </>
  );
}
