import { useEffect, useRef, useState } from 'react';
import { EditorView } from '@codemirror/view';
import { EditorState, Compartment } from '@codemirror/state';
import { MergeView } from '@codemirror/merge';
import type { ConflictFile } from '../ipc';
import { hasUnresolvedMarkers } from '../utils/conflictRegions';
import {
  cmTheme,
  editableExtensions,
  readonlyExtensions,
  loadLanguageExtension,
  readTheme,
} from './conflictCmSetup';
import { conflictSelfTest } from './conflictSelfTest';

// P12 §2.3: unified editable conflict-resolution editor. Renders the worktree
// text (markers included) in a CodeMirror doc; edits sync to one React-owned
// result string; Stage-resolved gates on `hasUnresolvedMarkers`. Per-region
// widgets (P12c) and side-by-side (P12d) extend this shell — props are stable.

export interface ConflictEditorProps {
  /** The conflicted file (already fetched by RepoWorkspace's fetchConflictSlot).
   *  Guaranteed kind ∈ {bothModified, bothAdded} and !binary && !tooLarge &&
   *  !missing by the mount guard (§5); other kinds never reach this component. */
  file: ConflictFile;
  /** Stage the given resolved text (RepoWorkspace → ipc.resolveConflictText →
   *  refreshAll). Rejects on backend error; the editor shows it inline. */
  onResolve(path: string, content: string): Promise<void>;
  /** Close the editor without staging (collapse the slot). */
  onCancel(): void;
  /** Busy flag (RepoWorkspace `mutating`) — disables Save while a mutation runs. */
  mutating: boolean;
}

// ---- component ----------------------------------------------------------

type Mode = 'unified' | 'split';

export function ConflictEditor({ file, onResolve, onCancel, mutating }: ConflictEditorProps) {
  // One React-owned result string, seeded ONCE per file.path (a new path
  // re-seeds — see the reseed effect below). This is the shared result doc
  // (§0.6) that P12d reads/reseeds across mode toggles.
  const [result, setResult] = useState<string>(file.text);
  const [saveError, setSaveError] = useState<string | null>(null);
  // View mode: unified single editable doc, or side-by-side ours | result.
  const [mode, setMode] = useState<Mode>('unified');

  const hostRef = useRef<HTMLDivElement | null>(null);
  // Exactly ONE of these is non-null at a time (unified EditorView OR split
  // MergeView) — enforced by the view-creation effect below.
  const viewRef = useRef<EditorView | null>(null);
  const mergeRef = useRef<MergeView | null>(null);
  // Latest result kept in a ref so the view-creation effect can seed without
  // re-running when `result` changes (edits flow through the doc, not remounts).
  const resultRef = useRef(result);
  resultRef.current = result;

  // Re-seed when the file identity changes (new conflict opened in the slot).
  // Runs BEFORE the view-creation effect below, so it updates `resultRef`
  // synchronously and the freshly-created view seeds from the new file's text.
  const seededPathRef = useRef(file.path);
  useEffect(() => {
    if (seededPathRef.current === file.path) return;
    seededPathRef.current = file.path;
    resultRef.current = file.text;
    setResult(file.text);
    setSaveError(null);
  }, [file.path, file.text]);

  // Register the conflictSelfTest hook (mock/dev only) via a NON-destructive
  // merge — GraphCanvas owns window.__bonsai's lifecycle (§2.2).
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    // Non-destructive merge — GraphCanvas owns __bonsai's lifecycle (§2.2). Cast
    // because the object is built incrementally (scrollSweep is set by
    // GraphCanvas, which mounts before this overlay).
    window.__bonsai = {
      ...(window.__bonsai ?? {}),
      conflictSelfTest,
    } as typeof window.__bonsai;
    return () => {
      if (window.__bonsai?.conflictSelfTest === conflictSelfTest) {
        delete window.__bonsai.conflictSelfTest;
      }
    };
  }, []);

  // Create the active view (unified EditorView OR split MergeView). Recreated
  // ONLY on mount, mode change, or file.path change — NEVER on keystrokes; edits
  // are fed through the doc via `updateListener`, so cursor/undo history survive.
  useEffect(() => {
    const host = hostRef.current;
    if (host === null) return;

    let disposed = false;

    // Shared doc->React sync used by the unified doc and the split `b` editor.
    const updateListener = EditorView.updateListener.of((update) => {
      if (!update.docChanged) return;
      const next = update.state.doc.toString();
      // Guard the feedback loop: only setState when the string truly differs.
      if (next !== resultRef.current) {
        resultRef.current = next;
        setResult(next);
      }
    });

    // {view, compartment} pairs whose language compartment is reconfigured once
    // the grammar finishes loading (both panes in split mode).
    const langTargets: Array<{ view: EditorView; comp: Compartment }> = [];
    let applyTheme: () => void;

    if (mode === 'unified') {
      const theme = new Compartment();
      const lang = new Compartment();
      const view = new EditorView({
        parent: host,
        state: EditorState.create({
          doc: resultRef.current,
          extensions: editableExtensions(theme, lang, updateListener),
        }),
      });
      viewRef.current = view;
      mergeRef.current = null;
      langTargets.push({ view, comp: lang });
      applyTheme = () => view.dispatch({ effects: theme.reconfigure(cmTheme(readTheme())) });
    } else {
      const themeA = new Compartment();
      const langA = new Compartment();
      const themeB = new Compartment();
      const langB = new Compartment();
      const merge = new MergeView({
        parent: host,
        // a = OURS (read-only, left). b = the SHARED editable result (right),
        // seeded from the live result string so a mode switch keeps edits.
        a: { doc: file.ours, extensions: readonlyExtensions(themeA, langA) },
        b: {
          doc: resultRef.current,
          extensions: editableExtensions(themeB, langB, updateListener),
        },
        // Chunk-accept arrows copy ours (a) into the result (b) — complementary
        // to the region toolbar; both mutate the one `b` doc.
        revertControls: 'a-to-b',
        highlightChanges: true,
        gutter: true,
      });
      mergeRef.current = merge;
      viewRef.current = null;
      langTargets.push({ view: merge.a, comp: langA }, { view: merge.b, comp: langB });
      applyTheme = () => {
        merge.a.dispatch({ effects: themeA.reconfigure(cmTheme(readTheme())) });
        merge.b.dispatch({ effects: themeB.reconfigure(cmTheme(readTheme())) });
      };
    }

    // Track app theme changes (data-theme on <html>) and reconfigure.
    const observer = new MutationObserver(() => applyTheme());
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    });

    // Lazy syntax highlighting: resolve + load the grammar, then reconfigure each
    // mounted view's language compartment. Guarded via `disposed` against a mode
    // or file switch (or unmount) that lands before the async load resolves.
    void loadLanguageExtension(file.path)
      .then((ext) => {
        if (disposed || ext === null) return;
        for (const target of langTargets) {
          target.view.dispatch({ effects: target.comp.reconfigure(ext) });
        }
      })
      .catch(() => {
        // Non-fatal: a failed grammar load just falls back to no syntax
        // highlighting — the editor itself is unaffected.
      });

    return () => {
      disposed = true;
      observer.disconnect();
      viewRef.current?.destroy();
      mergeRef.current?.destroy();
      viewRef.current = null;
      mergeRef.current = null;
    };
  }, [mode, file.path, file.ours]);

  // Switch view mode, capturing in-progress edits from the live view first so
  // the target mode re-seeds from them (§0.6 / §4.1 — never lose edits either
  // direction). `resultRef` is set synchronously so the recreation effect (fired
  // by `setMode`) seeds from the captured text.
  const switchMode = (next: Mode): void => {
    if (next === mode) return;
    const live = viewRef.current
      ? viewRef.current.state.doc.toString()
      : (mergeRef.current?.b.state.doc.toString() ?? null);
    if (live !== null) {
      resultRef.current = live;
      setResult(live);
    }
    setMode(next);
  };

  const unresolved = hasUnresolvedMarkers(result);
  const saveDisabled = mutating || unresolved;

  const handleStage = (): void => {
    setSaveError(null);
    void onResolve(file.path, result).catch((e: unknown) => {
      setSaveError(e instanceof Error ? e.message : String(e));
    });
  };

  return (
    <div className="conflict-editor" data-testid="conflict-editor">
      <div className="conflict-editor-header">
        <span className="conflict-editor-spacer" />
        <div className="conflict-editor-mode-toggle" role="group" aria-label="Editor view mode">
          <button
            type="button"
            className={`conflict-editor-mode-btn${mode === 'unified' ? ' is-active' : ''}`}
            aria-pressed={mode === 'unified'}
            onClick={() => switchMode('unified')}
          >
            Unified
          </button>
          <button
            type="button"
            className={`conflict-editor-mode-btn${mode === 'split' ? ' is-active' : ''}`}
            aria-pressed={mode === 'split'}
            onClick={() => switchMode('split')}
          >
            Side-by-side
          </button>
        </div>
        <button type="button" className="btn-secondary" onClick={onCancel}>
          Cancel
        </button>
        <button
          type="button"
          className="btn-primary"
          disabled={saveDisabled}
          title={unresolved ? 'Resolve all conflict markers first' : 'Stage the resolved file'}
          onClick={handleStage}
        >
          Stage resolved
        </button>
      </div>
      {saveError !== null && (
        <div className="error-banner error-banner-dismissible conflict-editor-error" role="alert">
          <span className="error-banner-text">{saveError}</span>
          <button
            type="button"
            className="error-dismiss"
            aria-label="Dismiss error"
            onClick={() => setSaveError(null)}
          >
            {'×'}
          </button>
        </div>
      )}
      {mode === 'split' && (
        <div className="conflict-editor-split-labels" aria-hidden="true">
          <span className="conflict-editor-split-label">Ours</span>
          <span className="conflict-editor-split-label">Theirs / Result</span>
        </div>
      )}
      <div className="conflict-editor-cm" ref={hostRef} />
    </div>
  );
}

// Default export so `React.lazy(() => import('./ConflictEditor'))` can code-split
// CodeMirror out of the main bundle (P12b SHOULD-FIX). Named export retained.
export default ConflictEditor;
