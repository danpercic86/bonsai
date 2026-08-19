/**
 * P69 §3.2 — the TypeScript half of the defaults parity mechanism.
 *
 * The chain this pins:
 *   `UiSettings` (interface)  →  DEFAULT_UI_SETTINGS (literal, annotated, so a
 *   missing or misspelt key is a COMPILE error)  →  uiSettingsDefaults.json (the
 *   oracle, deep-equal below)  →  Rust `Settings::default()` (the `assert_eq!` in
 *   `src-tauri/src/settings_ui_tests.rs`).
 *
 * DEFERRED (P69e): the Rust link of that chain is NOT yet in place — landing it
 * needs a `cargo test` run, which the P73 concurrency protocol forbids while the
 * shared target dir is busy. Until it lands, the oracle is pinned from the TS side
 * ONLY, and a Rust-side default change would go unnoticed here. Two details for
 * whoever writes it:
 *   - Rust has no `UiSettings::default()`; the value to serialise is
 *     `ui_settings_of(&settings::Settings::default())`, and `ui_settings_of` is
 *     currently private (`fn`) in `commands/ui_settings.rs` — it needs
 *     `pub(crate)`.
 *   - `aiMaxBudgetUsd` is an `f64`, so the oracle writes `0.0`. `serde_json`
 *     compares `Number::Float(0.0) != Number::PosInt(0)`; writing plain `0` in the
 *     JSON would fail the Rust assert while passing this file (JS has one number
 *     type). Do not "tidy" that `0.0`.
 */
import { describe, expect, it } from 'vitest';
import { DEFAULT_UI_SETTINGS as MOCK_DEFAULT_UI_SETTINGS } from '../ipc/mock/persistence';
import { DEFAULT_UI_SETTINGS, ENV_DERIVED_DEFAULT_KEYS, cloneDefaultUiSettings } from './defaults';
import oracle from './uiSettingsDefaults.json';

describe('DEFAULT_UI_SETTINGS ⟷ the shared Rust⟷TS oracle', () => {
  it('deep-equals the oracle exactly (both directions — an added, removed or changed default fails)', () => {
    expect(DEFAULT_UI_SETTINGS).toEqual(oracle);
    // Explicit key-set check as well: `toEqual` already covers it, but this
    // reports WHICH key drifted instead of dumping two 30-key objects.
    expect(Object.keys(DEFAULT_UI_SETTINGS).sort()).toEqual(Object.keys(oracle).sort());
    expect(Object.keys(DEFAULT_UI_SETTINGS.graph).sort()).toEqual(
      Object.keys(oracle.graph).sort(),
    );
  });

  it('spot-pins the values other suites and the UI copy depend on', () => {
    expect(DEFAULT_UI_SETTINGS.theme).toBe('dark');
    expect(DEFAULT_UI_SETTINGS.listView).toBe('tree');
    expect(DEFAULT_UI_SETTINGS.panelDensity).toBe('cozy');
    expect(DEFAULT_UI_SETTINGS.graph.rowHeight).toBe(32);
    expect(DEFAULT_UI_SETTINGS.graph.avatarRadius).toBe(10);
    expect(DEFAULT_UI_SETTINGS.graph.laneWidth).toBe(16);
    expect(DEFAULT_UI_SETTINGS.paneWidths).toEqual({ sidebar: 240, rightPanel: 380 });
    expect(DEFAULT_UI_SETTINGS.autoFetch).toEqual({ enabled: false, intervalMinutes: 5 });
    expect(DEFAULT_UI_SETTINGS.healthRefresh).toEqual({ enabled: false, intervalMinutes: 30 });
  });

  it('carries NO identity profiles — the two seeded ones are mock-only (§3.3)', () => {
    expect(DEFAULT_UI_SETTINGS.profiles).toEqual([]);
    expect(JSON.stringify(DEFAULT_UI_SETTINGS)).not.toContain('mock-');
  });

  it('keeps the P68 mode sentinels (§3.4: these rows get no ↺)', () => {
    expect(DEFAULT_UI_SETTINGS.aiIdleTimeoutSecs).toBe(300);
    expect(DEFAULT_UI_SETTINGS.aiHardCapSecs).toBe(0);
    expect(DEFAULT_UI_SETTINGS.aiMaxBudgetUsd).toBe(0);
    expect(DEFAULT_UI_SETTINGS.aiMaxTurns).toBe(6);
    expect(DEFAULT_UI_SETTINGS.aiBulkMaxBytes).toBe(400_000);
  });

  it('leaves the external-tool commands empty — `` means auto-detect (§3.4)', () => {
    expect(DEFAULT_UI_SETTINGS.terminalCommand).toBe('');
    expect(DEFAULT_UI_SETTINGS.editorCommand).toBe('');
  });

  it('has an EMPTY env-derived escape hatch, and every listed key would be real', () => {
    expect(ENV_DERIVED_DEFAULT_KEYS).toEqual([]);
    for (const key of ENV_DERIVED_DEFAULT_KEYS) {
      expect(DEFAULT_UI_SETTINGS).toHaveProperty(key);
    }
  });
});

describe('the mock seed composes from the production defaults (§3.3)', () => {
  // The contract puts this assertion in `persistence.test.tsx`; that file belongs
  // to another increment's lane, so the divergence pin lives here, next to the
  // constant it protects. Move it if/when the two files are owned together.
  it('differs from production defaults ONLY in `profiles`', () => {
    const keys = Object.keys(DEFAULT_UI_SETTINGS) as (keyof typeof DEFAULT_UI_SETTINGS)[];
    expect(Object.keys(MOCK_DEFAULT_UI_SETTINGS).sort()).toEqual([...keys].sort());
    for (const key of keys) {
      if (key === 'profiles') continue;
      expect({ [key]: MOCK_DEFAULT_UI_SETTINGS[key] }).toEqual({ [key]: DEFAULT_UI_SETTINGS[key] });
    }
  });

  it('seeds exactly the two harness identity profiles other suites depend on', () => {
    expect(MOCK_DEFAULT_UI_SETTINGS.profiles.map((p) => p.id)).toEqual([
      'mock-work',
      'mock-personal',
    ]);
    expect(DEFAULT_UI_SETTINGS.profiles).toEqual([]);
  });

  it('does not alias the production defaults (a mock mutation cannot leak)', () => {
    expect(MOCK_DEFAULT_UI_SETTINGS.graph).not.toBe(DEFAULT_UI_SETTINGS.graph);
    expect(MOCK_DEFAULT_UI_SETTINGS.paneWidths).not.toBe(DEFAULT_UI_SETTINGS.paneWidths);
  });
});

describe('cloneDefaultUiSettings', () => {
  it('returns a deep copy that cannot corrupt the shared defaults', () => {
    const copy = cloneDefaultUiSettings();
    expect(copy).toEqual(DEFAULT_UI_SETTINGS);
    expect(copy).not.toBe(DEFAULT_UI_SETTINGS);
    expect(copy.graph).not.toBe(DEFAULT_UI_SETTINGS.graph);
    copy.graph.rowHeight = 99;
    copy.profiles.push({
      id: 'x',
      label: 'x',
      userName: 'x',
      userEmail: 'x@x.dev',
      signingKey: null,
    });
    expect(DEFAULT_UI_SETTINGS.graph.rowHeight).toBe(32);
    expect(DEFAULT_UI_SETTINGS.profiles).toEqual([]);
  });
});
