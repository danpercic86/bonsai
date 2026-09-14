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

**Do not over-invest in this path (noted 2026-09-14).** `forge_remove_account_inner`
(`src-tauri/src/commands/forge_accounts.rs:280-310`) swallows both substantive failures
(`let _ = delete_token`, `let _ = settings::update`), so the only rejections that reach the frontend
are `cannot resolve app config dir` and a task-join error. **Row 8 is near-unreachable in the real
app** and its harness case (`?forgeRemoveFail=1`) is mock-only. It stays specced — it is ~5 lines,
and the dialog-stays-open behaviour it handles is real — but it earns no further design time. The
swallowed errors are a **backend defect, filed separately**; if they are ever surfaced, this path
becomes live with no UI change required, which is the right place for it to already exist.

---

## 4. The two recipes

### 4.1 Error tone — `.settings-row-note--warn` (new CSS, written here)

**CORRECTED 2026-09-14 (ruling R3): its own file, not `settings-primitives.css`.** My line count was
stale — that file is **468** lines, not 401, so +42 breaches the 500 ratchet. The recipe goes in its
own file, which is what the house rule says anyway ("new UI in its own file, never appended to a
large one"). Two constraints come with that and are **load-bearing**:

- **Import it AFTER `settings-primitives.css`** in `src/styles.css`'s fixed import list.
  `.settings-row-note--warn` and `.settings-row-note` are both single-class, specificity (0,1,0), and
  both declare `margin` and `color` — so **source order alone** decides the modifier. Imported before
  the base, the `--warn` ink and margins silently lose and the note renders as ordinary help text.
  That file's own header ("Cascade order is fixed by the import list — do not reorder") is the
  precedent; this is a new dependency on it and must be commented at the import site.
- `.settings-account-group > .settings-row-note` is (0,2,0) and wins regardless of order.

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
   so a note that is its flex child must not add its own top margin — and, while
   IDLE, must also cancel the gap the container hands every child. An
   always-mounted zero-height flex child still earns its 8px `gap`; `margin: 0`
   alone leaves the group 8px taller than before the note existed.
   (AMENDED 2026-09-14, ruling R2 — measured idle gap unchanged after the fix.) */
.settings-account-group > .settings-row-note {
  margin: 0;
}
.settings-account-group > .settings-row-note:empty {
  margin-top: -8px;
}
```

**The general rule this is an instance of:** the always-mounted-empty shape is only inert if it is
checked against the **parent's** layout, not just its own box. `gap`, `space-*` utilities and
`:first-child`/`+` sibling selectors all act on an element that is present at zero height. Any future
reuse of `SettingsOutcomeNote` in a `gap`-based flex or grid container must repeat this measurement.

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
| `src/styles/settings-outcome-note.css` (or the implementer's chosen name) | **new** | §4's rules. **Not** appended to `settings-primitives.css` — that file is 468 lines and +42 breaches the ratchet (ruling R3). Imported **after** `settings-primitives.css`; see §4.1 for why source order is load-bearing here. | ~45 lines |
| `src/styles/dialogs.css` | edit | `.dialog-error:empty { margin: 0 }` only. **Safe on a 13-site shared rule** because every existing `.dialog-error` call site renders the element *conditionally* — none of them is ever empty, so the new rule can only ever match the one site added here. | +3 |
| `eslint.config.js` | edit | §13.1's guard block. | +14 |
| `src/ipc/mock/handlers/obs.ts`, `handlers/forge.ts`, `handlers/external.ts` | edit | §14's knobs. | +~25 total |

**Why a new component rather than inline `<p>`s:** the element must be *permanently mounted with
empty text* and must never carry `aria-live` — two rules that are invisible at a call site and were
already broken once on this surface. One file makes them reviewable in one place, and P112-4 gets
them for free.

`SettingsAccountCard.tsx` is **not touched** (§10.3).

---

## 6. Placement — call sites 1-10

**Five more were found after implementation (§17.2): the sweep is 15.** Sites 11-15 and their
placements are in §17.3; this section covers 1-10.

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
notes and the section note are description-only.

**CORRECTED 2026-09-14 — this paragraph used to claim "the strings already name their host/login, so
one announcer serves N hosts without ambiguity". That premise is false against the tree.** Two of the
five do not name either: `Could not open the token page: ${errorMessage(e)}`
(`SettingsAccountsSection.tsx:111`) and `Could not set the default account: ${errorMessage(e)}`
(`:134`). With one announcer and N host groups, those two announce a failure **without saying which
host it belongs to** — and only to the user who cannot see which group the note sits in. A sighted
user reads the answer off the placement; an AT user gets nothing. **Placement is not an accessible
carrier of meaning**, which is the a11y analogue of the house rule that colour never carries meaning
alone, and it is the same defect class as P91 §6.8 R5: a message that under-specifies its scope to
exactly the user who cannot recover it from context.

**RULING — AMENDMENT A5: add the host to both strings. Recommended, needs the orchestrator's call**

> **A5 — APPROVED (orchestrator, 2026-09-14). Ship both strings; the documented-ambiguity
> alternative stays rejected.** I verified the two factual preconditions against source before
> approving, since this session has repeatedly found copy that outran its mechanics:
> * **`setDefault` always has the host** — `SettingsAccountsSection.tsx:130` is
>   `(host: string, accountId: string)`, non-optional. So the unconditional form is correct there.
> * **The token-page path genuinely can lack one** — `:104` is `const slot = host ?? ADD_SLOT`, so the
>   `host === null` fallback to the current string is required, not defensive.
>
> The deciding argument is the designer's and it generalises: **placement is not an accessible carrier
> of meaning.** A sighted user reads which host failed off the note's position in its group; with one
> announcer serving N groups, a screen-reader user gets a failure with no subject. That is the same
> defect class as P91 §6.8 R5 — a message that under-names what it refers to — and the a11y analogue
> of the rule that colour may not be the sole carrier.
>
> Changing the **visible** note text too is correct, not collateral: it keeps the string
> **channel-independent**, which is the property every placement decision in this contract has leaned
> on, and mild redundancy against the group title costs nothing.
>
> **Unchanged by this approval:** both strings keep their raw `errorMessage(e)` tail. That is
> `P113-F1-accounts-error-copy`'s job and is still deferred — A5 adds a subject, it does not map the
> cause.
(these are P80-era strings, not signed P91 copy, but the flag-don't-rewrite rule applies to both):

| Site | Now | Proposed |
|---|---|---|
| `:134` set default | `Could not set the default account: {raw}` | **`Could not set the default account for {host}: {raw}`** — `setDefault` always has `host`, so this is unconditional |
| `:111` token page | `Could not open the token page: {raw}` | **`Could not open the token page for {host}: {raw}`**, falling back to the current string when `host` is `null` (the global add form, before a host is chosen) |

This changes the **visible** note text too, which is correct rather than incidental: naming the host
is mildly redundant against the group title the note sits under, and harmless there, while it keeps
the string **channel-independent** — the property every placement decision in this contract has
relied on. A string that only works in one of its two channels is the thing we have been avoiding
throughout.

*Alternative, rejected:* document that the announcement cannot identify the row and argue it is
acceptable. It is not — it is two string edits away from being unambiguous, and accepting known
ambiguity for the AT user while the sighted user gets the answer free is the wrong side of this
project's a11y line.

Same `begin()` reset applies: the section announcer is cleared at operation start, so repeating an
outcome for the same host announces twice rather than once.

**`begin(key)` clears the announcer GLOBALLY while clearing only that key's note — and that is the
specified behaviour.** There is exactly *one* announcement per section, so "clear the section's
announcement" is the only thing `begin` can mean.

**WITHDRAWN 2026-09-14 — the key-scoped refinement this paragraph previously proposed was wrong, and
it contradicted §8.1.** I suggested clearing the announcer only when the pending announcement belonged
to the **same** key, to stop a concurrent action on host B truncating an utterance about host A.
Measured in the harness (`?mcpFail=register`, `MutationObserver` on the section announcer, both
register rows raising byte-identical `Could not register: Claude Code CLI not found: program not
found`):

| Event | Announcer mutations |
|---|---|
| global row fails | `['', T]` |
| repository row fails — **different key, identical text** | **`['', T]` — the second event is silent** |

§8.1 declares that exact outcome a MUST violation ("the second outcome is **silent**"), so the
refinement reintroduced the bug this component exists to prevent, one axis over. It is also **worse
than what it replaced**: the global clear's truncation window is roughly one utterance, whereas a
last-announced-key ref never expires, so a note announced an hour ago would silence a different row
carrying the same text. The realistic trigger is the single likeliest MCP failure there is — no
`claude` on PATH fails both register rows with identical text.

**The rule, stated as an invariant so the two paragraphs cannot disagree again:**

> **Every reported outcome must produce a text CHANGE in the section announcer — whatever its key, and
> whether or not the previous announcement carried the same string.** A live region fires on change,
> not on assignment, so the announcer must be cleared in a commit strictly *before* the one that
> writes the outcome text whenever the two would otherwise be equal.

Consequences, binding:

- The outcome text is **not known at operation start**, so `begin(key)` cannot decide by comparing
  future text and therefore **clears unconditionally**.
- Any guard that skips a clear must key on **TEXT identity** — it may skip only where it can prove the
  next write differs from what is displayed — **never on key identity**. Key identity is not a proxy
  for text identity, which is precisely what the measurement above shows.
- `AC7` (one write per event) and `AC15` (a repeat announces twice) **provably never enter that
  branch**: both arrive with the announcer already at `''` from `begin`, so the incoming text always
  differs from what is displayed.
- **The accepted cost, explicitly:** unconditional clearing can truncate an in-flight utterance about
  another key on Accounts, which has no global busy gate (Dev is `anyBusy`-gated and unaffected). That
  is a bounded, roughly one-utterance window, and it is **strictly preferable to silence**, which is
  unbounded and permanent. §8.1's MUST wins. Covered by AC17.

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
- **`scrollIntoView`: forbidden on the Accounts surface, RELAXED for the Dev row slots
  (AMENDED 2026-09-14, ruling R1).** The original blanket ban was written for the Accounts case,
  where the outcome lands in the same commit that removes a card and scrolling under the pointer is a
  jump. That reasoning does not transfer to the Dev page, where the commit adds a line and removes
  nothing — and the ban produced a worse defect than the one it prevented (see §17.1). The relaxation
  is narrow, and all four conditions are required:
  1. **Measured, not assumed:** adjust only when the note's rect is not fully inside the scroll
     container's client rect.
  2. `block: 'nearest'`, **`behavior: 'auto'`** — instant. Never `'smooth'`. An instant scroll
     correction is not animation; it is the same class of thing as a caret-following scroll, so there
     is still **nothing to gate behind `prefers-reduced-motion`** and no media-query branch.
     **AMENDED 2026-09-14, on a reproduced measurement: `scrollIntoView` alone does NOT satisfy
     AC2b, so the call is not the whole step.** Two agents measured the same residual at different
     viewports: the correction settles with the note's bottom **0.171875 px** below the clip, and
     `scrollTop += 0.171875` **reads back unchanged** because at DPR 1 the scroll offset snaps to
     whole pixels — a sub-pixel deficit is simply unreachable. The prescribed step is therefore
     `scrollIntoView(...)` **followed by a ceil correction**: measure
     `deficit = noteRect.bottom - paneClientRect.bottom` and, while `deficit > 0`, apply
     `pane.scrollTop += Math.ceil(deficit)`. **`Math.ceil` is load-bearing, not defensive** — without
     it the four-edge test in AC2b cannot be met by following this contract, which is a defect in the
     contract rather than in the implementation. Ceiling to a whole pixel is also correct at DPR > 1,
     where fractional offsets are representable: it over-scrolls by under 1 px, which is invisible and
     cheaper than DPR-aware rounding.
  3. **Only when focus is inside the row that owns the slot** (`document.activeElement`). The user
     standing on the control gets the correction; a user who navigated elsewhere during the async
     operation is never yanked. `ConfirmDialog` restores focus to `Delete all…`, so the common path
     is covered.
  4. **Dev slots only.** The Accounts host slots keep the ban: they sit at the *top* of their group
     (far from the page end, rarely clipped) and their commit can remove a card.
- Apart from that correction, there is no motion, and nothing to gate behind
  `prefers-reduced-motion`.

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
- **A5 (SHOULD-FIX, recommended — added 2026-09-14).** Rows 6 and 7 name **no host**, which makes the
  section's single announcement ambiguous about which group failed. Proposed strings, reasoning and
  the rejected alternative are in §8.2. If accepted, §12.1's table rows 6 and 7 change accordingly;
  this is the one place in the contract where the channel change does force a string change, and it
  forces it for an a11y reason rather than a cosmetic one.
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

### 13.1 The guard — REDESIGNED 2026-09-14 (§17.2). A path lint is not sufficient.

The residual risk is exactly as stated: a *future* Settings feature reaches for `pushToast` and
silently gets an invisible, unclickable message. That is not hypothetical — the call sites here were
each written by someone who had no way to see the problem, and P112-4 adds a brand-new Settings
surface next.

**But a path-scoped lint cannot express "reaches the Settings surface", and this is not a theoretical
limitation — it is precisely how five call sites were missed (§17.2).** `useUiSettings.ts` and
`useMcpControls.ts` receive `pushToast` **as a parameter** and are wired from `App.tsx`; they render
Settings rows while living in `src/hooks/`. No `files:` glob can see that, because the property that
matters is reachability, not location. So the guard is now **two parts, and the lint is the cheaper
second one**:

**(1) Dev-time reachability warning — the primary guard.** In `DEV` only, `pushToast` emits a
`console.error` naming the message text when the Settings overlay is open:
`pushToast called while Settings is open — this message renders behind .dialog-overlay and is
unclickable. Use SettingsOutcomeNote (ui-reference §12.14).` It checks the condition that actually
matters, catches a caller anywhere in the tree including one reached through three layers of props,
and fires in the browser harness during development. It also covers §13.3's background-toast case,
which is no longer a separate concern — it is the same mechanism.
The `settingsOpen` signal it needs is the **same signal** §17.3 needs to route the global save
failure, so it is wired once and serves both.
*Limitation, stated honestly: it only fires if someone runs the path. It is a smoke alarm, not a type
system. That is still strictly more than the lint can do.*

**(2) The path lint — keep, unchanged, and stop calling it the guard.** It remains worth having for
the naive case (a new component under `src/components/settings/` importing `ToastContext` directly),
and it is free.

**Widening its `files:` glob to the two hooks would accomplish nothing, and specifying that would be
cargo cult:** `no-restricted-imports` restricts *imports*, and neither hook imports `ToastContext` —
they receive `pushToast` as a **parameter**. There is no import to ban. The structural fix for those
two is in the code, not the config:

- **`useMcpControls`: delete the `pushToast` parameter.** After §17.3 it has no remaining caller, so
  the capability is removed rather than guarded. This is the strongest available guard and it costs
  nothing.
- **`useUiSettings`: keeps the parameter**, because §17.3 keeps a toast for the Settings-closed
  branch — which is correct there and must not be linted away. Its guard is (1).

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

**Updated 2026-09-14:** §13.1's dev-time reachability warning now *detects* this case — it fires on
any `pushToast` while the overlay is open, whatever the caller — so the condition is at least
observable during development. It still does not *fix* it, and the recommendation is unchanged.
`useUiSettings.ts:287` turns out to be a live instance of exactly this shape (§17.3): one call site
that is broken on one surface and correct on another.

---

## 14. Harness states and mock fixtures

All ten are reachable in a plain browser (`pnpm dev`, `VITE_MOCK_IPC=1`). Existing knobs cover four;
four new knobs are required. **Sites 11-15 (§17.3) need two more:** an MCP failure knob for
`setMcpEnabled` / `registerMcpWithClaude` / `setMcpAllowWrite`, and **`?settingsSaveFail=1`** for
`setUiSettings` — nothing in the mock currently rejects a settings write, which is why the app's
highest-traffic Settings toast has never been seen rendered by anyone.

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

1. **No `pushToast` reaches the Settings surface — enumerated by REACHABILITY, not by directory.**
   **REWRITTEN 2026-09-14 (§17.2): the original wording is what let five call sites through.**
   `rg -n "pushToast\(" src/` → enumerate **every** call site in the repository, and account for each
   one in a table: either it is swept, or it is justified as unreachable from Settings. A call site is
   reachable if the component renders inside `SettingsPanel` **or** if the function is passed into a
   hook/helper that any such component calls — `src/hooks/**` is explicitly in scope. Grepping two
   directories is **not** this check.
2. **The note is the hit-test target, and is fully visible, and the two failures are distinguished.**
   **AMENDED 2026-09-14 (ruling R1)** — the original conflated occlusion with scroll clipping.
   For each slot, with `el = document.querySelector('[data-outcome-note="{slot}"]')`:
   **Both are measured at the scroll position the action itself leaves the pane in** — i.e. trigger
   the action, let focus restore and the re-render settle, and measure *there*, **without scrolling
   first**. **A pass obtained after scrolling to the end of the pane is not a pass**: as first
   written this criterion named no scroll position and was therefore literally satisfiable at max
   scroll, where the note is fully visible anyway and nothing is being tested. (Corrected
   2026-09-14.)
   - **2a — not occluded (the failure this contract exists to fix).** At a point **1px inside `el`'s
     top-left corner**, `document.elementFromPoint` returns `el` or a descendant — **never**
     `.dialog-overlay` or `.toast-stack`. Scroll-independent by derivation (§17.1 R1), so it must
     also hold at max scroll; asserting it at both positions is what demonstrates the independence.
   - **2b — not clipped (the failure R1 found).** At the action's own scroll position, after the
     §10.3 adjustment settles, `el`'s rect is **fully inside** the scroll container's client rect on
     all four edges, and `elementFromPoint` at `el`'s **centre** returns `el` or a descendant. Report
     the four edge numbers **and** the `scrollTop`/`scrollHeight` they were taken at, so the next
     reader can tell a real pass from a max-scroll one.
   - A failure of 2a and a failure of 2b are different defects with different fixes; a report must
     say which. Run both for row 4 (the last element on the Dev page, which is where 2b bites) and one
     Accounts host note.
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
    **Partly discharged 2026-09-14:** the four MCP rows have a **measured** idle note height of 0, so
    the "an idle note costs nothing" half is evidence rather than inference for those rows. **Still
    owed: the cozy/compact comparison**, for every slot. It is the half that guards
    `settings-primitives.css:14-16`'s deliberate one-geometry-in-both-densities decision, and no
    amount of idle-height measurement substitutes for it.
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
    **Scope, amended 2026-09-14 (ruling R4):** applies to every slot that has an observable operation
    start — the five Dev outcomes and the four MCP outcomes (§17.3). Rows 9/10 are covered by the
    form-open `begin()` of §17.1-R4; if the orchestrator defers that, AC15 excludes rows 9/10 and the
    limitation is recorded in TODO instead.
16. **The dialog case.** With `?forgeRemoveFail=1`: the Remove dialog is still open after the
    failure, contains a `.dialog-error` with `role="alert"` and the mapped text, and — by AC2a's
    method — that element is the hit-test target at its own top-left corner. The section announcer is
    **not** set for this outcome: its text stays `''`.
    **The parenthetical "(AC6's count for Accounts stays at 1)" is STRUCK, 2026-09-14 (ruling R5) —
    not reworded.** It was not imprecise, it was **self-contradictory**: §8.2 already states the count
    "rises by one per visible banner", and `ConfirmDialog` renders **inline, with no portal**, so the
    dialog's `role="alert"` is inside the section container by construction. A count of **2** while
    the dialog is shown is *entailed by this contract's own design*, and asserting 1 would have made
    AC16 unpassable against a correct implementation. The meaningful half — the announcer's `''`, on
    which one-event-one-utterance actually depends — is the assertion above. My error, not the
    implementer's.
17. **Two DIFFERENT keys raising BYTE-IDENTICAL text both announce.** Added 2026-09-14; this is the
    criterion the withdrawn key-scoped refinement would have failed (§8.2). With `?mcpFail=register`,
    fail the global register row and then the repository register row — identical strings, different
    slots. A `MutationObserver` on the section announcer must record **`['', T, '', T]`**. **`['', T]`
    is a failure**, and it is the exact trace the refinement produced. Assert the mutation sequence,
    never the final text: the defect is an absent change, so a final-value assertion passes against
    the bug. The same shape applies to the Accounts host slots (two hosts, one identical error).

---

## 16. Implementation status

**Phase 1 approved with no MUST-FIX and committed as `0c86376`** (2026-09-14); phase 2 — sites 11-15
(§17.3) and the flagged follow-ups (§17.4) — is being routed. The implementation raised five conflicts
against this contract and the sweep was found to be **10 of 15** call sites; both are settled in §17,
which is the authoritative amendment record. Where §17 and an earlier section disagree, §17 wins —
though the earlier sections have been corrected in place, so there should be nothing left to
disagree about. The `ui-reference.md` edits are summarised in §18.

---

## 17. Amendments after implementation — 2026-09-14

The implementation raised five conflicts against this contract. **Four of the five are my errors, not
the implementer's**, and are corrected in place above; the fifth is a genuine design call. Rulings:

### 17.1 The five rulings

**R1 — AC2 vs §10.3 (the delete note is clipped by scroll). RULING: relax §10.3, narrowly, and split
AC2.** The implementer is right that this is a **different failure mode** from the one this contract
exists to fix: the note is not occluded by a z-index, it is scrolled past the clip of its own
container. Row 4's note is the last element on the Dev page; at the scroll position a click or Tab
produces (`scrollTop 1207/1310`) its bottom sits 43.8px below the clip, so ~20 of its 64px are
visible and `elementFromPoint` at the **centre** falls through to the overlay.

Accepting the clipped state was the other real option, and I reject it: the pathological 7-line
outcome would show one line of seven, and "the message is there if you go and find it" is a weaker
version of the complaint that started this whole contract. My blanket ban on `scrollIntoView` was
written for the Accounts surface and I over-generalised it; keeping a rule I wrote over the outcome it
was written to protect would be the wrong trade. §10.3 now carries the four conditions, and AC2 is
split into **2a (occlusion, measured at the top-left corner, scroll-independent)** and **2b
(visibility, measured at the action's own scroll position)**. A future regression report must say
which one failed.

**The distinction is derivable from CSS, not only measurable — and that is worth more than the
measurement.** `.settings-pane` is `overflow-y: auto` inside `.dialog-card.settings-card`, which is
`overflow: hidden`. Nothing below that clip is **painted at all**, so `elementFromPoint` there
*necessarily* returns the next painted thing — the overlay. It could not return anything else, for
any element, in any correct browser. Contrast §1.1: the toast's **entire box lay inside the viewport,
painted, on top of the card**, and was still un-hit-testable at every point including its ✕. Those
are categorically different faults — one is "this pixel is outside the scrollport", the other is
"this pixel belongs to something stacked above". AC2a/2b can therefore be **reasoned about** and
reviewed on a diff, not only reproduced in a harness. That is why they are two criteria and not one.

**Mechanism, for the record.** `.settings-row-control { grid-row: 1 / -1; align-items: center }`
(`settings-primitives.css:49-55`) centres the button across both grid rows of the grown row, while the
help slot on row 2 extends *below* it. Focus restore scrolls the **button** into view and is satisfied
while the note's tail is still past the clip.

**The layout alternative — considered, rejected, and not a near miss.** The one §10.3-compatible fix
without script is to make `dev.delete-logs` `stacked`, so the help slot precedes the control and
scrolling the button into view necessarily brings the note with it. It has real merit: zero JS, and it
would make the two Dev rows consistent with each other (`dev.logs` is already stacked). **I am keeping
the scroll correction, and AC11 stands unrelaxed.** Three reasons:
1. **It fixes one instance; the scroll correction fixes the class.** Any slot that is the *last
   element of a pane* has this shape, and one already is: the section slot in the `accounts.add` row
   is the last thing in the Accounts pane. The layout route would have to be re-argued per row, and
   re-checked every time a pane's row order changes.
2. **It trades a permanent cost for an occasional one.** Stacking changes idle geometry for every
   user at all times — and moves a *destructive* control out of the right-hand control column into a
   stretched, left-aligned slot — to fix a state that exists only after an action, only on the last
   row.
3. **AC11 is the stronger guarantee.** It pins the promise that an idle note costs nothing, in both
   densities. Relaxing that to avoid an instant, measured, focus-gated scroll correction is the wrong
   direction: §10.3 exists to prevent animation contending with the render budget and jumps under the
   pointer, and the correction as conditioned is neither.

**R2 — the flex `gap` (§4.1). RULING: accept, and generalise.** My `margin: 0` rule was insufficient;
an always-mounted flex child earns the container's 8px `gap` at zero height. Their
`:empty { margin-top: -8px }` is correct and was measured. Folded into §4.1 with the general lesson:
the always-mounted shape is only inert if checked against the **parent's** layout.

**R3 — stale line count (§5). RULING: accept.** `settings-primitives.css` is 468 lines, not the 401 I
wrote; +42 breaches the ratchet. Its own file is also what the house rule wanted. §4.1 and §5 now
carry the **cascade-order constraint** that comes with it — the new file must be imported *after*
`settings-primitives.css` or the modifier loses on source order at equal specificity. That constraint
is new and easy to break silently, so it must be commented at the import site.

**R4 — rows 9/10 cannot honour `begin()`. RULING: half accepted; a cheaper fix exists.** They are
right that the *operation's* start is unobservable — `SettingsAccountAddForm` exposes only
`onSuccess`, and prying that open is the prop change §6.2 forbids. But `begin()` does not need the
operation's start; it needs **any** commit before the result, and the **form opening** is one:
`onSuccess` closes the form, so a second add always reopens it.

- **Row 10:** `begin('accounts')` inside the section's existing `onClick={() => setAddOpen(true)}`.
  Zero new props.
- **Row 9:** one new optional `onAddFormOpen(host)` callback on **`SettingsAccountHostGroup`** — not
  on `SettingsAccountAddForm`, so the shared form component is untouched and §6.2 holds.

**Priority: SHOULD-FIX, not MUST-FIX.** It is ~6 lines and closes a silent-announcement hole in the
component built to close silent-announcement holes; the realistic trigger is a user adding two
identities to the same host in a row, which is plausible. But it is not worth blocking the increment:
if the orchestrator defers it, record the limitation in TODO and scope AC15 to exclude rows 9/10.

**R5 — AC16 vs §8.2. RULING: accept; my error.** "AC6's count for Accounts stays at 1" contradicts
§8.2's own design — the dialog's `role="alert"` makes it 2 while shown. AC16 now asserts the half
that carries the meaning (the announcer's text stays `''`).

### 17.2 The sweep is 15, not 10 — and *why* it was 10 matters more than the five

The original list was assembled by searching **two directories**. The right question is
**reachability**: a hook that receives `pushToast` as a parameter and is wired from `App.tsx` renders
Settings rows while living in `src/hooks/`, and no directory search can see it. Ruling #24 said
"sweep every call site", so these are in scope by the ruling's own terms — the work simply was not
done.

This is the same class of error as the one that produced the contract (reading `innerText` and
concluding the toast was fine): **a check that measures the wrong property returns a confident wrong
answer.** AC1 is rewritten to enumerate every `pushToast(` in `src/` and account for each, and §13.1's
guard is redesigned around reachability, because a path-scoped lint has exactly the blind spot that
caused this.

### 17.3 Placement for the five missed call sites

**The MCP four — ordinary row slots.**

| # | Call site | What it reports | Tone | Lands in |
|---|---|---|---|---|
| 11 | `useMcpControls.ts:69` | could not start/stop the MCP server | error | the `ENABLED` switch row's help slot |
| 12 | `:80` | registered with Claude Code (`{scope}`) | success | the `REGISTER[scope]` row — **keyed by scope**, since there are two register rows (`user`, `local`) |
| 13 | `:82` | could not register | error | same scope-keyed slot |
| 14 | `:103` | could not enable/disable write access | error | the `ALLOW_WRITE` switch row's help slot |

- **State ownership.** `useMcpControls` (wired at `App.tsx:208`) owns a `useOutcomeNotes` instance and
  returns `mcpOutcomes` + `mcpAnnounce`; `SettingsPanel` threads them to `SettingsMcpSection`, which
  renders the three slots **and** the section's single `sr-only role="status" aria-live="polite"`
  announcer. The live-region element must live in the section, not in the hook's caller, for the
  per-section count to mean anything. **AI-access section live-region count: 1.**
- **`ALLOW_WRITE`'s state note is conditional** (`mcpEnabled ? <p> : undefined`,
  `SettingsMcpSection.tsx:131-138`) but the **outcome note must be unconditional** — the always-mounted
  shape. The hint slot therefore takes a fragment of both, the state note keeps its conditional render,
  and it needs a new id so `aria-describedby` can compose `{state-note-id} {outcome-id}` on the switch
  (same treatment as `dev-logs-note`, §9).
- All four have observable operation starts, so `begin()` applies and AC15 covers them.
- After this, `useMcpControls` has **no** `pushToast` caller: delete the parameter (§13.1).

**`useUiSettings.ts:287` — the hard one. RULING: a panel-level banner when Settings is open; the
toast stays when it is not.**

This is not a row outcome and must not be forced into one:

1. **The failure is global, not per-row.** The write is of the whole snapshot. Attaching it to the
   row the user last touched would name a smaller target than the failure covers — the same defect
   P91 §6.8 R5 corrected in the delete copy.
2. **It is a standing state, not a transient outcome.** After a failed write the UI shows values that
   are not on disk, and that stays true until a write succeeds. By §4.4's own disambiguation rule,
   standing state → **bordered banner**, not a barred note.
3. **It outlives the category.** The user can navigate to another category, or close and reopen
   Settings, while the condition persists; a section-scoped note would vanish while the fact remained.
4. **It is `--danger`, not `--warning`.** Unlike every other error in this contract, work *is* at
   risk: the change is in memory only and dies with the process. That is exactly the line §4.1 draws.
5. **`useUiSettings` serves the whole app, not just Settings.** Density, sidebar and graph-filter
   toggles all write settings, so this call site fires with Settings **closed**, where the toast is
   perfectly visible and correct. This is the sharpest possible illustration of "reachability, not
   directory": the same call site is broken on one surface and fine on another.

**Spec:**

- **Home.** A banner in the Settings card, above the pane content, spanning the content column so it
  is visible from **every** category. Its own class (e.g. `.settings-save-banner`) composed with the
  existing `.error-banner` recipe, so the `:empty` collapse is scoped to the new class and the shared
  13-site rule is not touched (the §5/§7 argument, applied again).
- **Channel branch.** Settings open → banner, announcer silent (the banner carries `role="alert"`).
  Settings closed → the toast, unchanged. The branch is on the **same `settingsOpen` signal** §13.1's
  dev-time guard needs; wire it once.
  *This is not the tone-routing I rejected in §3.1.2.* There, the location varied by what the backend
  returned — invisible to the user. Here it varies by **where the user is looking**, which the user
  themselves determines and can see.
- **Lifetime — it is a rendering of the existing failure streak.** `useUiSettings` already keeps
  `settingsFailureStreakRef` and already fires **one toast per streak, not per retry**
  (`:285-288`), resetting to 0 on success. The banner shows while the streak is non-zero and clears
  on the success branch. This is strictly better than the toast it replaces: the toast fired once and
  vanished while the condition persisted; the banner persists exactly as long as the condition does.
  **Note for the implementer:** the streak is a `ref` and does not re-render — mirror it in state.
- **Retry button** (`.section-action` inside the banner, the `SettingsAccountsSection.tsx:109-114`
  shape). Recommended, separable. It has a real job: the bounded backoff is 300/600/1200 ms and then
  stops, after which the pending patch sits unsent until the user happens to change something else.
  Retry calls `armSettingsSave(0)`.
- **Copy — AMENDMENT A4, flagged not applied.** The current string is
  `Could not save settings: {raw}`. In a transient toast the raw tail was merely against house rules;
  in a **persistent banner** it is a raw OS error sitting on screen indefinitely, and a permanent
  banner with no next step is a dead end. Recommended replacement:
  > **Your settings couldn't be saved. They still apply until Bonsai closes, and it will try again on
  > your next change. If this keeps happening, check that Bonsai can write to its config folder.**

  What happened / what it means / what happens next / what to do — and every clause is true of the
  mechanics at `:278-293`. **This needs the orchestrator's call**, per the rule that a string
  changing because its channel changed is flagged, not quietly rewritten. Fallback if declined: ship
  the existing string in the banner and accept a permanent raw-error line.

  > **A4 — APPROVED (orchestrator, 2026-09-14). Ship the replacement; the fallback is declined.**
  > I verified every clause against `src/hooks/useUiSettings.ts:265-295` rather than accepting the
  > claim, because this session has repeatedly found copy that misstated its own mechanics:
  > * *"They still apply"* — **true.** The values are already live in the UI; only persistence
  >   failed.
  > * *"until Bonsai closes"* — **true.** Nothing else persists them, so an unsaved patch dies with
  >   the process.
  > * *"it will try again on your next change"* — **true, and precisely right.** `:284` puts the
  >   patch **back** (`{ ...merged, ...pending }`, P69b defect 2) rather than dropping it, and the
  >   comment at `:289-291` states the bounded backoff "then wait[s] for the next change or
  >   teardown". The string describes the actual resume condition, not a hopeful one.
  > * *"check that Bonsai can write to its config folder"* — the only user-actionable cause, and it
  >   replaces a raw OS error that named no action at all.
  >
  > The **retry button is also approved** and should ship with it: without it the sentence "it will
  > try again on your next change" is the user's *only* recovery, which makes the banner a dead end
  > for anyone who has stopped changing settings — exactly the state a persistent failure produces.
  >
  > **The channel branch is approved and the distinction holds.** Routing by *where the user is
  > looking* is legitimate; routing by *what the backend returned* is not — the same rule that
  > settled the one-component-two-tones question in §3.1.2. Note what makes this call site unique:
  > it is **correct as a toast** when Settings is closed, so this is the one place in the sweep where
  > `pushToast` must survive.
- **Harness:** needs a new knob (`?settingsSaveFail=1`) — nothing in the mock currently rejects
  `setUiSettings`, so this call site, the highest-traffic Settings toast in the app, has **never been
  seen rendered by anyone**. That is its own small finding.

### 17.3a Inventory — reachable later, not today

`src/hooks/useExternalTools.ts:22` and `:34` raise toasts for external-tool Browse/launch failures.
**Out of scope today:** they are reachable only from repo UI, where a toast is correct and visible.
**In scope the moment P112 sub-increment 4 lands**, because it puts an external-tool picker *inside*
Settings, and a failure raised from there hits the same scrim as everything in §6. The P112 UI
contract already rules **"no toast for Browse errors"** for exactly this reason; this entry exists so
the two contracts cannot drift apart on it — if P112-4 ships a toast from that picker, both contracts
are violated, not one.

This is also the first worked example of AC1 doing its job: these two call sites are **accounted
for**, not swept. "Reachable from Settings" is the test, and the answer changes when the UI changes —
which is precisely what a directory search can never notice.

### 17.3b Two checks that earned their keep

Recorded because both were cheap to write and each caught something a plausible alternative would
have missed — they are the parts of this contract worth copying into the next one.

- **AC1's reachability enumeration was independently re-run and reproduced: 233 call sites,
  classification intact.** Two of its judgements were non-obvious and correct: `AiAssetsPanel` is a
  **sibling** overlay rather than a child of `SettingsPanel`, so its toasts are fine; and
  `SettingsExternalToolsSection` receives **no** launcher callbacks, so it raises none. A directory
  search would have mis-classified both — one as in scope, one as out — which is the whole argument of
  §17.2 in two concrete instances.
- **§9's "compose, never replace" rule is load-bearing, not stylistic.** `SettingsSwitchRow.tsx:46`
  passes `describedBy ?? settingsRowHelpId(id)`, so a supplied value **replaces** the help id rather
  than adding to it. A switch row that wired only the outcome id would have silently dropped its own
  help text from the accessible description — a regression with no visual symptom whatsoever. The
  composed form was confirmed in the harness with all three ids resolving. Any future row wiring an
  outcome note through `SettingsSwitchRow` must pass the **composed** string.

### 17.4 Follow-ups this leaves open

- **A4** (the save-failure copy) — needs the orchestrator's call.
- **R4's `begin()` for rows 9/10** — SHOULD-FIX; TODO line if deferred.
- ~~**Key-scoped announcer clearing** (§8.2)~~ — **WITHDRAWN 2026-09-14**, measured to reintroduce
  §8.1's silence across keys. Superseded by the text-identity invariant in §8.2 and by **AC17**. Do
  not re-propose it: a last-announced-**key** ref is not a proxy for text identity, and unlike the
  global clear it never expires.
- **A5** (the host in the two Accounts strings, §8.2) — needs the orchestrator's call.
- **AC11's cozy/compact half** — still owed for every slot.
- **`useExternalTools.ts:22, :34`** — re-evaluate when P112-4 lands (§17.3a).
- **The swallowed `forge_remove_account_inner` errors** — backend defect, filed separately.
- **A1** (raw `errorMessage(e)` in the four Accounts strings) — unchanged, still recommended.
- `AC1`'s enumeration should be re-run once, by hand, at review time: it is the only check that would
  have caught §17.2, and it has never actually been run in the form now specified.

## 18. `ui-reference.md` changes made for this contract

1. **§12.13** — the two bullets at `:2377-2390` (the Settings-toast rule and the
   live-regions-per-section rule) are replaced by a one-line pointer. They were canonical
   cross-section rules living inside the P112 picker section; the picker-specific consequence
   (Browse errors are inline) stays.
2. **New §12.14 "Outcome notes — the Settings surface has no toasts (P113, SIGNED 2026-09-14)"** —
   the measurement, the two recipes with §4.3's ratios, the tone split and its reasoning, the
   lifetime rule, the one-live-region-per-section rule (moved here intact), the dialog-stays-open
   case, §4.4's banner-vs-note disambiguation, and the guard.
3. **Amended 2026-09-14, after implementation (§17):** §12.14's guard bullet is rewritten around
   **reachability** — a path-scoped lint cannot see a hook that receives `pushToast` as a parameter,
   which is how five call sites were missed; the always-mounted-empty bullet gains the **parent
   `gap`** clause from R2; and a new bullet records the global-failure case (panel banner when the
   overlay is open, toast when it is not).

No new tokens. Every value in this contract is an existing custom property.
