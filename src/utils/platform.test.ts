/** macOS-correctness: platform detection + shortcut-label rendering. Every
 *  assertion pins BOTH branches, so a regression on either OS is caught on the
 *  other one's CI. */
import { describe, it, expect } from 'vitest';
import {
  detectIsMac,
  isMac,
  keyLabel,
  shortcutKeys,
  shortcutLabel,
  shortcutSeparator,
} from './platform';

describe('detectIsMac', () => {
  it('detects macOS from userAgentData, platform, or userAgent', () => {
    expect(detectIsMac({ userAgentData: { platform: 'macOS' } })).toBe(true);
    expect(detectIsMac({ platform: 'MacIntel' })).toBe(true);
    expect(
      detectIsMac({ userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/605.1' }),
    ).toBe(true);
    expect(detectIsMac({ platform: 'iPhone' })).toBe(true);
  });

  it('reports NOT-mac for Windows/Linux and for anything unknown', () => {
    expect(detectIsMac({ platform: 'Win32', userAgent: 'Mozilla/5.0 (Windows NT 10.0)' })).toBe(
      false,
    );
    expect(detectIsMac({ platform: 'Linux x86_64', userAgent: 'Mozilla/5.0 (X11; Linux)' })).toBe(
      false,
    );
    // jsdom's UA contains "AppleWebKit" but is NOT a Mac — must not false-positive.
    expect(detectIsMac({ platform: '', userAgent: 'Mozilla/5.0 (win32) AppleWebKit/537.36' })).toBe(
      false,
    );
    expect(detectIsMac({})).toBe(false);
  });

  it('never throws on a hostile or exotic navigator', () => {
    const hostile = {
      get platform(): string {
        throw new Error('blocked');
      },
    } as unknown as Parameters<typeof detectIsMac>[0];
    expect(detectIsMac(hostile)).toBe(false);
    expect(detectIsMac({ platform: 42 as unknown as string })).toBe(false);
  });

  it('exports a resolved boolean for the running environment', () => {
    expect(typeof isMac).toBe('boolean');
  });
});

describe('keyLabel', () => {
  it('maps modifiers to words off-Mac and to glyphs on Mac', () => {
    expect(keyLabel('Mod', false)).toBe('Ctrl');
    expect(keyLabel('Mod', true)).toBe('⌘');
    expect(keyLabel('Shift', false)).toBe('Shift');
    expect(keyLabel('Shift', true)).toBe('⇧');
    expect(keyLabel('Alt', false)).toBe('Alt');
    expect(keyLabel('Alt', true)).toBe('⌥');
    // The LITERAL control key stays Control on Mac (⌃), not Command.
    expect(keyLabel('Ctrl', false)).toBe('Ctrl');
    expect(keyLabel('Ctrl', true)).toBe('⌃');
  });

  it('is case-insensitive on modifiers and passes other keys through', () => {
    expect(keyLabel('mod', true)).toBe('⌘');
    expect(keyLabel('MOD', true)).toBe('⌘');
    for (const mac of [false, true]) {
      expect(keyLabel('Enter', mac)).toBe('Enter');
      expect(keyLabel('F5', mac)).toBe('F5');
      expect(keyLabel('↑', mac)).toBe('↑');
      expect(keyLabel('?', mac)).toBe('?');
    }
  });
});

describe('shortcutKeys / shortcutLabel', () => {
  it('splits a spec into per-cap labels on both platforms', () => {
    expect(shortcutKeys('Mod+Shift+F', false)).toEqual(['Ctrl', 'Shift', 'F']);
    expect(shortcutKeys('Mod+Shift+F', true)).toEqual(['⌘', '⇧', 'F']);
    expect(shortcutKeys('Mod+Enter', false)).toEqual(['Ctrl', 'Enter']);
    expect(shortcutKeys('Mod+Enter', true)).toEqual(['⌘', 'Enter']);
  });

  it('tolerates whitespace, empty segments, and single-key specs', () => {
    expect(shortcutKeys(' Mod + O ', false)).toEqual(['Ctrl', 'O']);
    expect(shortcutKeys('Mod++O', false)).toEqual(['Ctrl', 'O']);
    expect(shortcutKeys('', false)).toEqual([]);
    expect(shortcutKeys('Esc', true)).toEqual(['Esc']);
  });

  it('joins with + off-Mac and runs glyphs together on Mac', () => {
    expect(shortcutSeparator(false)).toBe('+');
    expect(shortcutSeparator(true)).toBe('');
    expect(shortcutLabel('Mod+Shift+U', false)).toBe('Ctrl+Shift+U');
    expect(shortcutLabel('Mod+Shift+U', true)).toBe('⌘⇧U');
    expect(shortcutLabel('Mod+R', false)).toBe('Ctrl+R');
    expect(shortcutLabel('Mod+R', true)).toBe('⌘R');
  });

  it('defaults its platform argument to the detected one', () => {
    expect(shortcutLabel('Mod+K')).toBe(shortcutLabel('Mod+K', isMac));
  });
});
