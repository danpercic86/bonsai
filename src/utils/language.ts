export type LangId =
  | 'typescript' | 'javascript' | 'json' | 'html' | 'xml' | 'css' | 'scss'
  | 'markdown' | 'rust' | 'python' | 'csharp' | 'java' | 'go' | 'ruby'
  | 'php' | 'bash' | 'yaml' | 'toml' | 'sql' | 'c' | 'cpp' | 'kotlin' | 'swift';

export interface LangMeta {
  /** highlight.js language id used for both grammar load and highlight. */
  id: LangId;
  /** Short chip label shown to the user (may differ from id, e.g. tsx/jsx). */
  label: string;
}

const EXT_MAP: Record<string, LangMeta> = {
  ts:   { id: 'typescript', label: 'ts' },
  tsx:  { id: 'typescript', label: 'tsx' },
  mts:  { id: 'typescript', label: 'ts' },
  cts:  { id: 'typescript', label: 'ts' },
  js:   { id: 'javascript', label: 'js' },
  jsx:  { id: 'javascript', label: 'jsx' },
  mjs:  { id: 'javascript', label: 'js' },
  cjs:  { id: 'javascript', label: 'js' },
  json: { id: 'json',       label: 'json' },
  html: { id: 'xml',        label: 'html' },
  htm:  { id: 'xml',        label: 'html' },
  xml:  { id: 'xml',        label: 'xml' },
  svg:  { id: 'xml',        label: 'svg' },
  css:  { id: 'css',        label: 'css' },
  scss: { id: 'scss',       label: 'scss' },
  sass: { id: 'scss',       label: 'sass' },
  md:   { id: 'markdown',   label: 'md' },
  markdown: { id: 'markdown', label: 'md' },
  rs:   { id: 'rust',       label: 'rs' },
  py:   { id: 'python',     label: 'py' },
  cs:   { id: 'csharp',     label: 'cs' },
  java: { id: 'java',       label: 'java' },
  go:   { id: 'go',         label: 'go' },
  rb:   { id: 'ruby',       label: 'rb' },
  php:  { id: 'php',        label: 'php' },
  sh:   { id: 'bash',       label: 'sh' },
  bash: { id: 'bash',       label: 'sh' },
  zsh:  { id: 'bash',       label: 'sh' },
  yml:  { id: 'yaml',       label: 'yaml' },
  yaml: { id: 'yaml',       label: 'yaml' },
  toml: { id: 'toml',       label: 'toml' },
  sql:  { id: 'sql',        label: 'sql' },
  c:    { id: 'c',          label: 'c' },
  h:    { id: 'c',          label: 'h' },
  cpp:  { id: 'cpp',        label: 'cpp' },
  cc:   { id: 'cpp',        label: 'cpp' },
  cxx:  { id: 'cpp',        label: 'cpp' },
  hpp:  { id: 'cpp',        label: 'hpp' },
  kt:   { id: 'kotlin',     label: 'kt' },
  swift:{ id: 'swift',      label: 'swift' },
};

export function detectLanguage(path: string): LangMeta | null {
  const base = path.slice(path.lastIndexOf('/') + 1);
  const dot = base.lastIndexOf('.');
  if (dot <= 0) return null; // no extension / dotfile
  const ext = base.slice(dot + 1).toLowerCase();
  return EXT_MAP[ext] ?? null;
}
