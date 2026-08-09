import { describe, expect, it } from 'vitest';

import { errorMessage, isAppError } from './errors';

describe('isAppError', () => {
  it('true for a minimal AppError shape', () => {
    expect(isAppError({ kind: 'io', message: 'boom' })).toBe(true);
  });

  it('true even with extra fields', () => {
    expect(isAppError({ kind: 'git', message: 'x', code: -3, detail: {} })).toBe(true);
  });

  it('false for null / undefined / primitives', () => {
    expect(isAppError(null)).toBe(false);
    expect(isAppError(undefined)).toBe(false);
    expect(isAppError('kind message')).toBe(false);
    expect(isAppError(42)).toBe(false);
  });

  it('false when kind or message is missing', () => {
    expect(isAppError({ message: 'no kind' })).toBe(false);
    expect(isAppError({ kind: 'no message' })).toBe(false);
    expect(isAppError({})).toBe(false);
  });

  it('false when message is not a string', () => {
    expect(isAppError({ kind: 'x', message: 42 })).toBe(false);
    expect(isAppError({ kind: 'x', message: null })).toBe(false);
  });

  it('a plain Error is NOT an AppError (no kind)', () => {
    expect(isAppError(new Error('e'))).toBe(false);
  });
});

describe('errorMessage', () => {
  it('AppError → its message', () => {
    expect(errorMessage({ kind: 'git', message: 'not a repo' })).toBe('not a repo');
  });

  it('Error → its message (including subclasses)', () => {
    expect(errorMessage(new Error('plain'))).toBe('plain');
    expect(errorMessage(new TypeError('typed'))).toBe('typed');
  });

  it('string / number / null / undefined → String(e)', () => {
    expect(errorMessage('raw')).toBe('raw');
    expect(errorMessage(7)).toBe('7');
    expect(errorMessage(null)).toBe('null');
    expect(errorMessage(undefined)).toBe('undefined');
  });

  it('plain object without the shape → String(e)', () => {
    expect(errorMessage({ oops: true })).toBe('[object Object]');
  });

  it('empty-string message is preserved (AppError with message: "")', () => {
    expect(errorMessage({ kind: 'x', message: '' })).toBe('');
  });
});
