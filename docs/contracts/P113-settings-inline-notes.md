# P113 — Settings inline outcome notes (the toast sweep)

**Status:** spec complete, awaiting implementation.
**Owner:** ui-designer. Implemented by `senior-dev`. No application code is edited by this contract.
**Authority:** user ruling #24, 2026-09-14 — *sweep every call site.* The narrower "fix only the F6
delete outcome" option was recommended and **rejected by the user**; it is settled and is not
re-argued here.
**Supersedes:** `P91-privacy-copy-ui.md` §6.11.6 (flagged MUST-FIX, deliberately unspecced there).
That passage stays readable with this pointer, and its two errata are carried in §1.2 below.
**Related:** `ui-reference.md` §12.2 (row anatomy), §12.3.4 (the idle-empty live-region shape),
§12.13 (pickers — its Settings-toast bullets are replaced by a pointer), **new §12.14** (the
canonical rule, written in this pass), `P112` sub-increment 4 (external-tool picker — see §13.2).

---

## 1. What is wrong, measured

### 1.1 The channel is not dim, it is unreachable

`SettingsPanel` renders the Settings card *inside* `.dialog-overlay`. Measured in the harness at
1280×720 with a toast raised from the Developer page:

| Quantity | Measured |
|---|---|
| `.toast-stack` computed `z-index` | **90** |
| `.dialog-overlay` computed `z-index` / background | **100** / `rgba(0,0,0,0.45)` |
| Toast rect | 360 × 37 at (908, 52) |
| Settings card rect | 880 × 656 at (200, 32) |
| Horizontal overlap | **172 of 360 px** (independently derived and then measured — identical) |
| Vertical overlap | the toast's full 37 px is inside the card's vertical extent |
| `document.elementFromPoint(toast centre)` | **`dialog-overlay <DIV>`** — `onTopIsToast: false` |

So the toast is **not merely washed out: it is not the hit-test target anywhere on its own box, and
its ✕ dismiss button cannot be clicked at all.** A message the user cannot dismiss and (at common
widths) cannot fully read is not a delivery channel.

`ui-reference.md:2377` already ruled this case — *"A Settings-surface error is inline, never a
toast"* — and every toast on this surface violated it.

### 1.2 Two errata to P91 §6.11.6

1. **"Pure reuse of an existing signed recipe" was true of the design and false of the code.**
   `.settings-row-note` exists and is widely used (`src/styles/settings-primitives.css:180`), but
   **`.settings-row-note--warn` has zero users and no CSS rule anywhere in `src/`.** The recipe is
   *written* in §4 of this contract, with measured ratios, before it is reused.
2. **The count is 10, not 17.** The 17 came from a `wc -l` that also counted `usePushToast()`
   declarations and `[pushToast]` dependency arrays. The exhaustive verified list is §6.
   *Third erratum, found while writing this contract:* `DevCategory.tsx:96` is the **reveal**
   failure ("Couldn't open the logs folder…"), **not** the delete outcome. The delete outcome is
   `:149`. The count of 10 is unaffected.

### 1.3 A fact the measurement did not cover

`SettingsAccountsSection.confirmRemove`'s failure branch (`:95-98`) sets `removing = false` and
**does not clear `removeTarget`** — so **the Remove confirm dialog stays open on failure.** A note
placed in the host group would be behind that dialog, i.e. the same defect one layer up. That call
site therefore lands in the dialog (§6, row 8; §3.3).

---

## 2. Scope

**In:** the ten `pushToast` call sites listed in §6; the two CSS recipes; one shared component and
one shared hook; the live-region arithmetic for both sections; a lint guard against relapse; the
mock knobs needed to see all ten in the harness.

**Out (stated so review does not widen the diff):** `.toast-stack` / `.dialog-overlay` z-index
(unchanged), the toast component itself, the strings (§12), any non-Settings surface, and
`errorMessage()` mapping for the Accounts errors (§12.2, amendment A1).

**Not a redesign of the confirm dialogs.** P91 §6.11.6's constraint is carried forward verbatim in
new terms: in `runDelete`, the outcome is reported and the dialog closed **in the same synchronous
continuation** (no `await` between them), so the note appears in the same React commit that unmounts
the dialog. An `await` inserted there would park the note behind `.dialog-overlay` for the gap.

---

## 3. The central question: do success outcomes get an inline note?

**Decision: yes. All ten outcomes are inline. One component, two tones.**

### 3.1 Why

1. **Tone does not change stacking.** Four of the ten are successes, and a success toast is behind
   the same 45 % scrim and the same failed hit test as an error toast. "Readable but dim" is false at
   1280 (172 px clipped) and false for the ✕ at every width.
2. **The decisive argument: one action, one place.** `deleteResultToast` computes `tone` at
   **runtime** from `LogsDeleteResult` — the same button press yields `success` or `error`. Routing by
   tone would put the result of one button in two different locations on screen depending on what the
   backend returned. That is not a tolerable interaction model, and no wording can rescue it.
3. **One of the successes carries non-redundant information.** The export success names the folder —
   `exports/`, the sibling of the `logs/` folder that `Show in folder` reveals. P91 records that the
   toast was "the only place that gap is closed". That sentence must be visible, and inline it lands
   directly beside the `Kept in {dir}` state note it contrasts with — strictly better than the toast.
4. **A success must not look like an alert.** Hence the second, *quieter* variant: no tint, no bar,
   no `role="alert"`, polite announcement only (§4.2). The tone split is carried by words plus the
   presence/absence of the bar, never by hue alone.

### 3.2 Options rejected

| Option | Rejected because |
|---|---|
| Successes stay toasts | Same broken channel; and it splits one action's outcome across two locations (§3.1.2). |
| Successes get no message at all (rely on the visible state change) | Loses the export folder sentence and the delete's counts/freed-bytes; and `Connected to {host} as {login}.` is the only confirmation that a *token* was stored, which the list cannot show. |
| A `--success`-tinted note | New chrome for the least consequential four of the ten, louder than the row's own state note, and hue would be doing work the words already do. |

### 3.3 The one home that is not a row note

The **remove-account failure** goes to a `.dialog-error` inside the still-open `ConfirmDialog`
(§1.3). This is **not an exception to the ruling** — it is inline, it is not a toast — only the home
differs, because the failure path deliberately leaves the dialog open so Remove can be retried in
place. `.dialog-error` is a genuinely signed recipe with 13 existing call sites, so this is the reuse
P91 §6.11.6 wrongly claimed for `--warn`.

*Alternative considered and rejected:* close the dialog on failure and show a host-group note. It
loses the in-place retry and adds a state transition on the failure path, which is the path least
worth complicating.

---

## 4. The two recipes

### 4.1 Error tone — `.settings-row-note--warn` (new CSS, written here)

Added to `src/styles/settings-primitives.css` **immediately after `.settings-row-note`**
(currently `:180-185`; file is 401 lines → ~425, under the limit). Not a new file: the base class,
and the only rule the modifier can lose a specificity fight to, live there.

```css
/* P113 §4.1 — an ACTION OUTCOME that failed, in a row's help slot or an account
   group. The §10.2 bar recipe (`.settings-row--sensitive`, `.dev-warning-bar`)
   plus its own 12% tint. --text-1, NOT --warning: --warning as 12px text is
   3.5:1 in light. --warning, NOT --danger: the operation did not happen, nothing
   was lost (the §12.3.4 rule). Meaning is carried by the words and the bar; hue
   is third, never alone. */
.settings-row-note--warn {
  margin: 4px 0 0;
  padding: 6px 8px 6px 12px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--warning) 12%, transparent);
  box-shadow: inset 3px 0 0 var(--warning);
  color: var(--text-1);
  overflow-wrap: anywhere;
}

/* The §12.3.4 shape: the element is PERMANENTLY PRESENT with empty text, because
   a live region (or a describedby target) mounted in the same tick as its text is
   not announced. `:empty` collapses the chrome; `display: none` would cost the
   announcement and is forbidden here. */
.settings-row-note--warn:empty,
.settings-row-note--result:empty {
  margin: 0;
  padding: 0;
  background: none;
  box-shadow: none;
}

/* P113 §4.2 — an action outcome that SUCCEEDED. --text-1 is the whole recipe: it
   is the line the user came for, one step brighter than the --text-2 state note
   beside it, and it must not read as an alert. No tint, no bar, no glyph. */
.settings-row-note--result {
  margin: 4px 0 0;
  color: var(--text-1);
  overflow-wrap: anywhere;
}

/* The account group is `display: flex; gap: 8px` (settings-legacy-sections.css:287),
   so a note that is its flex child must not add its own top margin. */
.settings-account-group > .settings-row-note {
  margin: 0;
}
```

`overflow-wrap: anywhere` is required, not decorative: four of the strings append raw
`errorMessage(e)`, which can be a space-free keychain/OS token (§12.2).

### 4.2 Success tone — `.settings-row-note--result`

Above. Both variants are applied **on top of** the base `.settings-row-note` (class list
`settings-row-note settings-row-note--warn`), so typography (12px, `max-width: 56ch`) comes from one
place.

### 4.3 Measured contrast — both themes

Method: alpha-composite 12 % of the token over the base, then WCAG 2.1 relative luminance. The base
is **`--bg-0`** for every placement in this contract — `.settings-pane` is `var(--bg-0)`
(`settings-shell.css:290`) and `.settings-account-group` declares no background of its own.
Composite tint: **rgb(45, 41, 31)** dark / **rgb(243, 237, 224)** light.

| Pair | Dark | Light | Bar | Verdict |
|---|---|---|---|---|
| `--text-1` on 12 % `--warning` tint over `--bg-0` | **12.03:1** | **14.16:1** | text, AA 4.5 | pass |
| `--warning` 3 px bar vs its own 12 % tint | **6.46:1** | **4.17:1** | graphics, 3.0 | pass |
| `--warning` 3 px bar vs `--bg-0` | **7.92:1** | **4.87:1** | graphics, 3.0 | pass |
| `--text-1` on `--bg-0` (`--result`) | **14.74:1** | **16.52:1** | text, AA 4.5 | pass |
| `--danger-strong` on `--bg-1` (`.dialog-error`, unchanged) | **7.62:1** | **6.01:1** | text, AA 4.5 | pass |

**Method validation:** the same arithmetic reproduces §2's two signed `--warning` bar figures
(6.46 / 4.17 and 7.92 / 4.87) **exactly**, which is why the two new rows can be trusted.

The tint-to-`--bg-0` step is deliberately **not** held to 3:1 — the bar is the edge carrier and it
clears 3:1 in both themes. That is the §2 precedent for all 12 % tints.

**P107's serialization trap applies to verification:** Chrome serialises
`color-mix(… transparent)` as `rgba()` **with alpha**, so AC4 must composite the computed
`background-color` over the ancestor's opaque colour before computing a ratio. Reading the `rgba`
alone understates it.

### 4.4 Disambiguation rule (prevents the next drift)

The Accounts surface already has a 12 % `--warning` block: `.forge-reauth-banner`
(`forge-account.css:163`, bordered, with a ⚠ glyph, rendered from `ForgeConnect.tsx`). Both stay.
The rule, recorded in `ui-reference` §12.14:

- **Standing state** that must be noticed on arrival → **bordered banner** (`.forge-reauth-banner`,
  `.error-banner`).
- **Result of an action the user just took** → **barred note** (`.settings-row-note--warn`).

---

## 5. Component decomposition and file paths

| File | New? | Role | Size |
|---|---|---|---|
| `src/components/settings/SettingsOutcomeNote.tsx` | **new** | Presentational `<p>`. Props: `slot: string`, `id: string`, `outcome: { tone: 'success' \| 'error'; text: string } \| null`. Renders `<p id={id} data-outcome-note={slot} className="settings-row-note settings-row-note--warn\|--result">{text}</p>`, **always**, with `''` when `outcome === null`. Carries **no** `aria-live`, **no** `role`. **The `{text}` expression must be the element's only child with no surrounding JSX whitespace** — a stray newline inside the `<p>` is a text node, `:empty` stops matching, and the idle chrome silently stops collapsing (AC5). | ~45 lines |
| `src/components/settings/useOutcomeNotes.ts` | **new** | The slot bookkeeping both surfaces need: `begin(key)` clears a slot when an operation that reports into it **starts**; `report(key, tone, text, announceText = text)` sets the slot and sets the section's single announcement; `notes: Map<string, Outcome>`; `announce: string`. Centralising it is what makes the one-live-region rule structural rather than per-call-site discipline. | ~50 lines |
| `src/components/settings/categories/DevCategory.tsx` | edit | Drop `usePushToast`; five pushes become `report(...)`. Its existing `announce` state is replaced by the hook's. **Container only** — no render added. | 223 → ~225 |
| `src/components/settings/SettingsDevLogsSection.tsx` | edit | Two `SettingsOutcomeNote`s in the two `hint` slots; two new ids; `aria-describedby` composition on three buttons. | 166 → ~185 |
| `src/components/settings/SettingsAccountsSection.tsx` | edit | Hook; host-bound `onOpenUrl` closures; `removeError` state + the dialog's `.dialog-error`; the section slot in the `accounts.add` row. | 193 → ~240 |
| `src/components/settings/SettingsAccountHostGroup.tsx` | edit | New `outcome` prop + one `SettingsOutcomeNote` + `aria-describedby` composition. | 101 → ~115 |
| `src/styles/settings-primitives.css` | edit | §4's three rules, after `:185`. | 401 → ~425 |
| `src/styles/dialogs.css` | edit | `.dialog-error:empty { margin: 0 }` only. **Safe on a 13-site shared rule** because every existing `.dialog-error` call site renders the element *conditionally* — none of them is ever empty, so the new rule can only ever match the one site added here. | +3 |
| `eslint.config.js` | edit | §13.1's guard block. | +14 |
| `src/ipc/mock/handlers/obs.ts`, `handlers/forge.ts`, `handlers/external.ts` | edit | §14's knobs. | +~25 total |

**Why a new component rather than inline `<p>`s:** the element must be *permanently mounted with
empty text* and must never carry `aria-live` — two rules that are invisible at a call site and were
already broken once on this surface. One file makes them reviewable in one place, and P112-4 gets
them for free.

`SettingsAccountCard.tsx` is **not touched** (§10.3).

---

## 6. Placement — all ten call sites

Two slots in Dev, one per host plus one section slot in Accounts, one dialog error.

| # | Call site | String source | Tone | Lands in |
|---|---|---|---|---|
| 1 | `DevCategory.tsx:96` reveal failed | literal | error | **`dev.logs` row** help slot, id `dev-logs-outcome` (slot key `dev.logs`) |
| 2 | `:115` export succeeded | literal | success | **`dev.logs` row** help slot, same slot |
| 3 | `:118` export failed | `exportErrorText()` | error | **`dev.logs` row** help slot, same slot |
| 4 | `:149` delete outcome | `deleteResultToast()` | **varies** | **`dev.delete-logs` row** help slot, id `dev-delete-logs-outcome` |
| 5 | `:152` delete threw | `deleteErrorText()` | error | **`dev.delete-logs` row** help slot, same slot |
| 6 | `SettingsAccountsSection.tsx:57` token page would not open | literal + raw | error | **host slot** of the group whose card/locked form raised it; **section slot** when raised by the global add form (no host yet) |
| 7 | `:79` default not set | literal + raw | error | **host slot** (`setDefault` already has `host`) |
| 8 | `:97` could not remove `{host}` | literal + raw | error | **`.dialog-error` inside the open Remove dialog** (§1.3, §3.3) |
| 9 | `:138` added `{login}` to `{host}` | literal | success | **host slot** |
| 10 | `:149` connected to `{host}` as `{login}` | literal | success | **section slot** (the `accounts.add` row) |

### 6.1 Dev — two slots, why not five

`dev.logs` owns `Show in folder` **and** `Export session…`; `dev.delete-logs` owns `Delete all…`.
Rows 1-3 cannot collide: all three actions are gated by `anyBusy`, so at most one is in flight and
the newest result replaces the slot. A slot per *button* would put two notes 6 px apart reporting the
same button's two outcomes.

Row geometry: `dev.logs` is `stacked`, so its help slot is grid row 2 — the note appears between the
row label and the button pair, directly under `logsNote`. `dev.delete-logs` is a standard row, so
the note appears under the label in column 1, left of the danger button. In both, the note is the
**second** tenant of the help slot: the state note stays visible (§12.2's "note plus conditional
caveat"), it is never replaced.

### 6.2 Accounts — host slots, and the one section slot

- **Host slot:** one per rendered `SettingsAccountHostGroup`, placed **directly under the group head,
  above the OD-4 nudge**. Rationale: the group already has exactly one message zone there (the
  nudge), the host title sits immediately above so scope is visually obvious, and it is a stable
  home for outcomes produced by controls scattered through the group. The nudge is **not** hidden
  while an outcome shows — they are different facts, and the nudge is `--text-2` prose with no tint,
  so there is no amber-on-amber adjacency (checked: `.settings-account-group-note` is untinted).
- **Several hosts failing at once:** Accounts has no global busy gate, so it is reachable. Outcomes
  are keyed by host, **one note per host**, newest wins within a host. N failing hosts show N notes,
  each naming its own host, each in its own group. Only the section announcer speaks (§8.2).
- **A host that disappears** (its last account removed successfully) takes its group — and therefore
  its note — with it. Unmatched host entries are **never** re-homed to the section slot: a host that
  vanished by the user's own successful action has no pending message worth reading.
- **Section slot:** in the **`accounts.add` row's** help slot, id `accounts-add-outcome`. That row
  renders in **every** state of the pane — loading, list-error, empty, populated — which is exactly
  what row 10 needs (the new host's group does not exist until `refetch` resolves, and may never
  exist if `refetch` fails). It is also where the user physically is: they pressed
  `Add a token for a host` there and the form rendered under it.
- **Host binding without a prop change:** `SettingsAccountsSection` builds a per-group closure
  `onOpenUrl={(url) => openUrl(url, g.host)}` and `onOpenUrl={(url) => openUrl(url, null)}` for the
  global add form. `SettingsAccountHostGroup`, `SettingsAccountCard` and `SettingsAccountAddForm`
  keep `onOpenUrl(url: string)` **unchanged** — no signature churn across three files.

### 6.3 Wireframe

```
 Developer ▸ Log files                          Connected accounts
┌──────────────────────────────────────┐   ┌──────────────────────────────────┐
│ Log files                            │   │ ⟨badge⟩ github.com               │
│ Kept in ~/…/logs after you turn …    │   │ ┃ Could not set the default      │  ← host slot (warn)
│ ┃ Couldn't open the logs folder.     │   │ ┃ account: <raw>                 │
│ ┃ It may have been moved or deleted. │   │ Pick a default account for …     │  ← nudge, stays
│ [ Show in folder ] [ Export session…]│   │ ( ) octocat   Default            │
├──────────────────────────────────────┤   │ ( ) octocat-2                    │
│ Delete logs and usage counts         │   │ [ Add another account to … ]      │
│ Removes all 3 log files (1.2 MB) …   │   └──────────────────────────────────┘
│ Deleted 3 log files. 1.2 MB freed.   │   ┌──────────────────────────────────┐
│ Usage counts cleared.        [Delete │   │ Add a token for a host           │
│                               all…]  │   │ Connected to gitlab.com as dan.  │  ← section slot (result)
└──────────────────────────────────────┘   │ [ Add a token for a host ]        │
   ┃ = 3px --warning bar                   └──────────────────────────────────┘
```

---

## 7. Lifetime and dismissal

An inline note is not a toast and must not imitate one.

| Rule | Behaviour |
|---|---|
| Dismiss control | **None.** No ✕. The absence is the point — the unreachable ✕ is half the defect. |
| Auto-expiry | **None.** No timer. A 4 s window is exactly what the user who looked away misses, and a timed inline note is a toast with extra steps. |
| Clears when | **Any operation that reports into that slot begins.** `begin(key)` is called at the *start* of the operation, not when a confirm dialog opens — so cancelling the delete dialog leaves the previous outcome intact, while pressing `Delete all…` through to the operation clears it before the new result lands. |
| Also clears on | Unmount: leaving the category, or closing Settings. State is component-local by design; reopening Settings is a clean page. |
| Never clears on | A timer, blur, focus change, window focus, or an action in a *different* slot. |
| Dialog error (row 8) | Cleared when the Remove dialog **opens** (`setRemoveTarget`) and on cancel. `ConfirmDialog` returns `null` while closed, so the element unmounts with the dialog; the explicit clear is what stops a reopened dialog from showing the previous failure. |

**The meaning changes, and that is intended.** A toast reads *"this just happened"*. A note reads
**"this is the result of the last time you pressed this"** — which is why it lives in the slot of the
control that produced it and clears when that control is pressed again. Consequences:

- **No timestamps.** The app has no time formatter, and the user who just pressed the button knows
  when (the §12.13 precedent: report *what happened*, not a wall-clock time).
- **Copy must be past-tense outcome, never in-progress.** All ten strings already are. The delete
  row's `Deleting…` is the *state* note and is unaffected.
- A stale error that has not been retried is **true**, not misleading: nothing has happened since.

---

## 8. Live regions — the arithmetic, per section

`ui-reference.md:2386`: count per **section**, not per element. Every `SettingsOutcomeNote` is
**description-only** — no `aria-live`, no `role="status"`, no `role="alert"`.

### 8.1 `DevCategory` — exactly one, already present, unchanged

The existing `<p className="sr-only" role="status" aria-live="polite">{announce}</p>`
(`DevCategory.tsx:217`) stays the sole live region for the page. Up to **five** note texts can be
written across two slots; **none** is live, so no event can queue two utterances.

**The double-fire risk is real and this is how it is avoided:** for the delete outcome,
`deleteResultToast` returns `announce` **byte-identical** to `text` (P91 §6.11.1 R12). Making the
note live would announce the same sentence twice for one key press. The hook's `report()` writes the
note and the announcer in one call from one source, so the two cannot drift and cannot double-fire.

**A second, pre-existing bug the hook fixes for free — MUST.** A live region only fires on a text
*change*. Export twice and `DevCategory` sets `announce` to the identical string twice; React skips
the identical state, the text node never changes, and **the second outcome is silent.** This is
today's behaviour for every repeated outcome on the page. Fix: `begin(key)` clears `announce` to
`''` as well as clearing the slot. `begin` and `report` are separated by the operation's `await`, so
they are separate commits and the transition is always `'' → text`, a real change every time.
Covered by AC15.

**One a11y addition, MUST:** the reveal failure (row 1) sets **no** announcement today — it was
toast-only, so a screen-reader user heard nothing. Inline-only would make it visible-only, which is
worse. It must announce, with the same string. Precedent: the export-failure parity already added at
`DevCategory.tsx:119-121`.

Announcement text per row: 1 = the note text; 2 = `Session log exported.` (the folder is dropped —
the sanctioned "not actionable by voice" trim, P91 §6.11.1); 3 = `The log was not exported.`;
4 = `deleteResultToast().announce`; 5 = `Nothing was deleted.`

### 8.2 `SettingsAccountsSection` — one new region

The section has **no** live region today. Add exactly one: an always-mounted
`<p className="sr-only" role="status" aria-live="polite">` holding the newest outcome text. The host
notes and the section note are description-only. The strings already name their host/login, so one
announcer serves N hosts without ambiguity.

Same `begin()` reset applies: the section announcer is cleared at operation start, so repeating an
outcome for the same host announces twice rather than once.

- **Baseline, measured for AC6:** the pane already contains `role="alert"` elements that are
  **conditionally rendered** — the list-error `.error-banner` (`SettingsAccountsSection.tsx:109`) and
  the add form's own error banner (`SettingsAccountAddForm.tsx:200`, which can appear twice: the
  global form and a group's locked form). The new announcer is the **only** always-mounted region, so
  AC6's count is `1` in the state it names and rises by one per visible banner. None of them is an
  action-outcome channel.
- The existing list-error `.error-banner` keeps its `role="alert"` (`:109`). It is a **different
  event** (list load) from an action outcome, so there is no overlap in normal operation. (It is also
  mounted *with* its text, the shape the house rule says does not reliably announce — pre-existing,
  out of scope, recorded.)
- **The one compound case:** an add succeeds and the following `refetch` then fails. These land in
  **different ticks** — `report()` runs in the action's own continuation, the banner's alert arrives
  when the later round trip rejects — so the polite success is spoken first and the assertive alert
  follows. No suppression rule is needed; it is two facts and two utterances, in the right order.
- **Row 8 announces only in the dialog.** The `.dialog-error` `<p>` carries `role="alert"` and the
  section announcer is **not** set for that outcome — one utterance for one event. Inside an
  `aria-modal` dialog, focus is on the (re-enabled) Remove button, so an alert is the only channel
  that reaches the user without a focus move.
- The dialog's error `<p>` follows the §12.3.4 shape *within the dialog's lifetime*: it is rendered
  **always** (empty when idle) from the moment the dialog opens, so its text arrives in a later tick
  than its mount, which is the condition for being announced at all.

---

## 9. `aria-describedby` composition

Always **static** — both ids are always in the attribute, and an id resolving to an empty element
reads as nothing. Attribute churn is avoided on purpose: mutating `aria-describedby` can itself
trigger a re-announcement. Order is **state note first, outcome second**, matching §12.3.4's
`{help-id} {id}-draft`.

| Control | `aria-describedby` |
|---|---|
| `Show in folder`, `Export session…` (`SettingsDevLogsSection`) | `dev-logs-note dev-logs-outcome` — `logsNote` needs a new id (`dev-logs-note`); today it has none and neither button has any description. |
| `Delete all…` | `dev-delete-logs-hint dev-delete-logs-outcome` — composes onto the existing `DELETE_HINT_ID`, never replaces it. |
| The host `radiogroup` | `{nudgeId when shown} {hostOutcomeId}` — the outcome id is **unconditional** (the element is always mounted); the nudge id stays conditional as today. |
| `Add another account to {host}` | `{hostOutcomeId}` |
| `Add a token for a host` (`accounts.add`) | `{catalog help id when present} accounts-add-outcome` |

**The Remove confirm button gets no `aria-describedby`, deliberately.** `ConfirmDialogProps`
(`ConfirmDialog.tsx:5-22`) has **no** passthrough to the confirm button's attributes, and adding one
would touch a shared component with 13+ consumers to duplicate a channel that already works: the
error `<p>` is `role="alert"` inside an `aria-modal` dialog, and focus is on the button the user just
pressed. **`ConfirmDialog.tsx` is therefore not touched by this increment** — the `.dialog-error` is
rendered by `SettingsAccountsSection` as the last child of the dialog's `children`.

**Deliberately not wired:** the per-account token-page link inside `SettingsAccountCard`. A
group-level description would describe *every* card's link with a note about one of them, and the
failure is announced live at the moment it happens. `SettingsAccountCard.tsx` is not touched.

---

## 10. States

### 10.1 The note

| State | Spec |
|---|---|
| Idle (no outcome) | Element **present**, `textContent === ''`, `display` unchanged (block), `:empty` zeroes margin/padding/background/bar → rendered height **0**. `display: none` is forbidden: it costs the announcement. |
| Error | 12 % `--warning` tint, `inset 3px 0 0 var(--warning)`, `--text-1`, `padding: 6px 8px 6px 12px`, `border-radius: 4px`. |
| Success | `--text-1`, no tint, no bar, no padding. |
| Hover / active / focus | **None.** Not interactive, not focusable, no `tabIndex`, no cursor change. Text **is** selectable (`user-select` default) — an error a user may want to copy, which the toast made near-impossible. |
| Loading | The note is not a progress surface. In-flight state stays where it is: `aria-busy` on the button, `Deleting…` in the delete row's state note. The slot is empty during the operation (`begin`). |
| Disabled | N/A. A disabled row dims to `.55` via `.settings-row.is-disabled`; none of the ten rows is ever in a disabled row (`aria-disabled` is used on these buttons, which does not dim). |
| Long content | `max-width: 56ch` (inherited), `overflow-wrap: anywhere`, wraps freely. The row grows: the pathological 7-line delete outcome takes `dev.delete-logs` from 44 px to ~160 px; the card body scrolls, nothing is clipped, and the control does not move out of its grid cell. |
| Empty-list / error-list pane | The section slot's home (`accounts.add`) renders in all four pane states, so row 10 always has a home. |

### 10.2 Themes and densities

- **Both themes:** every value is a token; ratios in §4.3 are measured for both.
- **Both densities:** `panelDensity` `cozy` and `compact` share **one** settings-row geometry —
  44 px min row, 24 px controls, in both (`settings-primitives.css:14-16`, deliberate, D10). The note
  therefore has **identical** padding and margins in both densities, and **no compact variant may be
  added.** The only height change in either density is the note's own line count.

### 10.3 Motion

**None.** No appear/dismiss transition, no slide, no fade, no auto-scroll-into-view. Stated
explicitly because the channel being replaced *had* motion:

- A transition on a block that changes the height of a row inside a scrolling card animates layout,
  which is the one thing the house motion budget forbids (transform/opacity only).
- No `scrollIntoView`: the outcome often lands in the same commit that removes a card or resets a
  counter, and scrolling the pane under the user's pointer at that moment is a jump. The note is at
  the top of the group, or in the row, that the user just acted in; the announcer covers the
  non-sighted case.
- Nothing to gate behind `prefers-reduced-motion`, because there is no motion.

---

## 11. Interaction and keyboard

- No new click or right-click targets; no new hit areas; no ≥24 px concern (nothing is clickable).
- Keyboard order is **unchanged** — the note is not focusable and inserts nothing into the tab ring.
- Focus is never moved or restored by an outcome. All five Dev buttons use `aria-disabled` rather
  than `disabled`, so focus survives the operation and lands back on the control its note describes.
- Dialogs: Esc/Enter, focus trap and focus restore are `ConfirmDialog`'s existing behaviour,
  untouched. The `.dialog-error` does not steal focus.
- **Command palette: nothing is registered.** There is no new user-invocable action here — this is a
  delivery-surface change. Bonsai exposes ~150 commands already; adding one would be noise.

---

## 12. Microcopy

### 12.1 The ten strings are unchanged

P91 §6.11 was implemented and all five Dev seams were verified in the harness today; the strings are
**channel-independent**. This contract changes **where** a message appears, not what it says. The
Accounts five (P80) are likewise carried verbatim.

| # | String (verbatim) |
|---|---|
| 1 | `Couldn't open the logs folder. It may have been moved or deleted.` |
| 2 | `Session log exported. It is in the exports folder, next to your logs.` |
| 3 | `exportErrorText(errorMessage(e))` — four mapped branches, unchanged |
| 4 | `deleteResultToast(result).text` — T1/T2/T3, unchanged |
| 5 | `deleteErrorText(errorMessage(e))` — three mapped branches, unchanged |
| 6 | `Could not open the token page: {raw}` |
| 7 | `Could not set the default account: {raw}` |
| 8 | `Could not remove {host}: {raw}` |
| 9 | `Added {login} to {host}.` |
| 10 | `Connected to {host} as {login}.` |

One string is **added**, not changed: the reveal failure's live-region announcement (§8.1), which is
row 1's text reused.

### 12.2 Amendments flagged, not applied

- **A1 (SHOULD-FIX, own increment).** Rows 6-8 append raw `errorMessage(e)` — keychain/OS text — to
  a mapped lead. That already violated the house rule against leaking raw backend text; inline makes
  it permanent and prominent instead of transient and dim. Mapping it needs the backend error-kind
  inventory (`authFailed`, `forgeUnsupported`, keychain failures, `other`), which is outside a
  delivery-surface contract. **This increment renders them verbatim**, with
  `overflow-wrap: anywhere` so a space-free token cannot overflow. Recommend a follow-up
  `P113-F1-accounts-error-copy`.
- **A2 (NIT).** Row 8's `Could not remove {host}` now renders in a dialog titled
  `Remove {login}?` — the host is slightly redundant against the dialog title. Truthful; keep.
- **A3 (NIT).** `DevToast`, `deleteResultToast`, `devDeleteToastRows.test.ts` and the
  `// The toast …` comments in `DevCategory.tsx` become misnomers. **Do not rename in this
  increment** — it churns two test files for no behaviour, and §13.1's lint guard covers the actual
  risk (someone reaching for `pushToast` because the surrounding code says "toast"). Comments that
  *assert* toast behaviour (`DevCategory.tsx:111-114`, `:119-120`) must be corrected in place, since
  they would otherwise document the opposite of the code.

---

## 13. Keeping the channel closed

### 13.1 The lint guard — recommended, and it belongs here

The residual risk is exactly as stated: a *future* Settings feature reaches for `pushToast` and
silently gets an invisible, unclickable message. That is not hypothetical — the ten call sites here
were each written by someone who had no way to see the problem, and P112-4 adds a brand-new Settings
surface next.

`eslint.config.js` is flat (`tseslint.config(...)`) with per-`files` blocks, so this is a precise,
zero-runtime-cost addition:

```js
{
  // P113: the Settings surface renders inside `.dialog-overlay` (z-index 100),
  // `.toast-stack` is 90 — a toast raised here is unclickable, not merely dim
  // (measured: elementFromPoint at the toast's centre returns the overlay).
  // Outcomes go inline via SettingsOutcomeNote (ui-reference §12.14).
  files: ['src/components/settings/**/*.{ts,tsx}', 'src/components/Settings*.tsx'],
  rules: {
    'no-restricted-imports': ['error', {
      paths: [{
        name: '../../ToastContext',          // + the other relative depths in use
        message: 'Settings renders inside .dialog-overlay, so a toast raised here is invisible and unclickable. Use SettingsOutcomeNote (ui-reference §12.14).',
      }],
    }],
  },
}
```

`no-restricted-imports` matches literal specifiers, so senior-dev must list every relative depth
actually used (`../ToastContext`, `../../ToastContext`) or use
`patterns: [{ group: ['**/ToastContext'], message: … }]` — **prefer the `patterns` form**, it cannot
be defeated by a new directory depth. AC9 checks the rule actually fires.

### 13.2 Sequencing flag for the orchestrator

**P112 sub-increment 4 references `.settings-row-note--warn` for the external-tool picker's Browse
errors, and that class has no CSS until P113 lands.** Either P113 ships first, or P112-4 carries the
§4.1 recipe itself and P113 must then not re-add it. Recommendation: **land P113 first** — it is
self-contained, and P112-4 then gets the component, the lint rule and the recipe for free.

### 13.3 Out of scope, but recorded

A toast raised by a **background** event (a fetch completing, a watcher rescan) while Settings is
open is equally invisible, and no lint rule can catch it because the call site is not on the Settings
surface. Options are (a) raise `.toast-stack` above `.dialog-overlay`, which would resurrect the
overlap this sweep removes for any future Settings toast, or (b) queue background toasts until
Settings closes. **Recommendation: change nothing here**; file it as a TODO follow-up. It needs its
own increment and its own measurement.

---

## 14. Harness states and mock fixtures

All ten are reachable in a plain browser (`pnpm dev`, `VITE_MOCK_IPC=1`). Existing knobs cover four;
four new knobs are required.

| Case | URL |
|---|---|
| 2 export success | `?obsLogFiles=3` + Dev mode on, then `Export session…` |
| 3 export failure ×3 | `?obsExportFail=space` \| `permission` \| `other` (**existing**) |
| 4 delete success (T1/T2) | `?obsLogFiles=3&obsExports=1` / `?obsLogFiles=0` (**existing**) |
| 4 delete partial → **error tone** | `?obsDeleteFail=partial` (**existing**) — the tone-varies case, and the one that proves rows 4/5 share a slot |
| 5 delete threw | `?obsDeleteFail=throw` (**existing**) |
| 1 reveal failure | **`?obsRevealFail=1`** — new: `logRevealDir` currently always resolves, so row 1 is unreachable today |
| 6 token page failure | **`?openUrlFail=1`** in `handlers/external.ts` — new: the token URL is built by the app, so the `#fail` sentinel cannot be injected from the query string |
| 7 default-set failure | **`?forgeDefaultFail=1`** — new: the existing throw needs an off-host account, unreachable from the UI |
| 8 remove failure (**dialog stays open**) | **`?forgeRemoveFail=1`** — new |
| 9 / 10 add successes | existing `forgeAddAccount` happy path (a second `github.com` login already works) |
| Empty | `?forgeAccounts=0`-equivalent (existing empty store) — proves the section slot still has a home |
| Loading | existing `delay(120)` in `forgeListAccounts` |
| List error + outcome | existing list-error path + an add; proves §8.2's compound case |
| **Pathological** | `?obsLogFiles=10010&obsExports=129&obsDeleteFail=partial` (**existing**, 7-line outcome) **plus** a new long-raw-error form: `?forgeRemoveFail=long` producing a ~300-char space-free token, and a 60-char host name |

**USER CHECKPOINT items** (the harness is headless — `requestAnimationFrame` does not fire):
none for motion, because §10.3 specifies none. The remaining human-perception item is only whether
the note reads as subordinate to the row's own state note at real screen size, in both themes.

---

## 15. Acceptance criteria

Each is independently checkable, with its verification method. **AC2 and AC3 are geometric on
purpose:** every prior verification of this UI, including mine, read `innerText`, and a string in the
DOM proved nothing here.

1. **No `pushToast` remains on the Settings surface.**
   `rg -l "usePushToast|pushToast" src/components/settings src/components/Settings*.tsx` → **no
   output**. This is the check the lint rule then protects.
2. **The note is the hit-test target and is inside the card.** In the harness, Settings open on
   Developer, after triggering row 4: for `el = document.querySelector('[data-outcome-note="dev.delete-logs"]')`,
   `document.elementFromPoint(cx, cy)` at the centre of `el.getBoundingClientRect()` returns `el` **or
   a descendant of `el`**; `el`'s rect has non-zero area; and the rect is **fully inside**
   `.settings-card`'s rect on all four edges. Report the four numbers. Repeat for one Accounts host
   note. *(This is the AC that answers the failure mode that produced this contract.)*
3. **No toast is raised, and none is behind the overlay.** After each of the ten triggers,
   `document.querySelectorAll('.toast').length === 0`, and `.toast-stack` (if present) has
   `getBoundingClientRect().height === 0`.
4. **Contrast, measured, both themes.** For a rendered error note, read computed `color` and
   `background-color`, **composite the `rgba` tint over the ancestor's opaque colour** (P107's
   serialization trap), and compute the ratio: ≥ 4.5:1 for text, ≥ 3:1 for the bar against both its
   own tint and `--bg-0`. Expect §4.3's figures (12.03 / 14.16; 6.46 / 4.17; 7.92 / 4.87). Run with
   `resize_window colorScheme: light` as well. Report ratios, not assertions.
5. **Idle is present, empty, and zero-height.** With no outcome: the element exists,
   `textContent === ''`, `getComputedStyle(el).display !== 'none'`, and
   `getBoundingClientRect().height === 0`. Verified in a vitest DOM test plus one harness read.
6. **One live region per section.** `container.querySelectorAll('[aria-live],[role="status"],[role="alert"]').length`
   — `DevCategory`: **1** in every state. `SettingsAccountsSection`: **1** in the state
   *populated, no add form open, no list error*; **2** with the list-error banner shown; **2** with
   an add form showing its own error (§8.2's measured baseline — those banners are conditional and
   pre-existing). **No** element matching `[data-outcome-note]` has an `aria-live` attribute or a
   `role`, in any state. Vitest.
7. **No double-fire.** Triggering row 4 writes the announcer exactly once and the note exactly once,
   from one `report()` call; the announcer's text equals `deleteResultToast().announce`. Vitest with
   a text-mutation spy on the announcer node.
8. **`aria-describedby` composes, never replaces.** For each of the six controls in §9, the attribute
   contains **both** ids (where two are specified) and **every** id resolves to an element in the
   document. Vitest, per control.
9. **The guard fires.** A fixture file under `src/components/settings/` importing `ToastContext`
   fails `pnpm lint` with the §13.1 message; removing the import passes. Verified by
   `eslint --stdin --stdin-filename src/components/settings/__guard.tsx`, not by reading the config.
10. **Copy fidelity.** The ten rendered strings are byte-identical to the current toast strings —
    for the Dev five, to `devLogMessages` output for the same fixture. Vitest text comparison against
    the existing `dev.test.tsx` / `devDeleteToastRows.test.ts` expectations, which must need **no**
    edits to their expected strings.
11. **Row geometry unchanged when idle, in both densities.** `.settings-row` height for `dev.logs`
    and `dev.delete-logs` is identical before and after the change with no outcome showing, and
    identical between `cozy` and `compact`. Measured via `getBoundingClientRect`.
12. **Lifetime.** Pressing the action again clears the slot **before** the new result arrives (the
    note's text is `''` during the in-flight window); cancelling the delete confirm dialog leaves the
    previous outcome intact; reopening the Remove dialog after a failure shows **no** error.
13. **The reveal failure announces.** Triggering row 1 with `?obsRevealFail=1` sets the
    `DevCategory` announcer to the same string it renders inline.
14. **Pathological content does not escape or displace.** With the §14 pathological fixtures, the
    note's rect right edge is ≤ the card's content right edge, the note wraps to ≤ 56ch per line, the
    row's control cell keeps its own rect, and the card body scrolls rather than clipping. Bounding
    boxes, not text.
15. **The same outcome twice announces twice.** Trigger an identical outcome consecutively (export
    twice with the same fixture, or `?obsDeleteFail=throw` twice): the announcer's text node changes
    on **both**, via the `'' → text` transition `begin()` guarantees. Vitest with a mutation spy —
    asserting the final text is not evidence, since the bug is an *absent* change. Regression guard
    for the pre-existing silence described in §8.1.
16. **The dialog case.** With `?forgeRemoveFail=1`: the Remove dialog is still open after the
    failure, contains a `.dialog-error` with `role="alert"` and the mapped text, and — by AC2's
    method — that element is the hit-test target at its own centre. The section announcer is **not**
    set for this outcome (AC6's count for Accounts stays at 1).

---

## 16. `ui-reference.md` changes made in this pass

1. **§12.13** — the two bullets at `:2377-2390` (the Settings-toast rule and the
   live-regions-per-section rule) are replaced by a one-line pointer. They were canonical
   cross-section rules living inside the P112 picker section; the picker-specific consequence
   (Browse errors are inline) stays.
2. **New §12.14 "Outcome notes — the Settings surface has no toasts (P113, SIGNED 2026-09-14)"** —
   the measurement, the two recipes with §4.3's ratios, the tone split and its reasoning, the
   lifetime rule, the one-live-region-per-section rule (moved here intact), the dialog-stays-open
   case, §4.4's banner-vs-note disambiguation, and the lint guard.

No new tokens. Every value in this contract is an existing custom property.
