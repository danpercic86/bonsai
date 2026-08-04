# P43 — First-run onboarding + empty-state polish (contract)

Status: DRAFT for orchestrator review. **Frontend-mostly, UX-subjective.** Conservative
defaults chosen; every subjective call is flagged inline as **[SUBJECTIVE]** and collected in
§9. All flags are non-blocking — senior-dev implements the recommended default unless the
orchestrator overrides.

---

## 1. Overview & invariants

Goal: a short, dismissible **guided first-run flow** the first time Bonsai launches with no
repo ever opened, plus a friendlier **no-repo / unborn-HEAD empty state**. Reuse shipped flows;
add the absolute minimum backend.

Invariants held:
- **Rust owns Git logic.** No new Git logic here. Identity read/write reuses the P40
  `getConfig`/`setConfig` commands verbatim. Open/clone/init reuse the P21 handlers already in
  `App.tsx`.
- **Reuse the prefs store.** The "seen onboarding" flag is **one additive `bool` field** on the
  existing `Settings` struct, surfaced through the existing `getUiSettings` / `setUiSettings`
  commands. **No new Tauri command.** (§6)
- **Browser-harness verifiable.** `src/ipc/mock.ts` keeps compiling; the flag defaults *unset*
  in a fresh mock store and a `?onboarding=1` URL seam force-opens the overlay so the flow is
  provable in a plain browser. (§7, §8)
- **No regression** to the existing open/clone/init/recents path — the empty state is
  restructured additively, never removing a working action.
- **File discipline.** Onboarding lives in its own component file(s); the empty state is
  *extracted out of* `App.tsx` into its own file (App.tsx is already large). Soft ~500-line cap.

Backend touch is intentionally tiny: one `bool` field threaded through
`Settings` → `UiSettings` → `UiSettingsPatch` and the existing get/set mapping. Everything else
is frontend.

---

## 2. The onboarding flow

### 2.1 Surface — **[SUBJECTIVE #1]**

**Recommendation: a centered, dismissible modal overlay** (same visual family as
`ShortcutOverlay` / `SettingsPanel`; backdrop + Esc-to-close + a persistent **Skip / ✕**), NOT a
full-screen takeover and NOT inline-in-empty-state. Rationale: least intrusive that still
guides; consistent with every other Bonsai overlay; trivially dismissible; participates in the
existing `globalModalOpen` gate so it suppresses graph keyboard shortcuts while open. It renders
*above* the empty state so first-run shows overlay-over-empty-state (coherent, not competing).

### 2.2 Step order — **[SUBJECTIVE #2, important]**

The milestone's nominal order is Welcome → Identity → Open/Clone → Tour. **This is not
implementable as written without new backend**, because `getConfig`/`setConfig` are
**repo-scoped** (`get_config(repoId, level)` opens the repo's config handle even for
`level: 'global'`; see `commands.rs::get_config_inner` → `repo_path(state, repo_id)`). At the
Identity step in the nominal order **no repo is open yet**, so there is no `repoId` to pass.

**Recommended default (zero new backend): reorder to**

1. **Welcome**
2. **Open or clone a repo**
3. **Identity check** (now `activeRepo` exists → `getConfig`/`setConfig` work)
4. **Feature tour**

Justification: identity only matters once you have a repo to commit to; the tour points at the
graph / AI-assets / health chrome which only exist once a repo is open anyway. Reordering keeps
backend at zero and makes steps 3–4 fully functional.

Alternative (rejected as default, flag for orchestrator): keep the nominal order and add a tiny
repo-less `getGlobalIdentity()` / `setGlobalIdentity(name,email)` pair backed by
`git2::Config::open_default()`. Cleaner UX order, but it is *new backend* and duplicates P40.
Recommend only if the orchestrator wants identity strictly first.

Skip-tolerance: if the user dismisses the Open/Clone step without opening a repo, the Identity
and Tour steps render an informational variant ("Open a repository to finish setup") and the
overlay can still be closed; the seen-flag is still persisted (§2.4).

### 2.3 State machine

```
type OnboardingStep = 'welcome' | 'openRepo' | 'identity' | 'tour';
const ORDER: OnboardingStep[] = ['welcome', 'openRepo', 'identity', 'tour'];

state: { step: OnboardingStep }              // starts at 'welcome'
Next()  -> step = ORDER[idx+1]  (Finish when at last step)
Back()  -> step = ORDER[idx-1]  (disabled on first)
Skip()  -> close + persist seen=true         // available on every step
Finish()-> close + persist seen=true
Esc / ✕ / backdrop == Skip
```

Per-step behavior:

- **welcome** — static: product name, one-line value prop, "Get started" (Next) + "Skip".
- **openRepo** — reuses App's existing handlers (`handleOpenRepository`, `handleCloneOpen`,
  `handleInitRepository`) passed in as props, plus the recents list. On a successful open (i.e.
  `activeRepo` becomes non-null) the step **auto-advances** to `identity`. If the user opens
  nothing, Next is allowed but marks steps 3–4 as the informational variant.
- **identity** — on entry, if `activeRepo != null`, call
  `getConfig(activeRepo, 'global')` and read the curated `user.name` / `user.email` entries.
  - Both `effectiveValue` set → render **"Identity ready"** (name/email shown, greyed), Next.
  - Either unset → two text inputs (prefill any set value); **Save** calls
    `setConfig(activeRepo, 'global', 'user.name'|'user.email', value)` for each changed field,
    then Next. **[SUBJECTIVE #3]** default target level is **`global`** (identity is
    machine-wide) — confirmed no new command needed.
  - `activeRepo == null` → informational variant (see §2.2). No `getConfig` call is made with a
    null repo (guard against the `noRepo` reject).
- **tour** — **[SUBJECTIVE #4]** v1 is **a few static cards**, NOT interactive coach-marks.
  Three cards: the commit graph (center), the AI-assets panel (🤖 toolbar button), the health
  dashboard (📊 toolbar button). Each card = icon + title + one sentence. Rationale: coach-marks
  need live anchor positions against the canvas + toolbar and a highlight/portal system — high
  effort, brittle, and out of scope for a polish milestone. Static cards convey the same "here's
  where things live" at a fraction of the risk. Finish button persists + closes. Coach-marks are
  explicitly deferred.

### 2.4 Persistence — **[SUBJECTIVE #5]**

**Recommendation: a real backend pref** (`onboardingSeen: bool`), NOT localStorage. Rationale:
the `UiSettings`/`UiSettingsPatch`/`get_ui_settings`/`set_ui_settings` plumbing already exists,
so this is one additive field and *zero new commands*; it is unit-testable in `settings.rs`
alongside the other additive-field back-compat tests; and it lives with the rest of app prefs
rather than a parallel storage mechanism. localStorage would be simpler but is untestable in the
Rust suite and splits the source of truth.

Write timing: persist `onboardingSeen: true` on **Skip and on Finish** (any dismissal), via
`setUiSettings({ onboardingSeen: true })`. First-run detection at App startup:
`getUiSettings().onboardingSeen === false` ⇒ open the overlay once.

### 2.5 Re-trigger — **[SUBJECTIVE #6]**

**Recommendation: include a small re-trigger.** Add a **"Show welcome tour"** button to
`SettingsPanel` (a plain button near the top/"About"-ish area) that calls an `onShowOnboarding`
prop → App re-opens the overlay (does not reset the seen flag). This makes the flow re-findable
and repeatably testable. Also honor the `?onboarding=1` URL seam (§7) for the harness. Keep it
to these two entry points; no menu-bar item in v1.

---

## 3. Empty-state polish (P43b)

Scope — **[SUBJECTIVE #7]: keep additive, restyle-not-restructure-the-actions.**

### 3.1 No-repo state (App.tsx `.empty-state`)
- **Extract** the current inline `.empty-state` block (App.tsx ~lines 824–875) into a new
  presentational `src/components/EmptyState.tsx`. Pure extraction first (no behavior change),
  then polish. This shrinks App.tsx and matches file-discipline.
- Polish: keep the three primary actions (**Open** / **Clone…** / **New…**) and the recents
  list exactly as they behave today; add a short friendly sub-headline and a little visual
  breathing room / icon. Preserve the `error` banner and `loading` states.
- Do **not** add an identity link here: SettingsPanel's identity section is repo-scoped and
  `activeRepo` is null in the no-repo state, so an identity CTA would dead-end. (Identity in the
  no-repo world is handled by the onboarding flow after a repo is opened.)

### 3.2 Unborn-HEAD (empty repo) state
- A repo **is** open here, so identity works. This state renders inside the workspace
  (`StatusPanel` / right panel), not App's empty-state. **[SUBJECTIVE #7b]** Recommend a *light*
  touch only: friendlier copy for the "no commits yet" state and, where an identity is unset,
  reuse the existing `onOpenIdentitySettings` callback (already wired through RepoWorkspace) so
  "Set your Git identity" opens SettingsPanel focused on Identity. No structural change to the
  staging/first-commit path.

---

## 4. Component breakdown

New files:
- `src/components/OnboardingOverlay.tsx` — **container**: owns `step` state machine (§2.3),
  persistence call (§2.4), the identity `getConfig`/`setConfig` effect, and wires the reused
  open/clone/init handlers passed as props. Render body delegates to the step components; keep
  under the soft cap by extracting step markup.
- `src/components/OnboardingSteps.tsx` — **presentational** step cards
  (`WelcomeStep`, `OpenRepoStep`, `IdentityStep`, `TourStep`) as small exported components. If
  this file approaches ~400 lines, split per-step into `Onboarding<Step>.tsx` in the same
  increment.
- `src/components/EmptyState.tsx` — extracted + polished no-repo empty state (§3.1).

Changed files:
- `src/App.tsx` — (a) mount `<OnboardingOverlay>`; (b) read `onboardingSeen` from
  `getUiSettings()` at startup and open the overlay when unset OR when `?onboarding=1`; (c) pass
  the existing open/clone/init handlers, recents, `openIdentitySettings`, and `activeRepo` to the
  overlay; (d) add overlay `open` to the `globalModalOpen` OR-chain; (e) swap the inline
  empty-state block for `<EmptyState …>`; (f) hold `onShowOnboarding` and pass it to
  `SettingsPanel`.
- `src/components/SettingsPanel.tsx` — add the "Show welcome tour" button + `onShowOnboarding`
  prop (§2.5).
- `src/ipc/types.ts` — add `onboardingSeen` to `UiSettings` and `UiSettingsPatch` (§6).
- `src/ipc/mock.ts` — thread `onboardingSeen` through `readUiSettings`/`setUiSettings`; default
  it **`false`** in a fresh mock store so the harness first-load shows onboarding (§7).

Backend (the one addition):
- `src-tauri/src/settings.rs` — add `pub onboarding_seen: bool` to `Settings`
  (`#[serde(default)]` via the container default; `Default` = `false`), plus a back-compat unit
  test mirroring `old_settings_file_without_ai_fields_loads_defaults`.
- `src-tauri/src/commands.rs` — extend the `UiSettings`↔`Settings` mapping in
  `get_ui_settings` / `set_ui_settings` to carry `onboarding_seen` (read) and apply the optional
  patch field (write). No new command, no new capability entry.

---

## 5. TypeScript surface (exact)

```ts
// New — src/components/OnboardingOverlay.tsx
export type OnboardingStep = 'welcome' | 'openRepo' | 'identity' | 'tour';

export interface OnboardingOverlayProps {
  open: boolean;
  /** Called on Skip/Finish/Esc/✕. App persists seen=true and closes. */
  onClose: () => void;
  /** Null until a repo is opened during (or before) the flow. */
  activeRepo: string | null;
  recents: RecentRepo[];
  loading: boolean;
  /** Reused P21 handlers, owned by App. */
  onOpenRepository: () => void;
  onCloneOpen: () => void;
  onInitRepository: () => void;
  onOpenRecent: (path: string) => void;
}

// New — src/components/EmptyState.tsx
export interface EmptyStateProps {
  loading: boolean;
  error: string | null;
  recents: RecentRepo[];
  onOpenRepository: () => void;
  onCloneOpen: () => void;
  onInitRepository: () => void;
  onOpenRecent: (path: string) => void;
}

// SettingsPanel.tsx — new prop (additive)
onShowOnboarding: () => void;
```

Persistence is done through the **existing** IPC methods — no new signatures:
`getUiSettings(): Promise<UiSettings>` and `setUiSettings(patch: UiSettingsPatch): Promise<UiSettings>`.
Identity uses the **existing** `getConfig(repoId, 'global')` / `setConfig(repoId, 'global', key, value)`.

---

## 6. IPC / type additions (the only backend change)

TypeScript (`src/ipc/types.ts`):
```ts
export interface UiSettings {
  // …existing fields…
  /** P43: first-run onboarding has been shown+dismissed. Defaults false. */
  onboardingSeen: boolean;
}
export interface UiSettingsPatch {
  // …existing fields…
  onboardingSeen?: boolean;
}
```

Rust (`src-tauri/src/settings.rs`, inside `Settings`, camelCase wire key `onboardingSeen`):
```rust
/// P43: first-run onboarding shown+dismissed. Additive `#[serde(default)]`;
/// a legacy settings.json without this key loads as `false` (⇒ show once).
pub onboarding_seen: bool,
```
`Default for Settings` sets `onboarding_seen: false`. `commands.rs` get/set mapping carries it.
`SETTINGS_VERSION` stays `1` (additive field, per the established precedent in the `Settings`
doc comment).

**Confirmed:** zero new commands; one additive field; no new Tauri capability surface.

---

## 7. Mock seam (harness)

- `readUiSettings()` / `setUiSettings()` in `mock.ts` include `onboardingSeen`, default
  **`false`** in a fresh (no-localStorage) store → the first plain-browser load shows the
  onboarding overlay, proving the gate. Dismissal persists `true` to the mock's localStorage so
  it does not reappear on reload (mirrors real behavior).
- `?onboarding=1` (read in `App.tsx`, not the mock) force-opens the overlay regardless of the
  flag — the reliable, repeatable harness trigger and the manual re-find path. `?onboarding=0`
  is not needed; clearing localStorage / the Settings re-trigger cover reset.
- Identity step in the harness uses the mock `getConfig`/`setConfig`, which require an open repo
  — so the harness must open a mock repo at the Open/Clone step before the identity step is
  meaningful (matches the reorder in §2.2).

---

## 8. Acceptance criteria & gate plan

**AI gate — browser-harness verifiable (orchestrator):**
1. Fresh mock store (`onboardingSeen` unset) → overlay appears over the empty state on load.
2. `?onboarding=1` force-opens the overlay even after it was dismissed.
3. Next/Back walk welcome → openRepo → identity → tour; Skip/✕/Esc available on every step.
4. Opening a mock repo at the openRepo step auto-advances to identity.
5. Identity step with an unset field → typing + Save issues `setConfig(repo,'global',…)` (assert
   via mock state / console); with both set → shows "Identity ready", no write.
6. Skip and Finish both call `setUiSettings({ onboardingSeen: true })`; after either, a reload
   does **not** reshow the overlay (flag persisted).
7. "Show welcome tour" in Settings re-opens the overlay without clearing the flag.
8. Empty state renders (no-repo): Open/Clone/New + recents still work (no regression); unborn-
   HEAD state shows the friendlier copy + identity link.
9. `tsc` clean; `pnpm build` clean; mock layer compiles.

**USER CHECKPOINT — native only:**
- First real launch with a never-before-seen profile shows onboarding once; overall first-run
  *feel* / copy tone; that dismissal truly persists across an app restart (real `settings.json`);
  the folder picker + clone dialog invoked from the overlay behave natively.

Rust unit test (tester): `onboarding_seen` back-compat + round-trip in `settings.rs`.

---

## 9. Flagged subjective decisions (for orchestrator accept/adjust)

1. **Surface** = centered dismissible overlay (vs full-screen / inline). *Rec: overlay.*
2. **Step order** = reordered to Welcome → Open/Clone → Identity → Tour, because P40
   `getConfig`/`setConfig` are repo-scoped and cannot run before a repo is open. *Rec: reorder
   (zero backend).* Alternative: add repo-less `get/setGlobalIdentity` to keep identity first
   (new backend) — only if strict order is wanted.
3. **Identity target level** = `global`. *Rec: global; no new command.*
4. **Tour depth** = static cards, coach-marks deferred. *Rec: static cards.*
5. **Persistence** = backend `onboardingSeen` field (vs localStorage). *Rec: backend field.*
6. **Re-trigger** = "Show welcome tour" in Settings + `?onboarding=1` seam. *Rec: include both.*
7. **Empty-state scope** = additive restyle + extract to `EmptyState.tsx`; light unborn-HEAD copy
   touch. *Rec: additive.*

---

## 10. Sub-increments

- **P43a — onboarding overlay + flow + persistence + re-trigger + mock seam.**
  New `OnboardingOverlay.tsx` + `OnboardingSteps.tsx`; App mount + startup gate + `?onboarding=1`;
  `onboardingSeen` end-to-end (types.ts, mock.ts, settings.rs, commands.rs mapping); Settings
  re-trigger button. Harness gate items 1–7 + 9.
- **P43b — empty-state polish.** Extract `EmptyState.tsx` (pure move, then polish) + friendlier
  no-repo copy; light unborn-HEAD copy + identity link reuse. Harness gate item 8.

(Combine into one increment only if P43a lands small; recommended to keep them split so the
backend field + flow land and get committed before the cosmetic pass.)
