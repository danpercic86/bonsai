import { EditorView, keymap, lineNumbers, highlightActiveLine } from '@codemirror/view';
import { EditorState, Compartment, type Extension } from '@codemirror/state';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { LanguageDescription, syntaxHighlighting, HighlightStyle } from '@codemirror/language';
import { languages } from '@codemirror/language-data';
import { tags as t } from '@lezer/highlight';
import { conflictRegionWidgets, conflictOverviewRuler } from './conflictCmExtensions';

// ---- theme --------------------------------------------------------------

export type CmTheme = 'light' | 'dark';

export function readTheme(): CmTheme {
  return document.documentElement.getAttribute('data-theme') === 'light' ? 'light' : 'dark';
}

/** Minimal light/dark CM theme keyed on the app's `data-theme` (no CM theme
 *  package). Uses the app's own CSS vars so it tracks palette changes. */
export function cmTheme(mode: CmTheme): ReturnType<typeof EditorView.theme> {
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

// ---- syntax highlighting ------------------------------------------------

// Maps Lezer highlight tags to the app's `--syn-*` CSS vars (same palette the
// highlight.js diff viewer uses) so the conflict editor is colored like every
// other code view and tracks the light/dark theme automatically. The grammar is
// loaded lazily per file (loadLanguageExtension); without a HighlightStyle the
// parsed tokens get no color, which is why the editor rendered plain text.
const bonsaiHighlightStyle = HighlightStyle.define([
  { tag: [t.keyword, t.modifier, t.controlKeyword, t.operatorKeyword], color: 'var(--syn-keyword)' },
  { tag: [t.string, t.special(t.string), t.regexp], color: 'var(--syn-string)' },
  { tag: [t.comment, t.lineComment, t.blockComment], color: 'var(--syn-comment)', fontStyle: 'italic' },
  { tag: [t.number, t.bool, t.null, t.atom], color: 'var(--syn-number)' },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: 'var(--syn-function)' },
  { tag: [t.typeName, t.className, t.namespace], color: 'var(--syn-type)' },
  { tag: [t.propertyName, t.attributeName], color: 'var(--syn-attr)' },
  { tag: [t.tagName], color: 'var(--syn-tag)' },
  { tag: [t.operator, t.punctuation, t.separator, t.bracket], color: 'var(--syn-punctuation)' },
  { tag: [t.meta, t.docComment], color: 'var(--syn-meta)' },
]);

// ---- shared extensions (P12d §4.1) --------------------------------------

// Both the unified EditorView and the split `b` (result) editor mount the SAME
// editable extension list — factored here so the region widgets, overview ruler,
// updateListener, history and the theme/language compartments never diverge.
// The `theme`/`lang` Compartments are per-view instances (a Compartment may only
// belong to one EditorState), passed in by the caller.
export function editableExtensions(
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
    syntaxHighlighting(bonsaiHighlightStyle),
    theme.of(cmTheme(readTheme())),
  ];
}

// Extensions for the read-only `a` (ours) pane in split mode: same theme + lazy
// language as the editable side (ours is the same language), but no region
// widgets / ruler / history — it is never edited.
export function readonlyExtensions(theme: Compartment, lang: Compartment): Extension[] {
  return [
    lineNumbers(),
    highlightActiveLine(),
    EditorState.readOnly.of(true),
    lang.of([]),
    syntaxHighlighting(bonsaiHighlightStyle),
    theme.of(cmTheme(readTheme())),
  ];
}

// ---- lazy syntax highlighting (P12d §4.2) -------------------------------

// Resolve a CodeMirror language for `path` via `@codemirror/language-data` and
// lazily load its grammar. Returns null when nothing matches (plain text) or the
// async load fails. The resolved Extension is applied through a `Compartment` so
// it lands AFTER mount without recreating the view.
export async function loadLanguageExtension(path: string): Promise<Extension | null> {
  const desc = LanguageDescription.matchFilename(languages, path);
  if (desc === null) return null;
  try {
    return await desc.load();
  } catch {
    return null;
  }
}
