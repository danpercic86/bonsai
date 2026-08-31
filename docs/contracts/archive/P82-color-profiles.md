# P82 — Color-coded Git Identity Profiles (backend/data + IPC contract)

**Scope:** additive `color` field on `IdentityProfile` so the active/selected identity
is distinguishable at a glance even when labels are duplicated. Backend + IPC + types +
mock only. Visual rules (swatch geometry, pill styling, active indicator, contrast) are
owned by `ui-designer` in `docs/contracts/P82-ui.md`.

**Non-goals:** no change to how identities are *applied* to git config. Color is a
display-only attribute; it is never written to `user.*` config. `applyIdentityProfile`
is untouched.

---

## 1. Data model

### 1.1 Color representation — DECISION: fixed named palette (`ProfileColor` enum)

Use a small curated enum, **not** a free-form hex string.

Rationale:
- **Theme-aware:** each variant maps to a CSS token that the ui-designer defines per
  theme (dark/light). A raw hex would look wrong in one theme and break the token system.
- **A11y-safe:** the palette is a vetted set of ~8 hues with adequate contrast against
  both panel backgrounds; free hex lets users pick invisible/low-contrast colors.
- **Stable wire shape:** a closed enum round-trips cleanly and lets both Rust and TS
  exhaustively switch. No validation/clamping of arbitrary strings needed.

**Palette (8 variants + neutral fallback).** Names are semantic-neutral hue names so the
token layer owns the exact color:

```
Neutral | Slate | Blue | Teal | Green | Amber | Orange | Purple | Pink
```

(`Neutral` is the default/migration fallback; the other 8 are the assignable hues.)
The exact CSS custom-property token per variant per theme is defined in the UI contract;
Rust/TS only carry the enum tag.

### 1.2 Rust struct change — `src-tauri/src/settings.rs`

Add the enum (place near the other prefs enums, or in `settings/prefs.rs` and re-export;
recommend `settings.rs` alongside `IdentityProfile` to keep the P44 concern colocated):

```rust
/// Curated identity-profile color (P82). Closed named palette — maps to a
/// theme-aware CSS token in the frontend (see P82-ui.md); no raw hex on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default,
         serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileColor {
    #[default]
    Neutral,
    Slate,
    Blue,
    Teal,
    Green,
    Amber,
    Orange,
    Purple,
    Pink,
}
```

Extend `IdentityProfile` (currently ~lines 49-61) with one additive field:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityProfile {
    pub id: String,
    pub label: String,
    pub user_name: String,
    pub user_email: String,
    pub signing_key: Option<String>,
    /// P82: display color. Additive `#[serde(default)]` — a pre-P82 profile
    /// (no `color` key) deserializes as `ProfileColor::Neutral`. Display-only;
    /// never written to git config on apply.
    #[serde(default)]
    pub color: ProfileColor,
}
```

Wire shape (camelCase): `"color": "blue"` / `"neutral"` / `"amber"` … Absent key ⇒ `neutral`.

### 1.3 Backward-compat / migration rule

- **Field-level `#[serde(default)]`** on `color` (the container `Settings` already carries
  `default`, but that only covers whole missing structs; a `Vec<IdentityProfile>` element
  present-but-missing-`color` needs the field-level default). This is REQUIRED — without it,
  an old `settings.json` whose `profiles[*]` lack `color` fails to deserialize.
- No `SETTINGS_VERSION` bump — this meets the documented additive-default bar (settings.rs
  §63-82).
- **Migration = pure additive:** any profile persisted before P82 loads with
  `color = Neutral`. No rewrite pass on load.

**DECISION for orchestrator (flagged):** default color strategy —
- **Option A (recommended, minimal): fixed `Neutral` default.** Zero migration code,
  no cross-field logic (serde defaults can't read sibling `id`). Existing profiles all
  show neutral until the user edits them; new profiles get a color at create time (the UI
  auto-picks the next unused hue — a UI concern).
- **Option B: deterministic hue from `id` hash.** Nicer "instantly distinct" first run,
  but serde `default` cannot see `id`, so it needs a normalize pass in BOTH load paths
  (Rust `load_from` and TS `readUiSettings`/`sanitizeProfiles`) — more surface, must stay
  in sync, and it silently overrides `Neutral` (ambiguous vs a user who *chose* neutral).

Recommendation: **Option A.** Keep the default `Neutral`; let the create-flow assign the
first free hue in the UI layer (deterministic, visible, and user-overridable) rather than
baking hashing into two persistence layers. If the user prefers auto-distinct existing
profiles on upgrade, adopt B and add the normalize pass.

---

## 2. IPC surface

**No new commands.** Identity profiles are persisted as a whole-array field inside
`UiSettings`, mutated via the existing settings update path; `color` rides along.

- **`get_ui_settings` / `update_ui_settings`** (`src-tauri/src/commands/ui_settings.rs`):
  `UiSettings.profiles: Vec<IdentityProfile>` and `UiSettingsPatch.profiles:
  Option<Vec<IdentityProfile>>` are **whole-array replace** (lines ~38-39, ~94-96, 166-167,
  238). Adding `color` to `IdentityProfile` requires **no signature change** — the field
  flows through the existing serialize/patch automatically. Verify no field-by-field
  reconstruction of `IdentityProfile` exists that would drop `color` (grep confirms the
  Vec is cloned/replaced wholesale, so it is safe).
- **`applyIdentityProfile`** (`commands/config.rs` / mock `config.ts`): **unchanged.** It
  takes discrete `(userName, userEmail, signingKey)` and writes git config; `color` is not
  a parameter and is never applied.
- Frontend still generates `id` via `crypto.randomUUID()`; `color` is just another field
  on the profile object the frontend builds and sends in the `profiles` array. **Confirmed:
  color is a plain field on the create/update payload, no dedicated command.**

---

## 3. Frontend types + mock

### 3.1 `src/ipc/types.ts`

Mirror the palette and extend the interface (currently lines 1031-1038):

```ts
/** P82: curated identity-profile color palette. Mirrors Rust `ProfileColor`
 *  (serde camelCase). Maps to a theme-aware CSS token (see P82-ui.md). */
export type ProfileColor =
  | 'neutral'
  | 'slate'
  | 'blue'
  | 'teal'
  | 'green'
  | 'amber'
  | 'orange'
  | 'purple'
  | 'pink';

export interface IdentityProfile {
  id: string;
  label: string;
  userName: string;
  userEmail: string;
  signingKey: string | null;
  /** P82: display color. Optional on the wire (absent ⇒ 'neutral' for
   *  pre-P82 persisted profiles); always present on values Bonsai writes. */
  color?: ProfileColor;
}
```

Note the `?`: keep it optional so a legacy persisted object (localStorage in the harness)
without `color` still satisfies the type; readers treat `undefined` as `'neutral'`.

### 3.2 Mock layer — `src/ipc/mock/`

- **`persistence.ts`**
  - `DEFAULT_UI_SETTINGS.profiles` seeds (lines ~97-112): give the two seeded profiles
    distinct colors so the harness demonstrates the feature — e.g. `Work → 'blue'`,
    `Personal → 'green'`.
  - `sanitizeProfiles` (lines ~161-173): add a per-element check that `color`, **if
    present**, is one of the palette strings; drop/normalize an invalid `color` to
    `'neutral'` rather than rejecting the whole profile (color is non-essential). Missing
    `color` is valid (⇒ `undefined`, read as neutral). Keep the existing required-string
    checks for id/label/userName/userEmail/signingKey unchanged.
- **`session.ts`** (line ~96): `profiles: patch.profiles ?? current.profiles` — whole-array
  replace already carries `color`; no change beyond types.
- No change to `mock/handlers/assets.ts` — that `profiles` is the unrelated worktree/asset
  profile store, NOT identity profiles. Do not touch it.

Existing mock tests referencing `DEFAULT_UI_SETTINGS.profiles`
(`persistence.test.tsx`, `sanitizeProfiles.test.ts`) will need their fixtures updated to
include `color`; tester owns that.

---

## 4. Acceptance criteria

### AI gate
- `cargo check` + `clippy -D warnings` clean; `ProfileColor` derives compile.
- Round-trip test: serialize an `IdentityProfile { color: Blue }` → JSON contains
  `"color":"blue"`; deserialize back equals original.
- **Migration test:** a `settings.json` (or a `profiles` JSON blob) whose profile omits
  `color` deserializes with `color == ProfileColor::Neutral` (Rust) / reads as neutral
  (TS). No error, no version bump.
- `tsc` clean; `ProfileColor` union exported and used by `IdentityProfile`.
- Mock: `readUiSettings` returns seeded profiles carrying valid `color`; `sanitizeProfiles`
  keeps a profile with a good color, coerces an invalid color to neutral, and keeps a
  color-less legacy profile (as neutral). `update_ui_settings` round-trips a changed color
  through the mock and back.
- vitest + cargo suites green after fixture updates.

### USER CHECKPOINT (native, both themes)
- In `pnpm tauri dev`: create/edit a profile, pick a color; the color is unmistakable in
  the identity menu, the profile card, and the active-profile indicator, in BOTH dark and
  light themes, and it distinguishes two profiles that share the same label.
- Restart the app: the chosen colors persist (real `settings.json`).

---

## 5. Files senior-dev will touch

Backend:
- `src-tauri/src/settings.rs` — add `ProfileColor` enum + `color` field on `IdentityProfile`.
- (`src-tauri/src/settings/prefs.rs` only if the team prefers enums colocated there +
  re-export; recommend keeping it in `settings.rs`.)

Frontend:
- `src/ipc/types.ts` — `ProfileColor` union + `color?` on `IdentityProfile`.
- `src/ipc/mock/persistence.ts` — seed colors + `sanitizeProfiles` color check.

No change needed: `commands/ui_settings.rs`, `commands/config.rs`, `mock/handlers/config.ts`,
`mock/handlers/session.ts` (beyond types flowing through), `mock/handlers/assets.ts` (unrelated).

UI consumers (`IdentityMenu.tsx`, `settings/IdentityProfileCard.tsx`,
`settings/catalog/` identity rows) are rendering changes governed by `docs/contracts/P82-ui.md`.
