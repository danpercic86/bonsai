# Settings → AI → Assistance: "why is the autonomy choice disabled?" — UI addendum

Status: **spec, not yet implemented.** Extends `docs/contracts/ui-reference.md` §12.3.3
("Disabling a whole group") with the single-row variant of the same pattern. Fold this
addendum into ui-reference.md §12.3 (as a new §12.3.5, before §12.4 "Keyboard, roles, focus")
the next time that file is edited for an unrelated reason — it was written standalone here to
avoid a full-file rewrite of a large shared doc.

## 1. The gap

`src/components/SettingsAiSection.tsx`'s `AUTONOMY` row (`id="ai.conflict-resolution"`,
`disabled={!aiActive}`, `aiActive = aiEnabled && aiConsented`) renders no text anywhere
explaining why its two radios are inert. Two distinct causes collapse into the one
`!aiActive` boolean, and neither is stated:

- (a) **`!aiEnabled`** — the "Enable AI features" switch above is off. Self-evident *if*
  noticed, but the row is a full-width `stacked` block (two option paragraphs) some visual
  distance below the switch, not a tight one-line pairing like `general.fetch-interval` under
  its switch — and the sibling `SettingsAiRunSection` fieldset **already shows a reason note
  for this exact case** (`GATE_NOTE`, §12.3.3). Leaving the Autonomy row silent while the
  section right below it explains itself is the inconsistency a user actually reported.
- (b) **`aiEnabled && !aiConsented`** — the switch reads **on** (default `aiEnabled` is
  `true`, per `useUiSettings.ts`, on a repo that has never completed consent) but the
  one-time `AiConsentDialog` was never confirmed. This case is **not self-evident at all**:
  nothing on screen says a consent step exists, and the unrelated `aiAvailability` CLI-status
  line right below can simultaneously read "installed and ready," which looks contradictory.

Decision: show a reason for **both** (a) and (b), not just (b). Rejected alternative: omit
case (a) as "self-evident" (matches `general.fetch-interval`'s no-text precedent) — rejected
because it would leave the Autonomy row as the one inconsistent surface on a page where the
Runs/Limits fieldset immediately below it already explains the identical `aiActive` gate.

## 2. Copy (verbatim, curly quotes to match the existing frozen `GATE_NOTE`)

```
Case (a), !aiEnabled:
  Turn on “Enable AI features” above to change this.

Case (b), aiEnabled && !aiConsented:
  Turn “Enable AI features” off and on again to confirm access.
```

Case (a) mirrors `SettingsAiRunSection.tsx`'s existing `GATE_NOTE` string 1:1 (only "these" →
"this", singular row vs. the 8-knob group) — same voice, same imperative construction, zero
new vocabulary.

Case (b) is new copy. It states the actual recovery path: `setAiEnabled` (in
`useSettingsPanelAdapter.ts`) only opens `AiConsentDialog` when the switch transitions
**off→on** while `!aiConsented`; since the switch already reads on, the user must cycle it to
retrigger that dialog. "Confirm access" echoes what the dialog itself grants (repo-content
access to Claude), not the dialog's own button label ("Enable"), to avoid two different UI
elements both saying "Enable" back at the user in the same sentence.

Exactly one of the two strings renders at a time (`aiEnabled` disambiguates them) — never
both, never a third generic fallback.

## 3. Placement / mechanism — sibling paragraph, NOT the `SettingsRow` `hint` prop

Do **not** pass this text through `SettingsRow`'s `hint` prop on the `AUTONOMY` row itself.
That prop renders inside `.settings-row-help-slot`, which is a descendant of the row `div`
that gets `.settings-row.is-disabled { opacity: 0.55 }` — exactly the trap §12.3.3 already
documents and avoids for the fieldset case ("dimming the fieldset would drag the very note
that explains the state down with it"). A 0.55-dimmed reason-for-the-dim is a contrast
regression happening in the same edit that's supposed to fix legibility.

Instead: a plain sibling `<p className="settings-group-lead">`, placed in
`SettingsAiSection.tsx` **between** the closing `</SettingsRow>` of `AUTONOMY` and the
existing `{aiAvailability === null ? …}` status block — i.e. outside every row, same
placement rule as the `GATE_NOTE` paragraph in `SettingsAiRunSection.tsx`, just without a
wrapping `<fieldset>` (there isn't one here — it's a single row, not a group).

```
<SettingsRow id={AUTONOMY} stacked disabled={!aiActive}>
  … unchanged radiogroup + the two existing per-option hints …
</SettingsRow>

{!aiActive && (
  <p className="settings-group-lead" id="ai-autonomy-disabled-hint">
    {aiEnabled
      ? 'Turn “Enable AI features” off and on again to confirm access.'
      : 'Turn on “Enable AI features” above to change this.'}
  </p>
)}

{aiAvailability === null ? ( … unchanged … ) : … }
```

`.settings-group-lead` is the existing class for exactly this role (group-level reason text,
`margin: 0 0 8px; max-width: 56ch; font-size: 12px; color: var(--text-2)` — see
`src/styles/settings-primitives.css` lines 165–174, whose own comment already names the
"outside every row" placement rule). No new CSS, no new class, no new token.

One id, not two: only one branch is ever mounted at a time, so `ai-autonomy-disabled-hint`
never dangles and never needs disambiguating between the two copy variants.

## 4. Accessibility

- **`aria-describedby` composes, doesn't replace.** Each radio already carries a static
  `aria-describedby` pointing at its own always-visible per-option hint
  (`ai-autonomy-propose-hint` / `ai-autonomy-auto-hint`). Add the new id to that value **only
  while the row is disabled**, matching the "dangling idref is worse than none" rule already
  applied to the fieldset's `aria-describedby` in §12.3.3:

  ```
  aria-describedby={
    aiActive ? 'ai-autonomy-propose-hint' : 'ai-autonomy-propose-hint ai-autonomy-disabled-hint'
  }
  ```
  and correspondingly `'ai-autonomy-auto-hint'` / `'ai-autonomy-auto-hint ai-autonomy-disabled-hint'`
  on the second radio.
- **Keyboard, not hover-only.** This is static visible text wired via `aria-describedby`, not
  a tooltip — it is announced whenever a screen-reader user's focus lands on either
  (disabled, but still focusable via Tab in most AT since these are native `<input
  type="radio" disabled>`... note: **disabled native inputs are removed from the tab order in
  all browsers** and cannot themselves receive focus. That's not new — it's the existing
  behaviour of every disabled control in this file (§12.3.3 already accepts this trade-off for
  the fieldset case: `disabled` is what remove­s the descendants from the tab order in the
  first place). Sighted-disabled/screen-reader-visible parity here comes from the text being
  **permanently on-screen** (not hover-gated, not focus-gated) — a screen reader in browse/scan
  mode reaches the `<p>` directly as regular page content even though the radios themselves
  are skipped in tab order. No keyboard-only dead end: nothing here is discoverable *only* by
  hover.
- No role change needed on the paragraph itself (plain text, not a live region — the state
  only changes on user action elsewhere in the same view, not asynchronously).

## 5. Both themes, both densities

- Reuses `--text-2`, already measured in `ui-reference.md` §2: **7.9:1** dark / **4.9:1**
  light on `--bg-0` — both pass WCAG AA (4.5:1) for body text. Because this paragraph is a
  sibling of the disabled row (§3), it is **not** subject to the row's `opacity: 0.55`, so
  those ratios hold at full strength, not the degraded on-dim value.
- No color-only signal — this is a text explanation, not a status indicator.
- Density: `.settings-group-lead` carries one fixed geometry already (12px, `margin: 0 0 8px`)
  used identically in `cozy` and `compact` throughout Settings (§12.3.3's own note: "one
  geometry in both densities... do not add a density variant"). Same rule here — no density
  variant.

## 6. Harness / verification

Both branches are reachable in the mock-IPC harness by toggling `aiEnabled`/`aiConsented` in
the relevant fixture/store before rendering `SettingsAiSection` — no native-window dependency,
this is not a USER CHECKPOINT item. Verify:
- `aiEnabled: false` → case (a) copy renders, radios dimmed, `aria-describedby` composes.
- `aiEnabled: true, aiConsented: false` → case (b) copy renders (this is also the **default
  fresh-repo state** per `useUiSettings.ts`'s initial `useState`, so it should be the first
  thing exercised).
- `aiEnabled: true, aiConsented: true` → neither paragraph renders, `aria-describedby` reverts
  to the single per-option hint id, catalog help text for the row is unaffected (untouched by
  this change).

## 7. Files senior-dev touches

- `src/components/SettingsAiSection.tsx` — add the conditional `<p>` (§3) and the two
  `aria-describedby` composions (§4). No signature change to `SettingsAiSectionProps` — `aiEnabled`
  and `aiActive` are both already props.
- No change to `SettingsRow.tsx`, `settingsAvailability.ts`, or any catalog file.
