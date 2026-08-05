import { useEffect, useRef, useState } from 'react';
import { EditorView, keymap, lineNumbers, highlightActiveLine } from '@codemirror/view';
import { EditorState, Compartment, type Extension } from '@codemirror/state';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { MergeView } from '@codemirror/merge';
import { LanguageDescription } from '@codemirror/language';
import { languages } from '@codemirror/language-data';
import type { ConflictFile } from '../ipc';
import { detectLanguage } from '../utils/language';
import {
  parseConflictRegions,
  applyResolution,
  hasUnresolvedMarkers,
} from '../utils/conflictRegions';
import { conflictRegionWidgets, conflictOverviewRuler } from './conflictCmExtensions';
import type { P7SelfTestResult } from '../graph/frameStats';

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

// ---- theme --------------------------------------------------------------

type CmTheme = 'light' | 'dark';

function readTheme(): CmTheme {
  return document.documentElement.getAttribute('data-theme') === 'light' ? 'light' : 'dark';
}

/** Minimal light/dark CM theme keyed on the app's `data-theme` (no CM theme
 *  package). Uses the app's own CSS vars so it tracks palette changes. */
function cmTheme(mode: CmTheme): ReturnType<typeof EditorView.theme> {
  return EditorView.theme(
    {
      '&': {
        color: 'var(--text-1)',
        backgroundColor: 'var(--bg-0)',
        fontSize: '12px',
        height: '100%',
      },
      '.cm-scroller': {
        fontFamily: 'var(--font-mono, ui-monospace, monospace)',
        lineHeight: '1.5',
      },
      '.cm-content': { caretColor: 'var(--accent)' },
      '&.cm-focused .cm-cursor': { borderLeftColor: 'var(--accent)' },
      '.cm-gutters': {
        backgroundColor: 'var(--bg-0)',
        color: 'var(--text-3)',
        border: 'none',
      },
      '.cm-activeLine': {
        backgroundColor: 'color-mix(in srgb, var(--accent) 8%, transparent)',
      },
      '.cm-activeLineGutter': {
        backgroundColor: 'color-mix(in srgb, var(--accent) 8%, transparent)',
      },
      '&.cm-focused': { outline: 'none' },
    },
    { dark: mode === 'dark' },
  );
}

// ---- shared extensions (P12d §4.1) --------------------------------------

// Both the unified EditorView and the split `b` (result) editor mount the SAME
// editable extension list — factored here so the region widgets, overview ruler,
// updateListener, history and the theme/language compartments never diverge.
// The `theme`/`lang` Compartments are per-view instances (a Compartment may only
// belong to one EditorState), passed in by the caller.
function editableExtensions(
  theme: Compartment,
  lang: Compartment,
  updateListener: Extension,
): Extension[] {
  return [
    lineNumbers(),
    highlightActiveLine(),
    history(),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    EditorState.allowMultipleSelections.of(true),
    conflictRegionWidgets(),
    conflictOverviewRuler(),
    updateListener,
    lang.of([]),
    theme.of(cmTheme(readTheme())),
  ];
}

// Extensions for the read-only `a` (ours) pane in split mode: same theme + lazy
// language as the editable side (ours is the same language), but no region
// widgets / ruler / history — it is never edited.
function readonlyExtensions(theme: Compartment, lang: Compartment): Extension[] {
  return [
    lineNumbers(),
    highlightActiveLine(),
    EditorState.readOnly.of(true),
    lang.of([]),
    theme.of(cmTheme(readTheme())),
  ];
}

// ---- lazy syntax highlighting (P12d §4.2) -------------------------------

// Resolve a CodeMirror language for `path` via `@codemirror/language-data` and
// lazily load its grammar. Returns null when nothing matches (plain text) or the
// async load fails. The resolved Extension is applied through a `Compartment` so
// it lands AFTER mount without recreating the view.
async function loadLanguageExtension(path: string): Promise<Extension | null> {
  const desc = LanguageDescription.matchFilename(languages, path);
  if (desc === null) return null;
  try {
    return await desc.load();
  } catch {
    return null;
  }
}

// ---- self-test (P12 §2.2) ----------------------------------------------

// Inline copy of the mock `MERGE_AUTH_TEXT` fixture (§2.2 permits an inline
// copy rather than importing mock internals). Keep in sync with src/ipc/mock.ts.
const SELFTEST_TEXT = [
  'import { hash } from "./crypto";',
  '',
  'export function login(user: string, password: string): Session {',
  '<<<<<<< HEAD',
  '  const token = hash(`${user}:${password}:v2`);',
  '  return { user, token };',
  '=======',
  '  const token = hash(password + user);',
  '  return { user: user.toLowerCase(), token };',
  '>>>>>>> feature/login',
  '}',
  '',
].join('\n');

/** Run the pure conflict-region helper assertions; logs one line, mirroring
 *  `p7SelfTest`. Mock/dev only (registered by the mount effect). */
function conflictSelfTest(): P7SelfTestResult {
  let pass = 0;
  const failures: string[] = [];
  const check = (name: string, cond: boolean): void => {
    if (cond) pass++;
    else failures.push(name);
  };

  const regions = parseConflictRegions(SELFTEST_TEXT);
  check('parse finds 1 region', regions.length === 1);
  const r = regions[0];
  if (r !== undefined) {
    check('region index 0', r.index === 0);
    check('region startLine', r.startLine === 3);
    check('region sepLine', r.sepLine === 6);
    check('region endLine', r.endLine === 9);
    check('region oursLabel HEAD', r.oursLabel === 'HEAD');
    check('region theirsLabel feature/login', r.theirsLabel === 'feature/login');
    check(
      'region oursLines',
      r.oursLines.length === 2 &&
        r.oursLines[0] === '  const token = hash(`${user}:${password}:v2`);' &&
        r.oursLines[1] === '  return { user, token };',
    );
    check(
      'region theirsLines',
      r.theirsLines.length === 2 &&
        r.theirsLines[0] === '  const token = hash(password + user);' &&
        r.theirsLines[1] === '  return { user: user.toLowerCase(), token };',
    );
  } else {
    failures.push('region 0 undefined');
  }

  check('parse "no markers" -> []', parseConflictRegions('no markers').length === 0);
  check('hasUnresolvedMarkers true on fixture', hasUnresolvedMarkers(SELFTEST_TEXT) === true);

  if (r !== undefined) {
    // §3.4: all three accept choices on the single-region fixture.
    const OURS_BODY = ['  const token = hash(`${user}:${password}:v2`);', '  return { user, token };'];
    const THEIRS_BODY = [
      '  const token = hash(password + user);',
      '  return { user: user.toLowerCase(), token };',
    ];
    const oursText = applyResolution(SELFTEST_TEXT, r, 'ours');
    check('applyResolution ours has no markers', hasUnresolvedMarkers(oursText) === false);
    check(
      'applyResolution ours keeps ours body',
      oursText.includes(OURS_BODY.join('\n')) && !oursText.includes('hash(password + user)'),
    );

    const theirsText = applyResolution(SELFTEST_TEXT, r, 'theirs');
    check('applyResolution theirs has no markers', hasUnresolvedMarkers(theirsText) === false);
    check(
      'applyResolution theirs keeps theirs body',
      theirsText.includes(THEIRS_BODY.join('\n')) &&
        !theirsText.includes('hash(`${user}:${password}:v2`)'),
    );

    const bothText = applyResolution(SELFTEST_TEXT, r, 'both');
    check('applyResolution both has no markers', hasUnresolvedMarkers(bothText) === false);
    check(
      'applyResolution both is ours-then-theirs',
      bothText.includes([...OURS_BODY, ...THEIRS_BODY].join('\n')),
    );
  }

  // §3.4: two-region synthetic fixture — resolving region 0 leaves exactly one
  // remaining region, correctly re-indexed (the property P12c's buttons rely on).
  const TWO_REGION_TEXT = [
    'top',
    '<<<<<<< HEAD',
    'a-ours',
    '=======',
    'a-theirs',
    '>>>>>>> branch-a',
    'middle',
    '<<<<<<< HEAD',
    'b-ours',
    '=======',
    'b-theirs',
    '>>>>>>> branch-b',
    'bottom',
  ].join('\n');
  const twoRegions = parseConflictRegions(TWO_REGION_TEXT);
  check('two-region fixture parses 2 regions', twoRegions.length === 2);
  const first = twoRegions[0];
  if (first !== undefined) {
    const afterFirst = applyResolution(TWO_REGION_TEXT, first, 'ours');
    const remaining = parseConflictRegions(afterFirst);
    check('after resolving region 0, exactly 1 region remains', remaining.length === 1);
    const only = remaining[0];
    if (only !== undefined) {
      // region 1 was at lines 7/9/11; region 0's block (lines 1..5, 5 lines)
      // collapsed to its 1-line ours body removed 4 lines, so it shifts up by 4.
      check('remaining region re-indexed to 0', only.index === 0);
      check('remaining region startLine', only.startLine === 3);
      check('remaining region sepLine', only.sepLine === 5);
      check('remaining region endLine', only.endLine === 7);
      check(
        'remaining region bodies intact',
        only.oursLines.length === 1 &&
          only.oursLines[0] === 'b-ours' &&
          only.theirsLines.length === 1 &&
          only.theirsLines[0] === 'b-theirs',
      );
    } else {
      failures.push('remaining region undefined');
    }
  } else {
    failures.push('two-region first undefined');
  }

  const result: P7SelfTestResult = { pass, fail: failures.length, failures };
  if (import.meta.env.DEV) console.log(`[bonsai] conflictSelfTest ${JSON.stringify(result)}`);
  return result;
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
    void loadLanguageExtension(file.path).then((ext) => {
      if (disposed || ext === null) return;
      for (const target of langTargets) {
        target.view.dispatch({ effects: target.comp.reconfigure(ext) });
      }
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

  const lang = detectLanguage(file.path);
  const unresolved = hasUnresolvedMarkers(result);
  const saveDisabled = mutating || unresolved;

  const handleStage = (): void => {
    setSaveError(null);
    void onResolve(file.path, result).catch((e: unknown) => {
      setSaveError(e instanceof Error ? e.message : String(e));
    });
  };

  return (
    <div className="conflict-editor">
      <div className="conflict-editor-header">
        <span className="conflict-editor-path mono" title={file.path}>
          {file.path}
        </span>
        {lang !== null && (
          <span className="lang-chip" data-lang={lang.id}>
            {lang.label}
          </span>
        )}
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
