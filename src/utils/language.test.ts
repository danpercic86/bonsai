import { describe, expect, it } from 'vitest';

import { detectLanguage } from './language';

describe('detectLanguage', () => {
  it('maps common extensions to id + label', () => {
    expect(detectLanguage('src/app.ts')).toEqual({ id: 'typescript', label: 'ts' });
    expect(detectLanguage('src/App.tsx')).toEqual({ id: 'typescript', label: 'tsx' });
    expect(detectLanguage('a/b/c.rs')).toEqual({ id: 'rust', label: 'rs' });
    expect(detectLanguage('x.py')).toEqual({ id: 'python', label: 'py' });
    expect(detectLanguage('x.json')).toEqual({ id: 'json', label: 'json' });
  });

  it('html/htm/svg alias to the xml grammar with their own labels', () => {
    expect(detectLanguage('index.html')).toEqual({ id: 'xml', label: 'html' });
    expect(detectLanguage('old.htm')).toEqual({ id: 'xml', label: 'html' });
    expect(detectLanguage('icon.svg')).toEqual({ id: 'xml', label: 'svg' });
  });

  it('extension match is case-insensitive', () => {
    expect(detectLanguage('README.MD')).toEqual({ id: 'markdown', label: 'md' });
    expect(detectLanguage('Main.JAVA')).toEqual({ id: 'java', label: 'java' });
  });

  it('uses the LAST dot of the basename (multi-dot names)', () => {
    expect(detectLanguage('archive.tar.gz')).toBeNull();
    expect(detectLanguage('component.test.tsx')).toEqual({ id: 'typescript', label: 'tsx' });
  });

  it('unknown extension → null', () => {
    expect(detectLanguage('binary.exe')).toBeNull();
    expect(detectLanguage('notes.txt')).toBeNull();
  });

  it('no extension → null', () => {
    expect(detectLanguage('Makefile')).toBeNull();
    expect(detectLanguage('src/LICENSE')).toBeNull();
  });

  it('dotfiles → null (leading dot is not an extension)', () => {
    expect(detectLanguage('.gitignore')).toBeNull();
    expect(detectLanguage('config/.bashrc')).toBeNull();
  });

  it('only the basename is inspected — dots in directories do not count', () => {
    expect(detectLanguage('v1.2/README')).toBeNull();
    expect(detectLanguage('my.dir/app.go')).toEqual({ id: 'go', label: 'go' });
  });

  it('trailing dot → empty extension → null', () => {
    expect(detectLanguage('weird.')).toBeNull();
  });

  it('empty string and bare slash → null, no throw', () => {
    expect(detectLanguage('')).toBeNull();
    expect(detectLanguage('/')).toBeNull();
  });

  it('unicode filename with known extension still detects', () => {
    expect(detectLanguage('docs/読み方.md')).toEqual({ id: 'markdown', label: 'md' });
  });

  it('c-family header/source labels', () => {
    expect(detectLanguage('x.h')).toEqual({ id: 'c', label: 'h' });
    expect(detectLanguage('x.hpp')).toEqual({ id: 'cpp', label: 'hpp' });
    expect(detectLanguage('x.cc')).toEqual({ id: 'cpp', label: 'cpp' });
  });

  it('shell variants all label "sh"; toml uses the ini grammar id "toml"', () => {
    expect(detectLanguage('run.zsh')).toEqual({ id: 'bash', label: 'sh' });
    expect(detectLanguage('run.bash')).toEqual({ id: 'bash', label: 'sh' });
    expect(detectLanguage('Cargo.toml')).toEqual({ id: 'toml', label: 'toml' });
  });
});
