import { describe, expect, it } from 'vitest';

import { isImagePath } from './imagePaths';

describe('isImagePath (D4)', () => {
  it('accepts the full raster image set', () => {
    for (const ext of ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'ico', 'avif']) {
      expect(isImagePath(`logo.${ext}`), ext).toBe(true);
    }
  });

  it('is case-insensitive on the extension', () => {
    expect(isImagePath('sprite.PNG')).toBe(true);
    expect(isImagePath('photo.Jpg')).toBe(true);
    expect(isImagePath('anim.GIF')).toBe(true);
    expect(isImagePath('icon.AvIf')).toBe(true);
  });

  it('EXCLUDES svg — it stays a text diff (OQ3)', () => {
    expect(isImagePath('icon.svg')).toBe(false);
    expect(isImagePath('assets/icon.SVG')).toBe(false);
  });

  it('rejects non-image extensions', () => {
    for (const p of ['notes.txt', 'main.rs', 'index.tsx', 'data.json', 'archive.zip']) {
      expect(isImagePath(p), p).toBe(false);
    }
  });

  it('keys off the basename, ignoring dots in parent directories', () => {
    expect(isImagePath('src/assets/logo.png')).toBe(true);
    expect(isImagePath('my.assets/README')).toBe(false); // dot only in a dir segment
    expect(isImagePath('a.b/c/d.webp')).toBe(true);
  });

  it('treats a file with no extension as not an image', () => {
    expect(isImagePath('README')).toBe(false);
    expect(isImagePath('Makefile')).toBe(false);
    expect(isImagePath('src/mod')).toBe(false);
  });

  it('does not treat a leading-dot dotfile as an image', () => {
    // basename ".png" -> dot at index 0 -> no real extension.
    expect(isImagePath('.png')).toBe(false);
    expect(isImagePath('.gitignore')).toBe(false);
    expect(isImagePath('config/.png')).toBe(false);
  });

  it('uses only the LAST extension segment', () => {
    expect(isImagePath('image.png.txt')).toBe(false); // final ext = txt
    expect(isImagePath('photo.tar.gz')).toBe(false);
    expect(isImagePath('screenshot.backup.png')).toBe(true); // final ext = png
  });
});
