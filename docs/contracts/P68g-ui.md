# P68g-2 — AI run settings, honest consent copy, and the ask-block hardening (UI contract)

Owner: ui-designer. Implementer: senior-dev. **Design-only document — no application code is
touched by this file.**

Inputs read (verified, not guessed): `docs/contracts/ui-reference.md` (§2 tokens + contrast notes,
§3 type/spacing, §8 states, §9 dock), `docs/contracts/P68-ai-conflict-streaming.md` (§1a verified CLI
sandbox behaviour, §8.3 the ten settings, §9 P68g scope + acceptance),
`docs/contracts/P68-security-audit.md` (H1, M2, M3, M5, M6, L4),
`docs/contracts/P68e-ai-activity-dock.md` (§3 log empty states, §4.1-4.4, §9 budgets, §11 a11y,
§12-F1/F2), `src/components/SettingsPanel.tsx` (1-395, the AI-assistance block at 330-378),
`src/components/SettingsAppearanceSection.tsx` (the label-bearing toggle-button idiom, 49-62),
`src/components/SettingsGraphSection.tsx` (section + `NumberSlider` + fieldset idioms),
`src/components/NumberSlider.tsx` (clamping behaviour), `src/components/ConfirmDialog.tsx`
(focus/Esc/variant), `src/components/AiActivityAsk.tsx` (as shipped by the M3 increment),
`src/components/dialogs/BulkAiConfirmDialog.tsx`, `src/App.tsx` (`app.settings` palette entry
611-617; the AI consent dialog 1036-1049; the MCP dialogs 1050-1079), `src/settings/ranges.ts`
(AI_* clamps), `src/ipc/mock/aiRunSettings.ts` (defaults + `clampAiRunSettings`),
`src/ipc/types.ts` (1233-1236, 1480-1549), `src/styles.css` (`.settings-*` 935-1057 + 1205-1254,
`.settings-config-hint` 1079-1093, `.dialog-card`/`.dialog-body`/`.dialog-body-note` 4473-4505,
`.confirm-name-list` 5845-5856), `crates/bonsai-core/src/git/ai_resolve_stream_events.rs` (127-141,
the `notable` exemption).

---

## 0. Decisions this contract makes (V1–V11)

**V1 — Two new sections, and `SettingsPanel.tsx` gets *smaller*.** The AI-assistance block (the
enable checkbox, the autonomy fieldset, the CLI-status line) moves verbatim out of
`SettingsPanel.tsx` into `SettingsAiSection.tsx`, because job 2's autonomy caveat belongs *inside*
that block and appending it to a 395-line container is exactly the growth this project forbids.
Net: SettingsPanel 395 → ~355 lines, two new small files.

**V2 — `0` is a mode, not a number.** `aiHardCapSecs: 0` and `aiMaxBudgetUsd: 0` are locked user
decisions. A number field showing `0` reads as empty/unset/broken. Each is therefore an **unchecked
checkbox plus a disabled numeric field** — the app's own `Background jobs` idiom
(`SettingsPanel.tsx:258-275`). Unchecked is a state the user chose, not a value they failed to
enter, and the copy says so ("Off by default: … Cancel … is how you stop one"). The same shape
carries `aiIdleTimeoutSecs`'s `0`, which is reachable by hand-editing (audit L4).

**V3 — Repository access is the section's first control, and its copy states the grant, not the
flag.** Audit M2: `aiConflictTools` shipped with no UI and no disclosure. It leads the section, uses
the house label-bearing toggle button, and its hint changes with the value so the *current* grant is
always a readable sentence rather than an inference from a button label.

**V4 — No warning colour anywhere in jobs 1 and 2.** These are settings the user chose and a consent
dialog they opened on purpose. `--warning` tints and `⚠` glyphs stay reserved for the CLI-missing
note that already exists and for the dock's `Needs you` state. Accuracy is carried by words.

**V5 — Must-read secondary text is `--text-2`, never `--text-3`.** `.dialog-body-note` and
`.settings-config-hint` are `--text-3` (3.38:1 dark / 2.96:1 light — below AA, ui-reference §2).
Consent facts and the `autoResolve` caveat are the *most* important text in their dialogs, so this
contract adds one class, `.dialog-body-detail` (`--text-2`), and moves
`BulkAiConfirmDialog`'s two existing notes onto it. `.dialog-body-note` stays for genuinely
decorative lines like `+N more`.

**V6 — The consent dialog body becomes its own file.** `src/components/dialogs/AiConsentDialog.tsx`.
Reasons: the copy is now ~140 words of load-bearing text that deserves a testable home; `App.tsx` is
1121+ lines and is currently owned by a concurrent refactor, so the edit there must be a 12-line
mechanical replacement, not a copy rewrite in place.

**V7 — The `autoResolve` caveat lives under the radio that turns it on.** Per-radio hint lines in
`SettingsAiSection.tsx`, visible while the user is choosing — not only in a consent dialog seen once.
The `autoResolve` label is also corrected to **"Resolve automatically"**, which is what the shipped
`BulkAiConfirmDialog` already calls it; "Auto-resolve, then review" promises a review step that
does not happen.

**V8 — `aiStreamLog` is exposed too (an 8th control).** `AiActivityLog`'s empty state already tells
the user to *"turn on \"Stream AI output\" in Settings"* — a control that does not exist. That is a
shipped dead-end pointer; one checkbox closes it, and it belongs in exactly this section.

**V9 — The Settings overlay has one density.** `panelDensity` is scoped to `.right-panel` (P67b) and
`.ai-dock` (P68e); the Settings card has never varied. This section inherits that: one geometry,
`.settings-number` 28px, buttons 32px, both ≥ the 24px AA floor. Nothing new to specify per density —
stated explicitly so the omission is not read as an oversight.

**V10 — No new palette entry, no new chrome.** `app.settings` already opens this surface; it gains
keywords (`ai claude limits budget tools spend`). Nothing is added to the header, toolbar or sidebar.

**V11 — Untrusted model text is attributed with words, not with colour** (job 3, §3). The shipped
`--text-1` choice in `AiActivityAsk` is **correct** and is now recorded in the dock contract; what it
was missing is salience, which comes from weight and one `--warning` glyph, not from yellow words.

---

## 1. Job 1 — `SettingsAiRunSection.tsx`

### 1.1 Placement

Inside `.dialog-card.settings-card` (560px), as the section immediately **after** AI assistance and
**before** AI access (MCP server) — the parent contract's §9 position ("after the autonomy
fieldset"). Standard `.settings-section` (margin-top 20px). No change to any other section's order.

```
┌ Settings ─────────────────────────────────────────────── × ┐
│ …                                                           │
│ AI ASSISTANCE                                               │  ← SettingsAiSection (job 2)
│   ☑ Enable AI features                                      │
│   Conflict resolution                                       │
│     ○ Propose & review                                      │
│       Each result opens as a proposal. …                     │
│     ○ Resolve automatically                                 │
│       Marker-free results are written to your files …        │
│   ✓ Claude Code CLI 2.1.234                                 │
│                                                             │
│ AI RUNS                                                     │  ← NEW SettingsAiRunSection
│   Applies to conflict resolution with Claude. Changes take   │
│   effect on the next run.                                    │
│                                                             │
│   Repository access                    [ Read-only ]        │  .settings-row
│   Claude can read, search and list files in this repository…│  hint, --text-2
│                                                             │
│   ☑ Stream AI output                                        │
│   Show every line the Claude CLI prints …                    │
│   ☐ Stream partial replies                                  │
│   Show Claude's text as it is typed …                        │
│                                                             │
│   Limits                                                    │  .settings-subsection-title
│   ☑ Stop a run that goes quiet                              │
│      After        [====|-----] [ 300 ] seconds              │  NumberSlider
│      Ends a run that has printed nothing for this long. …    │
│   ☐ Stop a run after a fixed time                           │
│      Limit        [==|-------] [1800 ] seconds  (disabled)  │
│      Off by default: a run has no deadline, and Cancel …     │
│      Replies per run [===|----] [   6 ] turns               │
│      How many times Claude may answer inside one run. …      │
│   ☐ Set a spend limit per run                               │
│      Limit                        [ 5.00 ] USD  (disabled)  │
│      Off by default: Bonsai does not cap what a run may …    │
│                                                             │
│   Bulk resolve                                              │  .settings-subsection-title
│      Batch size   [=|-------] [ 400 ] KB                    │
│      The most text Bonsai puts into one bulk run. …          │
│                                                             │
│ AI ACCESS (MCP SERVER)                                      │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Structure and geometry

- Root: `<section className="settings-section">` + `<h3 className="settings-section-title">AI runs</h3>`
  + one `<p className="settings-section-desc">`.
- All controls live inside **one** `<fieldset className="settings-section-fields" disabled={!aiActive}>`
  (`aiActive = aiEnabled && aiConsented`, already computed in `SettingsPanel.tsx:171` and passed in).
  A disabled `<fieldset>` makes every descendant inert and unfocusable in one place — no per-control
  `disabled` threading, and no half-live section.
- The "turn AI on first" line sits **outside** the fieldset so it is not dimmed by the fieldset's
  0.5 opacity.
- Two `.settings-subsection-title` groups (`Limits`, `Bulk resolve`) — the P63 Forge-signals
  precedent in `SettingsGraphSection.tsx:152`.
- Every hint is a `<p className="settings-hint">` directly after its control, `margin: 2px 0 10px`,
  11px, `--text-2`. (New class — `.settings-config-hint` is the right geometry at the wrong colour;
  see §4.)
- Spacing comes entirely from the existing classes: `.settings-row` / `.settings-control` margin-top
  8px, `.settings-checkbox` margin-bottom 10px. Nothing off the 4/8/12/16/24 scale is introduced.
- Indent for the two sentinel groups' numeric rows: `.settings-indent { margin-left: 24px; }` so the
  slider visibly belongs to the checkbox above it. 24px = the checkbox's own glyph+gap width.

### 1.3 The eight controls — exact spec

Ranges are imported from `src/settings/ranges.ts` (they already exist — do not re-declare numbers).
Every handler patches **exactly one** field.

| # | Field | Control | Range / step | Unit label |
|---|---|---|---|---|
| 1 | `aiConflictTools` | `.settings-row` + `.btn-secondary.settings-toggle-btn` showing the current value | `readOnly` ⇄ `none` | — |
| 2 | `aiStreamLog` | `.settings-checkbox` | — | — |
| 3 | `aiIncludePartialMessages` | `.settings-checkbox` | — | — |
| 4 | `aiIdleTimeoutSecs` | `.settings-checkbox` (on = non-zero) + `NumberSlider` | `AI_IDLE_TIMEOUT_MIN` 30 … `MAX` 3600, step 1 | `seconds` |
| 5 | `aiHardCapSecs` | `.settings-checkbox` (off by default) + `NumberSlider` | `AI_HARD_CAP_MIN` 60 … `MAX` 86400, step 1 | `seconds` |
| 6 | `aiMaxTurns` | `NumberSlider` | `AI_MAX_TURNS_MIN` 1 … `MAX` 20, step 1 | `turns` |
| 7 | `aiMaxBudgetUsd` | `.settings-checkbox` (off by default) + bare `.settings-number.settings-number-wide` | 0.5 … `AI_MAX_BUDGET_USD_MAX` 100, step 0.5 | `USD` |
| 8 | `aiBulkMaxBytes` | `NumberSlider` in **KB** | 20 … 4000 KB, step 1 → ×1000 bytes | `KB` |

**Sentinel-checkbox semantics (controls 4, 5, 7).** The checkbox is derived, not stored:
`checked = value !== 0`. Unchecking patches `{ field: 0 }`. Checking patches the field's **resume
value**: the last non-zero value held in component state for this Settings open, else the listed
default — `aiIdleTimeoutSecs: 300`, `aiHardCapSecs: 1800`, `aiMaxBudgetUsd: 5`. The numeric control
stays mounted and rendered while unchecked (`disabled`, showing the resume value) rather than
unmounting: a control that vanishes reads as "this feature does not exist", and re-mounting jumps the
section's height under the pointer.

**Control 7's number input.** `NumberSlider` rounds to integers, so it cannot carry USD. Spec a bare
row instead, reusing the markup `NumberSlider` produces minus the range input:
`.settings-control` > `<label className="settings-control-label" htmlFor="settings-ai-budget">` +
`.settings-control-inputs` > `<input id="settings-ai-budget" className="settings-number settings-number-wide" type="number" min={0.5} max={100} step={0.5}>` + `<span className="settings-unit">USD</span>`.
Commit on `change`: `Number(raw)`; ignore `NaN`; clamp to `[0.5, 100]`; round to 2 decimals
(`Math.round(n * 100) / 100`) — never round to an integer, and never let `0` arrive through this
field (that is the checkbox's job).

**Control 8's unit conversion.** Display `Math.round(aiBulkMaxBytes / 1000)`, patch `v * 1000`.
1 KB = 1000 B exactly, so the 20 000 / 4 000 000 clamps map to 20 / 4000 with no rounding drift and
no possibility of a value the Rust clamp would move.

### 1.4 Exact copy — implement verbatim

Section header and frame:

| Element | String |
|---|---|
| `h3` | `AI runs` |
| `.settings-section-desc` | `Applies to conflict resolution with Claude. Changes take effect on the next run.` |
| outside-fieldset line, only when `!aiActive` | `Turn on “Enable AI features” above to change these.` |
| `.settings-subsection-title` #1 | `Limits` |
| `.settings-subsection-title` #2 | `Bulk resolve` |

Control 1 — repository access:

| Element | String |
|---|---|
| `.settings-control-label` | `Repository access` |
| button, `aiConflictTools === 'readOnly'` | `Read-only` |
| button, `aiConflictTools === 'none'` | `No file access` |
| hint, `readOnly` | `Claude can read, search and list files in this repository while it resolves a conflict — that is what lets it match your surrounding code. Anything it reads is sent to Anthropic. It cannot write files, stage anything, or run commands, and reads outside this repository are refused.` |
| hint, `none` | `Claude sees only the conflicting versions of each file and nothing else in your repository. Resolutions are noticeably less accurate — this was Bonsai's behaviour before repository reads existed.` |

Controls 2-3 — output:

| Element | String |
|---|---|
| checkbox 2 | `Stream AI output` |
| hint 2 | `Show every line the Claude CLI prints in the AI activity dock. With this off, the dock still shows status, cost, which files Claude read, and any refused read — just not the rest of the output.` |
| checkbox 3 | `Stream partial replies` |
| hint 3 | `Show Claude's text as it is typed, instead of when each turn finishes. More output in the dock; partial text is never applied to a file.` |

Controls 4-7 — limits:

| Element | String |
|---|---|
| checkbox 4 | `Stop a run that goes quiet` |
| slider 4 label | `After` |
| hint 4, checked | `Ends a run that has printed nothing for this long — 300 seconds is five minutes. The clock pauses while Claude is waiting for your answer.` |
| hint 4, unchecked | `A run that stops printing is left alone. Cancel in the AI activity dock is how you end it.` |
| checkbox 5 | `Stop a run after a fixed time` |
| slider 5 label | `Limit` |
| hint 5, unchecked | `Off by default: a run has no deadline, and Cancel in the AI activity dock is how you stop one. Turn this on if you would rather have a hard limit.` |
| hint 5, checked | `Ends the run at this point whatever it is doing. The clock pauses while Claude is waiting for your answer.` |
| both 4 and 5 unchecked — extra line after hint 5 | `With neither limit on, a run continues until it finishes or you cancel it.` |
| slider 6 label | `Replies per run` |
| hint 6 | `How many times Claude may answer inside one run, including its answers to your questions. A run still asking after this many turns is ended.` |
| checkbox 7 | `Set a spend limit per run` |
| number 7 label | `Limit` |
| hint 7, unchecked | `Off by default: Bonsai does not cap what a run may spend. The AI activity dock shows the running cost after each turn, and Cancel stops it.` |
| hint 7, checked | `Passed to the Claude CLI as a budget for each run separately — not a total for the day.` |

Control 8 — bulk:

| Element | String |
|---|---|
| slider 8 label | `Batch size` |
| hint 8 | `The most text Bonsai puts into one bulk run. A larger merge is split into several runs, one after another — never truncated.` |

Copy rules applied, for the reviewer: sentence case; the two `0` defaults are described as decisions
("Off by default: …") and never as risks; no exclamation, no bold, no icon on any of these lines; no
raw flag name (`--max-budget-usd`, `--tools`, `--permission-mode`) leaks into the UI.

### 1.5 States

| State | Presentation |
|---|---|
| default | as §1.4 with the shipped defaults: `Read-only`, Stream AI output **on**, partial replies **off**, quiet-stop **on** at 300 s, fixed time **off**, 6 turns, spend limit **off**, 400 KB |
| hover, button/checkbox | inherited `.btn-secondary` / label hover — nothing new |
| active/pressed | inherited |
| `:focus-visible` | 2px `--accent`, offset 1px, on the toggle button, every checkbox, and both range and number inputs. `.settings-number:focus` currently only recolours its border (`styles.css:1030-1033`) — a **pre-existing** gap, not this contract's to fix; `.settings-number-wide` inherits it. Flagged §7-N1. |
| disabled — AI off (`!aiActive`) | whole fieldset inert at 0.5 opacity; the un-dimmed line `Turn on “Enable AI features” above to change these.` sits below the section description |
| disabled — sentinel off | the numeric row only: `NumberSlider` already renders `.settings-control.is-disabled` (0.5 opacity) and passes `disabled` to both inputs; the USD row gets the same wrapper class |
| `aiEligible === false` (AI on, consented, CLI **not** installed) | **controls stay fully enabled.** Settings persist regardless of whether the CLI exists, and the AI-assistance section directly above already carries the single `Claude Code CLI not found on PATH …` note. A second warning here would be duplicate chrome for one fact. No visual change. |
| loading | none. `UiSettings` is resolved before Settings can open; there is no async fetch in this section and therefore no skeleton. |
| error | none. Patching cannot fail visibly — `onChange` is App's debounced local-state + persist path shared with every other setting. A persist failure surfaces through App's existing toast, not here. |
| out-of-range typed value | the field **snaps** to the clamped value on `change`, silently. `min`/`max`/`step` are set on the inputs so the platform spinner and validation agree, and the hint states the meaningful bound in words. No red border, no error text: the user has not made a mistake worth an error, and the clamp is the same function Rust runs. |
| long content | the longest string here is hint 1 at ~300 characters; inside a 560px card at 11px that is ~5 lines and wraps normally. Nothing truncates, nothing is a list row. |

**When the Rust clamp disagrees with what was typed.** It cannot, by construction, and the contract
requires that to stay true: the section clamps with the same rules as `clampAiRunSettings`
(`src/ipc/mock/aiRunSettings.ts:69-90`, itself a mirror of Rust `clamp_ai_settings`) *before* calling
`onChange`, so the value App holds, the value Rust stores and the value shown are one value. If a
hand-edited settings file arrives out of range, Rust clamps on load and the control simply shows the
clamped number — the UI never reports "your file said 5, we are using 30", because by the time the
panel opens there is no other value to report. **Do not add a second clamp implementation in the
component; import the ranges and use the same shape.**

### 1.6 Keyboard and accessibility

- Tab order follows DOM order: repository-access button → Stream AI output → Stream partial replies →
  quiet-stop checkbox → its range → its number → fixed-time checkbox → its range → its number →
  turns range → turns number → spend checkbox → spend number → batch range → batch number. Disabled
  controls drop out of the order (that is why the fieldset carries `disabled`).
- Every `NumberSlider` already wires `htmlFor`/`id` and an `aria-label` on the range. The USD row
  must do the same (`htmlFor="settings-ai-budget"`).
- Hints are wired as descriptions, not orphan paragraphs. Each hint `<p>` has an `id`; its control
  carries `aria-describedby`:

| Control | `aria-describedby` |
|---|---|
| access button | `settings-ai-tools-hint` |
| Stream AI output | `settings-ai-streamlog-hint` |
| Stream partial replies | `settings-ai-partial-hint` |
| quiet-stop checkbox + its number input | `settings-ai-idle-hint` |
| fixed-time checkbox + its number input | `settings-ai-cap-hint settings-ai-nolimit-hint` (second id only while both limits are off) |
| turns number input | `settings-ai-turns-hint` |
| spend checkbox + USD input | `settings-ai-budget-hint` |
| batch number input | `settings-ai-bulk-hint` |

- The repository-access button's visible label is a **value**, so it needs an explicit name:
  `aria-label` = `Repository access — currently <current>. Activate to switch to <other>.` with
  `<current>`/`<other>` taken from the two strings in §1.4; `title` = the same sentence. (The shipped
  Theme / File lists / Panel density buttons have no such name; adding it there is a separate nit,
  §7-N2.)
- Hit targets: `.settings-number` 28px, `.btn-secondary` 32px, native checkboxes with a 13px label
  row and 10px margin — all ≥24px.
- Contrast, measured for this contract (dark / light, over `--bg-1` = the settings card):
  `--text-1` labels **13.6:1 / 15.5:1**; `--text-2` hints **7.3:1 / 7.4:1**; `--accent` focus ring as
  a graphic **4.6:1 / 4.4:1**. `--text-3` is used only by the pre-existing
  `.settings-section-title` / `.settings-section-desc` / `.settings-unit`, which this contract does
  not change; **no new `--text-3` text is introduced** (ui-reference §2).
- No colour carries meaning in this section — every state is a word or a control position.
- No motion at all: no transition, no reveal animation. Nothing to gate on
  `prefers-reduced-motion`.

### 1.7 Files

| File | New/edit | Budget | Responsibility |
|---|---|---|---|
| `src/components/SettingsAiRunSection.tsx` | NEW | ~230 | The eight controls. Owns only the three sentinel "resume value" numbers as local state; everything else is props + `onChange`. |
| `src/components/SettingsAiSection.tsx` | NEW | ~120 | The AI-assistance block moved out of `SettingsPanel.tsx:330-378`, plus job 2's autonomy hints and the renamed radio label. |
| `src/components/SettingsPanel.tsx` | EDIT | 395 → **~355** | Replace lines 330-378 with `<SettingsAiSection …/>` and `<SettingsAiRunSection …/>`; add the eight `UiSettings` fields to `SettingsPanelProps`. |
| `src/components/SettingsAiRunSection.test.tsx` | NEW | ~180 | §6.1 vitest list. |
| `src/styles.css` | EDIT | +~35 | The seven new classes in §4. |
| `src/App.tsx` | EDIT | +~10 | Pass the eight fields (all already in `useUiSettings`) into `SettingsPanel`; add keywords to `app.settings`. |

`SettingsAiRunSectionProps`: the eight values, `aiActive: boolean`, `onChange(patch: UiSettingsPatch)`.
Nothing else — it must not receive `aiAvailability` (§1.5 decides it has no opinion on the CLI).

---

## 2. Job 2 — consent and disclosure copy

### 2.1 What is wrong today

`src/App.tsx:1044-1048` states two things that are no longer true: that the payload is "the contents
of conflicted files" (the model now chooses additional repository files to read, and those bytes go
to Anthropic — contract §1a, audit M2) and that "no files are changed without your review" (under
`autoResolve`, `resolve_conflict_text` writes and stages with no review — audit H1, and P68f makes
that N files from one click).

The corrected copy states four facts in the order the user cares about: **what runs and where** ·
**what leaves this machine** · **what Claude cannot do** · **when Bonsai writes**. It does not
editorialise and it does not add a warning surface: the read grant is what makes conflict resolution
work, and a user who chose the feature is owed an accurate mental model rather than a scare.

### 2.2 `AiConsentDialog.tsx` — the whole component's copy, verbatim

New file `src/components/dialogs/AiConsentDialog.tsx`, rendering `ConfirmDialog`. Props:
`{ open: boolean; onConfirm(): void; onCancel(): void }`.

| `ConfirmDialog` prop | Value |
|---|---|
| `title` | `Enable AI features?` |
| `confirmLabel` | `Enable` |
| `confirmVariant` | `"primary"` — **changed.** The call site omits it today, so the button is `btn-danger`: a red primary for a reversible opt-in. Destructive styling is reserved for operations that lose work (BulkAiConfirmDialog's own comment says so). Consent is a choice, not a demolition; the honesty lives in the body text. |
| `busy` | `false` |
| `cardClass` | `"ai-consent-card"` (new optional prop — OQ-1) |

Body — four blocks, in this order. Block 1 is a bare `<div>` (inherits `.dialog-body`, 13px
`--text-1`); blocks 2-4 are `<p className="dialog-body-detail">` (12px `--text-2`, §4).

**1.**
> `Bonsai resolves conflicts with the Claude Code CLI installed on this machine, under your Claude subscription. Nothing is sent to Bonsai's own servers.`

**2.**
> `Claude receives the conflicting versions of the files you choose — and it can read other files in this repository while it works, which is what lets it match your surrounding code. Whatever it reads is sent to Anthropic with the request.`

**3.**
> `Its tools are read-only: it cannot write files, stage anything, or run commands, and reads outside this repository folder are refused. Refused reads appear in the AI activity dock.`

**4.**
> `Bonsai changes your files only when you apply a result. The exception is “Resolve automatically” under Settings → AI assistance, which writes and stages Claude's results with no review step.`

The trailing `Enable AI features?` in the current body is **deleted** — the dialog title already asks
it, and the duplicate wasted the last line of a now-longer body.

Geometry: `.dialog-card` is 360px, which turns this into a ~20-line column. Add the width variant
`.dialog-card.ai-consent-card { width: 420px; }` (the `settings-card` 560 / `onboarding-card` 520 /
`worktree-create-card` 520 idiom).

Behaviour is `ConfirmDialog`'s, unchanged and re-stated because it is part of the contract: initial
focus on **Cancel** (a stray Enter never enables), `Esc` and overlay click cancel, `role="dialog"
aria-modal="true" aria-label="Enable AI features?"`, focus restored by the caller. No motion.
At 420px the body wraps at ~62 characters/line; nothing scrolls at any supported window height, and
`overflow-wrap: anywhere` on `.dialog-body` already handles a narrow window
(`max-width: calc(100vw - 32px)`).

Contrast: `--text-1` on `--bg-1` **13.6:1 / 15.5:1**; `--text-2` on `--bg-1` **7.3:1 / 7.4:1**. Both
pass AA at 12px. The typographic characters `’` and `“ ”` are used exactly as written above; per house
style any JSX string containing them is wrapped as `{'…'}`.

### 2.3 The `autoResolve` caveat at the point of choice — `SettingsAiSection.tsx`

The autonomy fieldset moves across verbatim except for these three changes:

| Element | Before | After |
|---|---|---|
| radio 2 label | `Auto-resolve, then review` | `Resolve automatically` |
| new hint under radio 1, `id="ai-autonomy-propose-hint"` | — | `Each result opens as a proposal. Nothing is written to your files or staged until you apply it.` |
| new hint under radio 2, `id="ai-autonomy-auto-hint"` | — | `Marker-free results are written to your files and staged for you, with no review step. Results that still contain conflict markers open as proposals instead.` |

Both hints are always visible (not switched on the selected value): the consequence must be readable
*before* the choice, which is the entire point of moving it out of the consent dialog. Each hint is a
`<p className="settings-radio-hint">` sibling **after** its `<label>` (putting it inside the flex-row
label would break the layout), and each radio `<input>` gets `aria-describedby` pointing at its hint.
11px, `--text-2`, `margin: 2px 0 0 24px` — the 24px indent aligns the hint with the radio's label
text.

Also carried across unchanged: the enable checkbox, the `disabled={!aiActive}` fieldset, the three
CLI-status branches (`Checking for the Claude Code CLI…` / `aiAvailability.detail` /
`Claude Code CLI not found on PATH — install it and log in to use AI features`). The section
description is updated to name what the feature actually does with the repository:

| Element | String |
|---|---|
| `.settings-section-desc` | `Resolve merge conflicts with the local Claude Code CLI, under your Claude subscription. Claude can read files in this repository while it works — see Repository access below.` |

### 2.4 `BulkAiConfirmDialog.tsx` — the added sentence, and two corrections

The dialog cannot honestly state a run count: batching is computed in Rust from payload bytes
(`ai_resolve_stream.rs`, contract §6.3) and the frontend does not know the split. Duplicating that
arithmetic in React would put payload/Git logic in the frontend, which the architecture forbids. So
the copy commits to "one or more", which is true for every split, instead of the shipped "once",
which is false as soon as a batch splits.

| Element | Before | After |
|---|---|---|
| lead `<div>` | `Send {count} conflicted {file/files} to Claude in **one AI run**, so it can reason across them?` | `Send {count} conflicted {file/files} to Claude so it can reason across them?` |
| note 1 (`autoResolve`) | text unchanged | text unchanged, class → `dialog-body-detail` |
| note 1 (`proposeReview`) | text unchanged | text unchanged, class → `dialog-body-detail` |
| **new** note, `dialog-body-detail`, always | — | `Claude can also read other files in this repository while it works; whatever it reads is sent to Anthropic with the request.` |
| note 2 | `This runs the Claude CLI once and uses your Claude quota. You can stop it at any time with Cancel all.` | `Depending on how much text this adds up to, Bonsai makes one or more Claude runs, one after another, using your Claude quota. Cancel all stops the rest.` |

Order in the body: lead → path list (`.confirm-name-list`, unchanged; `+N more` stays on
`.dialog-body-note` because it *is* decorative) → autonomy note → repo-read note → run-count note.
`confirmVariant="primary"` and the `Resolve all with AI` label are unchanged, and the dialog keeps its
360px width — it already lists paths and reads fine; do not widen it.

### 2.5 Files for job 2

| File | New/edit | Budget | Responsibility |
|---|---|---|---|
| `src/components/dialogs/AiConsentDialog.tsx` | NEW | ~60 | The four copy blocks + the `ConfirmDialog` wiring. |
| `src/components/dialogs/AiConsentDialog.test.tsx` | NEW | ~70 | §6.2 list. |
| `src/App.tsx` | EDIT | −14 / +6 | Replace the inline `ConfirmDialog` at 1036-1049 with `<AiConsentDialog open={consentOpen} onConfirm={handleConfirmConsent} onCancel={() => setConsentOpen(false)} />`. **No other change in this file** — it is owned by a concurrent refactor. |
| `src/components/ConfirmDialog.tsx` | EDIT | 83 → ~86 | Optional `cardClass?: string` appended to `.dialog-card` (OQ-1). |
| `src/components/dialogs/BulkAiConfirmDialog.tsx` | EDIT | 73 → ~80 | §2.4. |
| `src/components/SettingsAiSection.tsx` | NEW | ~120 | §2.3 (the same new file as job 1's extraction). |

---

## 3. Job 3 — the ask block, as shipped (amendment to `P68e-ai-activity-dock.md`)

**Status: this section is the authoritative text for P68e §4.1/§4.2/§4.5.** It is written here rather
than spliced into `P68e-ai-activity-dock.md` because that file is 1064 lines and the ui-designer's
toolset can only rewrite a file whole — reproducing 1064 lines to change 30 risks truncating a
canonical contract. **Action for the orchestrator / docs-curator: splice §3.1–§3.4 below into P68e in
place (they are drop-in replacements with anchors), and while there, consider splitting that
1064-line document — it is twice the house soft limit.** Until that happens, P68e §4.1/§4.2 are
superseded by this section and P68e should get a one-line pointer at §4.

### 3.1 The token question — verdict: `--text-1` is right, and it needs one glyph

The implementer used `--text-1` for the attribution and guard lines, citing §12-F2. **That is
correct, and it should not be changed to `--warning`.** Reasoning, in order of weight:

1. `--warning` as small text over its own 14% tint measures **3.47:1 in light theme** (ui-reference
   §2, P68e §12-F2). A security sentence that fails AA is worse than one that is merely calm — it is
   one a user with low vision cannot read at all.
2. The ask block is **already** warning-tinted with a `--warning` left border and a filled `?` glyph.
   Yellow words inside a yellow-tinted panel add no salience; they only reduce contrast.
3. Cry-wolf is a real cost. `? Needs you` is a **normal** state of a healthy interactive run — the
   feature the user asked for. If the routine question block is styled like a security incident, the
   real signal (a run asking for a token) becomes invisible by habituation.

What the shipped code *is* missing is **salience**, which comes from weight and shape, not hue. Two
additions, both AA-safe:

- `.ai-dock-ask-guard` renders at **12px/600 `--text-1`** (not 11px/400) with a leading
  `aria-hidden` glyph `<span class="ai-dock-ask-guard-glyph">⚠</span>` coloured `--warning`. The
  glyph is a graphic (≥3:1 bar: 5.4:1 dark / 3.6:1 light over the tint); the words stay `--text-1`
  (9.1:1 / 12.1:1 over the tint). This is exactly the ui-reference §2 rule — warning hue for the
  glyph, `--text-1` for the words.
- `.ai-dock-ask-attrib` renders at **11px/600 `--text-1`**, no glyph, no hue. It is a label for the
  block below it, so size and weight separate it from the 13px question without competing with the
  guard line.

Net: no yellow words anywhere, no invisible words anywhere, one hue used as a graphic. Both new
selectors are colour-token aliases only — **no new custom property.**

### 3.2 Replacement for P68e §4.1 "Visual distinction"

> - Container `.ai-dock-ask`: `border-top: 1px solid color-mix(in srgb, var(--warning) 40%,
>   var(--border)); background: color-mix(in srgb, var(--warning) 14%, var(--bg-1));` padding
>   `var(--ai-dock-ask-pad)`, `display: flex; flex-direction: column; gap: 8px`.
> - `.ai-dock-ask-glyph`: 16px circle, `background: var(--warning); color: var(--bg-0);` content
>   `?`, `aria-hidden`. **Contrast checked:** `--bg-0` on `--warning` = 6.4:1 dark, 4.8:1 light.
> - `.ai-dock-ask-label`: `Claude needs your answer` — 11px/600, uppercase, letter-spacing 0.08em,
>   **`--text-1`** (not `--warning`: `--warning` as small text is only 3.5:1 in light theme —
>   §12-F2).
> - **`.ai-dock-ask-attrib` (AS BUILT, security audit M3).** Copy, verbatim:
>   `Claude wrote this — Bonsai did not:`. 11px/**600**, `--text-1`, `margin: 0`. Sits between the
>   label and the question and renders **only when a question exists**. It is attribution, not
>   decoration: the question text is model output and is reachable by an attacker without a
>   jailbreak (a conflicted file whose *both* sides begin with the literal `BONSAI_NEEDS_INPUT:`
>   line merges faithfully into one), so the block must never read as Bonsai asking.
> - `.ai-dock-ask-question`: the question verbatim, UI font 13px `--text-1`, `white-space:
>   pre-wrap`, `max-height: 96px; overflow: auto`, user-selectable. Rendered as **plain text only** —
>   never as markup, never as a link, never interpolated into another string.
> - **`.ai-dock-ask-guard` (AS BUILT, security audit M3).** Copy, verbatim:
>   `Bonsai never asks for passwords or tokens. Don’t paste secrets here.` 12px/**600**, `--text-1`,
>   `id="ai-dock-ask-guard"`, preceded by an `aria-hidden` `⚠` in
>   `.ai-dock-ask-guard-glyph { color: var(--warning) }`. It is **fixed chrome the model cannot
>   influence** and is rendered **even when `question === null`**, so a request for a secret is
>   visibly refused by Bonsai itself rather than sitting unanswered. Do not fold this sentence into
>   the question string, do not template any part of it, and do not hide it to save a line. Token
>   verdict and its reasoning: §3.1 of `P68g-ui.md`.
> - When any run is `awaitingInput`, the bar gets `data-attention="true"` → the same 14% `--warning`
>   background, so the bar reads as "needs you" without expanding. No blinking, ever.
>   **AS BUILT (N7): the attribute is set regardless of `collapsed`, not only when collapsed** — the
>   tint is on `.ai-dock-header`, which is the same row in both states, and scoping it to the
>   collapsed bar would make the header lose its "needs you" colour the moment the user expands it to
>   answer. Harmless superset; documented so it is not "fixed" back.

Vertical order inside the block (part of the contract): head row (glyph + label) → attribution →
question → guard → reply row → keyboard hint. The guard sits **between** the question and the input
deliberately: it is the last thing read before typing.

### 3.3 Replacement for P68e §4.2 "Reply control"

> `<textarea class="ai-dock-ask-input">` — UI font 13px (prose, not code), `--bg-2`, 1px
> `--border`, radius 6px, padding 6px 8px, `rows={2}`, min/max height per §1.7, `resize: none`,
> autogrow up to max. Placeholder: `Type your answer for Claude…`.
> `aria-label="Your answer to Claude"`.
> **`aria-describedby="ai-dock-ask-guard ai-dock-ask-hint"` (AS BUILT, M3) — a two-id list, in that
> order.** The guard id comes **first** so a screen-reader user hears "Bonsai never asks for
> passwords or tokens" *before* the keyboard hint; a single-id `aria-describedby` pointing only at the
> hint would leave the anti-phishing line unannounced, which is the whole control. Both ids must
> exist whenever the textarea does — the guard `<p>` is unconditional, so they do.
> `.ai-dock-send` `.btn-primary`, height `var(--ai-dock-ctl-h)`, padding 0 12px, label `Send`.
> `.ai-dock-ask-hint`: `Enter sends · Shift+Enter for a new line`, 11px `--ai-dock-meta`,
> `id="ai-dock-ask-hint"`.

### 3.4 New P68e §4.5 — untrusted text handling (insert after §4.4)

> ### 4.5 The question is untrusted input (security audit M3)
>
> The dock renders one string it did not author: `question`. `ai::stream::sentinel_question` fires on
> a line beginning `BONSAI_NEEDS_INPUT:`, and both sides of a conflicted file can carry that line, so
> a faithful merge reproduces it without any jailbreak. The contract's original A9 argument
> ("impossible in practice") holds for accidents and fails for adversaries. Three halves of the fix,
> and the UI owns the third:
>
> 1. **Rust** requires the sentinel line to be the only non-empty line and strips control characters.
> 2. **Rust** never logs tool results (`type:"user"` lines are reduced to a byte count — A11).
> 3. **This component** attributes the text (§4.1 `.ai-dock-ask-attrib`), states a fixed
>    non-model-controlled refusal (§4.1 `.ai-dock-ask-guard`), announces that refusal to screen
>    readers first (§4.2 `aria-describedby` order), and renders the question as plain text with no
>    markup, no link detection and no string interpolation.
>
> Invariants for any future change to `AiActivityAsk.tsx`:
> - the guard line is never conditional on `question`;
> - the guard line is never composed with model text;
> - the attribution line is never dropped when a question exists;
> - the announcement region (§11) never reads the question aloud — it announces
>   `Claude needs your answer about <path>`, i.e. Bonsai's own words plus a path the user chose;
> - nothing in this block links, executes, copies-to-clipboard or auto-fills anything.
>
> **Not designed here, deliberately:** the reviewer of a *proposal* still reads flat full-file text
> rather than a diff (audit M5), so "you review it" is weaker than it sounds for a 1500-line file.
> That is follow-up item 8, and no copy in this contract claims otherwise.

### 3.5 Two P68e table rows to update in the same splice

| P68e location | Change |
|---|---|
| §9 file table, `AiActivityAsk.tsx` row | budget `~110` → **`~130`**; responsibility becomes `Attribution + guard line + question + reply textarea + Send; owns the draft and all keyboard handling; exposes an imperative focus() via forwardRef.` |
| §11 a11y table, `.ai-dock-ask-input` row | `aria-describedby=".ai-dock-ask-hint"` → **`aria-describedby="ai-dock-ask-guard ai-dock-ask-hint"` (guard first — §4.2)** |
| §11 contrast table | add `--text-1` 600 on 14% warning tint (guard + attribution) **9.1:1 / 12.1:1**, and `--warning` glyph on the same tint (graphic, ≥3:1) **5.4:1 / 3.6:1** |
| §1.5 wireframe | add the two new lines between the question and the textarea so the ASCII matches the DOM |

---

## 4. New CSS classes (no new tokens)

All seven are geometry or a re-point at an **existing** token. **No new custom property is introduced
in `:root` or `[data-theme='light']`**, so both themes work from one rule set.

```css
/* P68g: must-read secondary text in a dialog body. .dialog-body-note is --text-3
   (3.38:1 dark / 2.96:1 light — below AA, ui-reference §2) and stays for decorative
   lines like "+N more"; consent facts and the autoResolve caveat use this. */
.dialog-body-detail {
  margin-top: 8px;
  font-size: 12px;
  color: var(--text-2);
}

/* Wider consent body so four short facts do not become twenty lines at 360px. */
.dialog-card.ai-consent-card {
  width: 420px;
}

/* One inert switch for a whole settings section (the .settings-radio-group
   :disabled idiom, applied at section scope). */
.settings-section-fields {
  margin: 0;
  padding: 0;
  border: none;
}
.settings-section-fields:disabled {
  opacity: 0.5;
}

/* Per-control explanation. Same geometry as .settings-config-hint, --text-2 instead
   of --text-3 because these sentences must be read, not glanced at. */
.settings-hint {
  margin: 2px 0 10px;
  font-size: 11px;
  color: var(--text-2);
}

/* Hint under a radio, aligned with the radio's label text. */
.settings-radio-hint {
  margin: 2px 0 0 24px;
  font-size: 11px;
  color: var(--text-2);
}

/* Indent a numeric row under the checkbox that enables it. */
.settings-indent {
  margin-left: 24px;
}

/* Two decimals plus a unit do not fit .settings-number's 56px. */
.settings-number-wide {
  width: 72px;
}
```

Job 3 additionally needs `.ai-dock-ask-attrib` (11px/600, `--text-1`), `.ai-dock-ask-guard`
(12px/600, `--text-1`, `display: flex; gap: 6px; align-items: baseline`) and
`.ai-dock-ask-guard-glyph` (`color: var(--warning)`, `flex: none`) in the existing
`/* ---------- P68e: AI activity dock ---------- */` block — **+~12 lines, in that section, not a new
one.**

`ui-reference.md` §8 gains one line recording `.dialog-body-detail` as the house class for must-read
secondary dialog text, since that is a genuinely new (if small) pattern; nothing else in the design
system changes.

---

## 5. Harness states (`VITE_MOCK_IPC=1`, `pnpm dev`)

Settings and both dialogs are plain DOM, so all of this is verifiable in the browser harness. The
mock settings blob is `localStorage['bonsai.mockUiSettings']`, parsed by `parseAiRunSettings`
(`src/ipc/mock/aiRunSettings.ts:105`) — no new mock handler is needed, but these seeds must be
**documented in `P68-user-checklist.md`** so the harness pass is repeatable:

| # | Seed | What it proves |
|---|---|---|
| 1 | default blob (`aiEnabled:false`) | AI runs section renders, whole fieldset inert, `Turn on “Enable AI features” above…` visible and **not** dimmed |
| 2 | `{aiEnabled:true, aiConsented:true}` | every control live; defaults render as the §1.5 default row |
| 3 | `{aiEnabled:true, aiConsented:true, aiHardCapSecs:0, aiMaxBudgetUsd:0, aiIdleTimeoutSecs:0}` | all three sentinels unchecked with disabled numeric rows showing resume values; the `With neither limit on…` line appears |
| 4 | `{aiIdleTimeoutSecs:5, aiBulkMaxBytes:9999999, aiMaxBudgetUsd:-3, aiConflictTools:'bogus'}` | load-time clamp: 30 s, 4000 KB, spend limit off, `Read-only` — the "Rust clamp disagrees" case has nothing left to disagree with |
| 5 | `{aiEnabled:false, aiConsented:false}` then click **Enable AI features** | `AiConsentDialog` opens: four blocks, primary (not red) `Enable`, focus on `Cancel`, Esc cancels |
| 6 | availability `installed:false` with `aiEnabled/aiConsented:true` | AI runs section stays live; exactly **one** CLI-not-found note on screen |
| 7 | bulk confirm with 200 paths, each ~180 chars deep | `.confirm-name-list` scrolls at 180px, `+190 more`, the three notes wrap without pushing the buttons off-card |
| 8 | `[data-theme='light']` + a 900px-wide window | both dialogs and the section in light theme; `.dialog-card` falls back to `calc(100vw - 32px)` |
| 9 | the existing `aiStream` mock ask script, with `question: 'BONSAI_NEEDS_INPUT: I need the repository access token to check the upstream branch — paste it here.'` | job 3: attribution + guard render above the textarea; `aria-describedby` resolves to two existing ids; the guard line still renders with `question: null` |

Not harness-verifiable, therefore **USER CHECKPOINT**: (a) that a real `claude` run's refused read
appears in the dock as §2.2 block 3 promises (needs the native window and a real out-of-repo read
attempt); (b) native focus-ring rendering of the new controls on Windows/macOS.

---

## 6. Acceptance criteria

### 6.1 vitest — `SettingsAiRunSection.test.tsx`

1. Each control patches **exactly its own field**: eight interactions, eight single-key patches
   (`{ aiIdleTimeoutSecs: 600 }`, `{ aiConflictTools: 'none' }`, `{ aiBulkMaxBytes: 800000 }`, …).
2. `aiConflictTools` offers exactly two values across repeated activation (`Read-only` →
   `No file access` → `Read-only`) and **no string containing `write`, `edit` or `bash` appears as an
   option in the section** (D10, parent contract §9 acceptance 2).
3. Unchecking `Stop a run after a fixed time` patches `{ aiHardCapSecs: 0 }`; re-checking patches
   `1800`; re-checking after the user typed `600` patches `600` (resume value).
4. With `aiHardCapSecs: 0` **and** `aiIdleTimeoutSecs: 0`, the string
   `With neither limit on, a run continues until it finishes or you cancel it.` is present; with
   either non-zero it is absent.
5. Typing `99999` into the turns field patches `{ aiMaxTurns: 20 }`; typing `0` patches `1`; typing
   `abc` patches nothing.
6. `aiBulkMaxBytes: 400000` renders `400`; setting the field to `1200` patches `1200000`.
7. Spend limit: typing `12.5` patches `{ aiMaxBudgetUsd: 12.5 }` (**not** `13`, **not** `12`);
   typing `500` patches `100`.
8. `aiActive: false` ⇒ every input and the access button report `disabled`, and
   `Turn on “Enable AI features” above to change these.` is present.
9. Every hint `id` referenced by an `aria-describedby` exists in the DOM (no dangling reference).

### 6.2 vitest — copy tests

10. `AiConsentDialog.test.tsx`: the four blocks are present verbatim; the confirm button carries
    `btn-primary`, not `btn-danger`; initial focus is `Cancel`; Esc calls `onCancel`.
11. The strings `no files are changed without your review` and `contents of conflicted files` appear
    **nowhere** in `src/` — audit M2's regression guard.
12. `BulkAiConfirmDialog.test.tsx`: the body contains `one or more Claude runs` and does **not**
    contain `one AI run` or `runs the Claude CLI once`; the repo-read sentence is present in both
    autonomy branches.
13. `SettingsPanel.test.tsx`: `Resolve automatically` is the second autonomy label,
    `Auto-resolve, then review` is gone, and both autonomy hints render.
14. `AiActivityAsk` (existing test file): the guard line renders with `question: null`; the textarea's
    `aria-describedby` is exactly `ai-dock-ask-guard ai-dock-ask-hint` in that order.

### 6.3 Design gate (ui-designer re-review of the diff)

15. No hardcoded hex, `rgb()` or `hsl()` in the new CSS or components.
16. `--text-3` is not used for any new text.
17. Every new interactive element has a `:focus-visible` ring and an accessible name.
18. `SettingsPanel.tsx` is **shorter** than before; no new file exceeds ~230 lines.

---

## 7. Flagged for the orchestrator

**OQ-1 — how the 420px consent card gets its class.** `ConfirmDialog` has no class prop. Options:
(a) add `cardClass?: string` to `ConfirmDialogProps` (~2 lines, reusable by every future long
dialog); (b) `AiConsentDialog` renders its own overlay/card and duplicates the focus/Esc logic.
**Recommendation: (a).** (b) forks a shipped focus-trap idiom for a width.

**OQ-2 — `aiIdleTimeoutSecs` / `aiHardCapSecs` unit.** Specified in **seconds**, matching the field
name and the clamp exactly. Minutes would read better ("5 minutes") but the 30 s minimum becomes
`0.5`, and any conversion invites a value the Rust clamp moves. **Recommendation: keep seconds; the
hint carries the human reading ("300 seconds is five minutes").** If minutes are wanted, the clamp
minimum must change in Rust first.

**OQ-3 — audit L4 (idle 0 + cap 0 = a run nothing reaps).** This contract makes that state reachable
from the UI, deliberately, and describes it in one factual sentence. It does **not** refuse the
combination, because "no hard timeout, the user cancels" is a locked product decision and a settings
panel that blocks a documented sentinel is worse than one that explains it. If the combination should
be refused, that is a Rust-side change to `clamp_ai_settings` and the UI would then show the refusal
as the checkbox simply not staying unchecked — say so and I will spec it.

**OQ-4 — P68e cannot be patched in place by this agent.** See §3's status note: 1064 lines, whole-file
writes only. §3.1–§3.5 are drop-in with anchors. Recommend the splice plus a `docs-curator` split of
that document.

**N1 (nit, pre-existing)** — `.settings-number:focus` recolours its border instead of drawing the
house 2px `--accent` ring, and uses `:focus` rather than `:focus-visible`
(`styles.css:1030-1033`). App-wide; a one-rule fix, but it changes every existing settings slider, so
it is its own increment.

**N2 (nit, pre-existing)** — the Theme / File lists / Panel density toggle buttons have no accessible
name beyond their current value, so a screen-reader user hears "Dark" with no idea what it controls.
The new access button fixes this for itself; retro-fitting the other three is a 3-line follow-up.

**N3 (noted, not designed)** — audit M5: a proposal is reviewed as flat full-file text, not a diff, so
"you apply the result" is weaker than the consent copy makes it sound for a 1500-line file. Nothing in
§2 or §3 claims the review is a diff. Rendering proposals as a diff is follow-up item 8, and the copy
here will not need to change when it lands.

**Design-vs-security tension, resolved.** Three places wanted a warning colour and two did not get
one. (a) The consent dialog: no tint, no icon — a user opening an opt-in dialog is not in an incident,
and four accurate sentences do more than a yellow band. (b) The `autoResolve` hint: plain `--text-2`
at the point of choice beats a red warning the user learns to skip; the words carry "with no review
step", which is the whole fact. (c) The dock's guard line: `--text-1` words with one `--warning`
glyph, because `--warning` as text is 3.47:1 in light theme — it would be simultaneously alarming and
unreadable. In all three, hue is used only as a graphic (≥3:1) and never as the carrier of meaning.
