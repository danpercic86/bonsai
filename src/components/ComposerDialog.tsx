import type { FileStatus } from '../ipc';
import { DiffView } from './DiffView';
import { ComposerGroupCard } from './ComposerGroupCard';
import type { MoveOption } from './ComposerGroupCard';
import type { UseCommitComposer } from './repoWorkspace/useCommitComposer';

// P54c: the commit-composer REVIEW overlay (generate → review → accept/edit).
// This is NOT AiOutputPanel (that panel is read-only prose): the proposal is a
// fully-editable plan the user reassigns / edits / drops / merges before the
// explicit "Create N commits" confirm. Presentational container — all state and
// behavior arrive via the `composer` hook; the only extra input is the working-
// dir status map for the per-file badges.

function Skeleton() {
  return (
    <div className="skeleton-group" aria-hidden="true">
      {Array.from({ length: 5 }, (_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}

function groupLabel(i: number): string {
  return `Commit ${i + 1}`;
}

export interface ComposerDialogProps {
  composer: UseCommitComposer;
  /** Status badge per changed path (from the working-dir snapshot). */
  statusByPath: Map<string, FileStatus>;
}

export function ComposerDialog({ composer, statusByPath }: ComposerDialogProps) {
  const {
    loading,
    error,
    notes,
    groups,
    unassigned,
    applying,
    canApply,
    close,
    apply,
    editMessage,
    moveFile,
    addGroup,
    dropGroup,
    mergeInto,
    preview,
    previewFile,
    closePreview,
  } = composer;

  // A hard propose failure (e.g. ?ai=off) leaves no plan at all → full banner.
  // An apply failure keeps the plan visible with an inline banner so the user
  // can adjust and retry.
  const proposeFailed = !loading && error !== null && groups.length === 0 && unassigned.length === 0;
  const createLabel = applying
    ? 'Creating…'
    : `Create ${groups.length} commit${groups.length === 1 ? '' : 's'}`;

  // Move-to targets for a group card at index `i`: every OTHER group + Unassigned.
  const groupMoveTargets = (i: number): MoveOption[] => [
    ...groups.map((_, j) => ({ value: j, label: groupLabel(j) })).filter((_, j) => j !== i),
    { value: 'unassigned' as const, label: 'Unassigned' },
  ];
  // The unassigned bucket can only push files INTO groups.
  const unassignedMoveTargets: MoveOption[] = groups.map((_, j) => ({
    value: j,
    label: groupLabel(j),
  }));

  return (
    <div className="composer-overlay" role="dialog" aria-modal="true" aria-label="Compose commits">
      <div className="composer-dialog">
        <header className="composer-header">
          <span className="composer-icon" aria-hidden="true">
            {'✨'}
          </span>
          <span className="composer-heading">Compose commits</span>
          <button
            type="button"
            className="btn-icon composer-close"
            aria-label="Close composer"
            title="Close (Esc)"
            disabled={applying}
            onClick={close}
          >
            {'×'}
          </button>
        </header>

        <div className="composer-note" role="note">
          <span className="composer-note-line">
            Compose manages staging — on Create your index is reset to HEAD and each commit is
            staged in turn; your working tree is never touched.
          </span>
          {notes.map((n, i) => (
            <span key={i} className="composer-note-line composer-note-normalizer">
              {n}
            </span>
          ))}
          {unassigned.length > 0 && (
            <span className="composer-note-line composer-note-uncommitted">
              {unassigned.length} file{unassigned.length === 1 ? '' : 's'} will be left uncommitted.
            </span>
          )}
        </div>

        <div className="composer-body">
          {loading ? (
            <Skeleton />
          ) : proposeFailed ? (
            <div className="error-banner error-banner-dismissible" role="alert">
              <span className="error-banner-text">{error}</span>
            </div>
          ) : (
            <>
              {error !== null && (
                <div className="error-banner error-banner-dismissible" role="alert">
                  <span className="error-banner-text">{error}</span>
                </div>
              )}
              {groups.map((g, i) => (
                <ComposerGroupCard
                  key={i}
                  variant="group"
                  title={groupLabel(i)}
                  message={g.message}
                  files={g.files}
                  statusByPath={statusByPath}
                  moveTargets={groupMoveTargets(i)}
                  canMergeIntoNext={i < groups.length - 1}
                  onEditMessage={(m) => editMessage(i, m)}
                  onMoveFile={(path, target) => moveFile(path, target)}
                  onRemoveFile={(path) => moveFile(path, 'unassigned')}
                  onPreviewFile={previewFile}
                  onDropGroup={() => dropGroup(i)}
                  onMergeIntoNext={() => mergeInto(i, i + 1)}
                />
              ))}
              <ComposerGroupCard
                variant="unassigned"
                title="Unassigned"
                message=""
                files={unassigned}
                statusByPath={statusByPath}
                moveTargets={unassignedMoveTargets}
                canMergeIntoNext={false}
                onEditMessage={() => {}}
                onMoveFile={(path, target) => moveFile(path, target)}
                onRemoveFile={() => {}}
                onPreviewFile={previewFile}
                onDropGroup={() => {}}
                onMergeIntoNext={() => {}}
              />
              <button type="button" className="btn-secondary composer-add-group" onClick={addGroup}>
                + New group
              </button>
            </>
          )}

          {preview !== null && (
            <div className="composer-preview" role="region" aria-label={`Preview ${preview.path}`}>
              <div className="composer-preview-head">
                <span className="composer-preview-path mono" title={preview.path}>
                  {preview.path}
                </span>
                <button
                  type="button"
                  className="btn-icon composer-preview-close"
                  aria-label="Close preview"
                  title="Close preview"
                  onClick={closePreview}
                >
                  {'×'}
                </button>
              </div>
              <div className="composer-preview-body">
                {preview.loading ? (
                  <Skeleton />
                ) : preview.error !== null ? (
                  <div className="error-banner error-banner-dismissible" role="alert">
                    <span className="error-banner-text">{preview.error}</span>
                  </div>
                ) : preview.diff !== null ? (
                  <DiffView diff={preview.diff} />
                ) : null}
              </div>
            </div>
          )}
        </div>

        <footer className="composer-footer">
          <button type="button" className="btn-secondary" disabled={applying} onClick={close}>
            Cancel
          </button>
          <button
            type="button"
            className="btn-primary composer-create"
            disabled={!canApply || applying}
            onClick={() => void apply()}
          >
            {createLabel}
          </button>
        </footer>
      </div>
    </div>
  );
}
