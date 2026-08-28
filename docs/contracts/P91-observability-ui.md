# P91 — Observability UI contract (Dev mode)

**Scope:** increment 4 (React causality instrumentation on **six** surfaces — *no visual change*,
§9 here) and increment 7 (Settings → Developer page + the app-level logging indicator).
**Input contract:** `docs/contracts/P91-observability.md` (§6/§6.1 log commands, §7 redaction, §10
settings surface, §12 increments). All of the architect's §13 open decisions are **resolved by the
user**; the ones that touch this surface are folded in (logs persist after Dev mode is switched off;
`metrics_reset` and the Statistics page get no UI). **Additionally resolved (2026-08-27):** a
**`Delete all log files` action ships in v1**, implemented on the architect's **roll-then-purge**
model — see §8.5 and DR-1 in §11. **Out of scope:** the Statistics page (§10 here reserves placement
only).

The user story this surface exists to serve, in order:

> **enable → reproduce the flicker → turn Dev mode back off → find the file → send it → erase it.**

Every placement decision below is checked against that sequence. In particular the *export and
delete paths must keep working after Dev mode is switched off*, and **delete must actually erase
everything** — a delete that leaves the raw-names file the user was trying to remove would defeat
the reason the action exists.

---

## 1. Placement & naming

### 1.1 Decision

A **new top-level Settings rail category**, `id: 'dev'`, **label `Developer`**, last in the rail,
`dividerBefore: true`. This matches architect §10 and is not re-opened here. Pane subtitle:

> `Diagnostic logging for troubleshooting. Everything stays on this computer.`

Rail order becomes: General · Appearance · Commit graph · AI · Identities · Accounts ·
Git config `[repo]` ── **Developer**.

### 1.2 Label: "Developer", not "Dev mode" / "Debug" / "Advanced"

- The rail label names a *place*; the master switch inside it names the *state* (`Dev mode`). A rail
  item called "Dev mode" reads as a toggle and would be the second thing on screen called that.
- "Advanced" is a junk drawer and attracts unrelated future rows. "Developer" states who it is for.
- The catalog `label` for the master row is `Dev mode` so that searching either word lands the user
  in the right place (`keywords: 'debug logging diagnostics troubleshoot verbose trace log'`).

### 1.3 Discoverable by everyone — NOT hidden behind a gesture

Rejected: a hidden category revealed by clicking the version string 5×, or a `--dev` launch flag.

1. **The workflow starts with a user who cannot find the problem.** The realistic entry point is the
   user (or a support answer, or an AI telling them) typing "log" into the Settings search box. A
   hidden category cannot be searched, which breaks step 1 of the only workflow this milestone has.
2. **Nothing here is dangerous by existing.** The privacy risk lives entirely in
   `dev.include-raw-names`, which is off by default, confirmed on enable, and permanently annotated
   (§4). The master toggle only writes a redacted file to the user's own disk with no network sink
   (architect §8: "No network sink exists", asserted by a test).
3. **Nothing else in Settings is hidden either.** Every existing category is in the rail and in the
   search index; a version-click easter egg would be the only such gesture in the app.

**Restraint check:** this adds one rail item (32px) to a 200px rail that scrolls; it displaces
nothing. It adds **zero** new top-level app chrome except the conditional §5 indicator, which is
absent from the DOM whenever Dev mode is off.

### 1.4 Command palette

Two entries only, both always available:

| Palette label | Action |
|---|---|
| `Settings: Developer` | opens Settings on the `dev` category |
| `Dev mode: reveal logs folder` | `log_reveal_dir()` — the "I already have the log, where is it" shortcut |

**Do not** put a "Toggle Dev mode" command in the palette. A one-keystroke fuzzy match that silently
starts writing files to disk is exactly the accident the §5 indicator exists to catch; make it a
deliberate visit to a settings page. **And do not put `Delete all log files` in the palette** — a
fuzzy-matched destructive action is the canonical palette footgun; it lives on the page only.

---

## 2. Page layout

Standard §12.1 pane (`--bg-0`, `padding: 16px 24px 24px`), standard §12.2 rows. **Density-invariant**
— per ui-reference §3 the Settings overlay and app chrome have one geometry in both `cozy` and
`compact`; no `--dev-*` density block is introduced, and the §5 header indicator is likewise
density-invariant. Row `min-height: 44px`, `padding: 10px 0`, 1px `--border` between siblings, in
both densities and both themes.

```
Developer
Diagnostic logging for troubleshooting. Everything stays on this computer.

DEV MODE                                                    ← group title, 11px caps --text-3
  Dev mode                                          [ ● ]   ← switch, dev.enabled
  Records what the app does to a file on this computer,
  so a problem can be diagnosed after it happens.

  ┌ Recording · 2 files · 3.1 MB · 41,208 records · 2 flagged  ┐   ← §6 status card
  └ bonsai-2026-08-27T14-03-11-a3f9.jsonl                      ┘

WHAT IS CAPTURED                                            ← <fieldset disabled> when master off
  Detail level                          [ Info | Debug | Trace ]
  Higher levels record more and produce larger files.

  App requests                                       [ ● ]
  Commands sent to the Git engine, their timing and result.

  Screen updates                                     [ ● ]
  Which parts of the window re-render, and why.

  Frame timing                                       [ ○ ]
  Drawing performance of the commit graph. High volume.

  ▌ Include raw repository names                     [ ○ ]   ← sensitive row, --warning leading bar
  ▌ Off: names appear as ref#3, path#7, repo#1. On: your real
  ▌ branch, tag, file and repository names are written to the
  ▌ log file.

WHAT A LOG FILE CONTAINS                                    ← always live, never disabled
  <the §7 privacy statement — static prose block>

LOG FILES                                                   ← always live, never disabled
  Log files                    [ Show in folder ] [ Export session… ]
  Kept in {dir} after you turn Dev mode off. Bonsai keeps the
  10 most recent sessions and deletes older ones automatically.
  ──────────────────────────────────────────────────────────
  Delete all log files                        [ Delete logs… ]   ← §8.5, danger, own row, last
  Removes all 5 log files (15.8 MB) from this computer.
```

### 2.1 Which rows are gated by the master switch

The split is **by workflow, not by topic**.

| Group | Gated by `dev.enabled`? | Why |
|---|---|---|
| DEV MODE (`dev.enabled` + status card) | n/a | the gate itself |
| WHAT IS CAPTURED (`level`, `capture-ipc`, `capture-react`, `capture-frames`, `include-raw-names`) | **yes — `<fieldset disabled>`** | they configure capture; meaningless with capture off |
| WHAT A LOG FILE CONTAINS | **no** | it is the text that decides whether the user *turns it on*; useless if it only appears afterwards |
| LOG FILES (reveal / export / **delete**) | **no** | **the workflow's last three steps happen after Dev mode is turned off, and logs deliberately persist.** Gating any of them breaks the milestone. Delete in particular must work **while Dev mode is on** — a user who realises mid-debug that raw names are recording needs to erase *now*, not after a settings detour (§8.5.4) |

### 2.2 Disabled, not hidden — justification

Dependent rows use the ui-reference **§12.3.3** mechanism verbatim: one `<fieldset disabled>` around
the WHAT IS CAPTURED group, `.55` dim on `.settings-row.is-disabled` only (never on the fieldset,
never nested), the reason leading the group as `.settings-group-lead` with an id that the
`<fieldset>` names via `aria-describedby` **only while it exists**.

- **Preview before commit.** The user decides whether to turn Dev mode on partly by seeing what it
  would capture. Hidden rows make that a leap of faith.
- **No layout jump.** Hiding five rows moves the privacy statement and the buttons ~230px up the
  moment the master flips — on the exact click where the user is reading them.
- **Simpler catalog.** Conditional rendering is a supported, tested pattern here
  (`SettingsRowRequirement`: `'repo'`, `'aiActive'`, `'mcpRunning'`, `'profile'`) — this is not an
  argument that hiding is impossible, only that it is unnecessary: disabling needs **no** new
  requirement predicate and no new coverage-test branch. The two arguments above are what decide it.

Group lead sentence, present **only while `dev.enabled` is off**:

> `Turn on Dev mode to change what is recorded.`

---

## 3. Component decomposition

All new files; nothing is appended to an existing file. Soft cap ~500 lines, expected far below.

| File | Responsibility | ~lines |
|---|---|---|
| `src/components/settings/catalog/dev.ts` | `DEV_ENTRIES: readonly SettingsIndexEntry[]` (§13) | ~130 |
| `src/components/settings/categories/DevPage.tsx` | container: reads `useSettingsValues()/useSettingsActions()`, holds `LogSessionInfo` polling, reveal/export/**delete** handlers, confirm-dialog state | ~210 |
| `src/components/settings/SettingsDevModeSection.tsx` | group 1 — master switch + `<DevSessionStatus>` | ~70 |
| `src/components/settings/SettingsDevCaptureSection.tsx` | group 2 — the `<fieldset>` with level + 3 switches + the sensitive raw-names row | ~130 |
| `src/components/settings/SettingsDevPrivacySection.tsx` | group 3 — the frozen §7 statement (pure; props = `redaction`) | ~90 |
| `src/components/settings/SettingsDevLogsSection.tsx` | group 4 — reveal + export + **delete** rows, all empty/busy/error states | ~170 |
| `src/components/settings/DevSessionStatus.tsx` | the §6 status card (presentational; props = `LogSessionInfo \| null`, `enabled`, `state`) | ~110 |
| `src/components/DevModeIndicator.tsx` | the §5 header chip (presentational + one click handler) | ~60 |

Registration: `SettingsCategoryId` gains `'dev'`; `SETTINGS_CATEGORIES` gains the entry;
`CATEGORY_PAGES` maps `dev → DevPage`; the catalog barrel spreads `DEV_ENTRIES`.
`DevModeIndicator` is rendered by `HeaderToolbar.tsx` (one line, conditional).

**Reuse, not invention.** Everything here is an existing primitive: `SettingsGroup`, `SettingsRow`,
`SettingsSwitchRow`, the §12.3.2 segmented radiogroup, `.btn-secondary` / `.btn-danger`,
`ConfirmDialog` (both variants), the §10.2 toast recipe, `.settings-row-note`, and the `'group'`
control kind (`types.ts` AM-2). The only genuinely new visuals are the **sensitive-row treatment**
(§4.2) and the **header chip** (§5) — both justified below, both built from existing tokens.

---

## 4. Control hierarchy

### 4.1 The benign controls

| Row id | Control | Label | Help (catalog, static) |
|---|---|---|---|
| `dev.enabled` | switch | `Dev mode` | `Records what the app does to a file on this computer, so a problem can be diagnosed after it happens.` |
| `dev.level` | segmented (3, max per §12.3.2) | `Detail level` | `Higher levels record more and produce larger files.` |
| `dev.capture-ipc` | switch | `App requests` | `Commands sent to the Git engine, their timing and result.` |
| `dev.capture-react` | switch | `Screen updates` | `Which parts of the window re-render, and why.` |
| `dev.capture-frames` | switch | `Frame timing` | `Drawing performance of the commit graph. Records a lot of data.` |

Segment labels: `Info` · `Debug` · `Trace`. Selected segment gets `--selection` + 600 weight +
`--accent` border (the two non-colour carriers, §12.3.2). Reset `↺` per row via
`resetField('dev', <field>, <'On'|'Off'|'Debug'>)`.

**Labels avoid jargon the user did not choose.** "App requests" not "IPC"; "Screen updates" not
"React renders"; "Frame timing" not "rAF/paint". The house rule (§12.2) that two rows in one dialog
never share an accessible name holds — all five are distinct.

**Trace + frame-capture interaction.** Architect §5 makes `level: 'trace'` force-enable frame
capture. The UI must not lie about a switch's position: when `level === 'trace'`, the
`dev.capture-frames` switch renders **checked and disabled** (inside the fieldset it already sits in,
so use `disabled` on the input plus `.settings-row.is-disabled`), and the row swaps its static help
for a `.settings-row-note` (never both, §12.2):

> `Always on at the Trace detail level.`

### 4.2 The sensitive control — `dev.include-raw-names`

This one must **not** read like the three switches above it. Treatment, all from existing tokens:

- New row modifier `.settings-row--sensitive`: `padding-left: 12px;`
  `box-shadow: inset 3px 0 0 var(--warning);` — the §10.2 leading-bar recipe, reused verbatim.
  Label stays `--text-1`, help stays `--text-2`. **No tinted background, no coloured label** (§2
  forbids hue-as-text; a filled row would also fight the switch).
- A 12px `--warning` shield-outline glyph sits before the label (`aria-hidden`), so the meaning is
  carried by **bar + glyph + words**, never by hue alone.
- Label: `Include raw repository names`
- The row uses `.settings-row-note` (call-site, value-tracking — so it carries **no** catalog
  `help`, per §12.2), with two texts:
  - off → `Off: names appear as ref#3, path#7, repo#1.`
  - on → `On: new log files contain your real branch, tag, file and repository names.`
- While **on**, the group additionally renders a `.settings-group-note` at the foot of the fieldset,
  same leading-bar treatment, id wired into the row's `aria-describedby` alongside the note id:

  > `Raw names are on. Read an exported log before sending it to anyone.`

- `--warning` as a 3px bar / glyph on `--bg-0`: **7.3:1** dark / **4.5:1** light (§2 measured) —
  clears the 3:1 graphics bar in both themes. No new token.

**Note the deliberate contrast with §8.5:** the raw-names switch is *sensitive* (`--warning` bar,
`primary` confirm) because it is reversible and destroys nothing; `Delete logs…` is *destructive*
(`--danger` button, `danger` confirm) because it loses data. Two different weights, two different
treatments — that distinction is the point of the pattern and must not be flattened. Note also that
the two rows are each other's remedy: the raw-names row is how the user creates the exposure,
`Delete logs…` is how they undo it, and the second must therefore reach **every** file the first
produced — including the one being written right now (§8.5.4).

### 4.3 The raw-names confirmation

Existing `ConfirmDialog`. Fires **only on enable**; turning it back off is silent and instant
(un-sharing is never confirmed).

- Title: `Include raw repository names in logs?`
- Body (three short paragraphs, plain language):
  > `New log files will contain the real names of your branches, tags, files and repository — instead of placeholders like ref#3 and path#7.`
  >
  > `Commit messages, file contents, author names and email addresses are still never written, and passwords and access tokens are never written in any mode.`
  >
  > `Bonsai starts a new log file now, so the file you are recording into does not mix the two settings. Read an exported log before sending it to anyone.`
- Primary: `Include raw names` · Secondary: `Cancel` (default focus: **Cancel**).
- `confirmVariant: 'primary'`, **not** `'danger'`. Justification: nothing is destroyed or
  irreversible — the setting is a switch the user can flip back, and it affects only files created
  afterwards. `danger` styling is reserved in this app for data loss (§12.7 sign-out, branch delete,
  and now §8.5); spending it here devalues it. The **privacy** weight is carried by the `--warning`
  row treatment, the persistent note, and the dialog's explicit consequence copy — not by a red
  button.
- Esc / backdrop / Cancel → leaves the switch **off** (the switch must not flip optimistically).
  Enter → confirms. Focus trap + restore to the switch, per the existing `ConfirmDialog`.

---

## 5. App-level "logging is active" indicator

### 5.1 Placement & chrome cost

`DevModeIndicator` renders in `.header-toolbar` **immediately left of the settings gear**
(order becomes: theme · list view · AI assets · health · **dev** · settings · identity).

**It is absent from the DOM whenever `dev.enabled` is false** — the same conditional-render
discipline the toolbar's repo-scoped controls already use (§1). So the cost against the "new header
chrome is expensive" rule is: **zero pixels in the default state**, and ~92px only while the user has
deliberately asked the app to record. It displaces nothing; the toolbar has room.

Rejected alternative: the §10 App notice bar. That bar is defined as a **global fault** channel; Dev
mode being on is a user-chosen state, not a fault, and re-defining the bar's semantics is a larger
change than this milestone warrants. See §11 (OQ-1) for an optional escalation.

### 5.2 Geometry & appearance

- Pill button, `height: 24px; padding: 0 8px; border-radius: 999px; gap: 6px;`
  `background: color-mix(in srgb, var(--warning) 14%, transparent); border: 1px solid
  color-mix(in srgb, var(--warning) 45%, transparent);` label 12px `--text-1`.
- **Contrast note / re-measure item:** the §2 figure for `--text-1` over a hue's 14% tint
  (**9.24–10.30:1** dark / **11.68–12.00:1** light) was measured over `--bg-2`; the header is
  `--bg-1`. The pair is comfortably above the bar either way, but the exact number must be
  re-measured on `--bg-1` in the P91 design-review pass and folded into §2.
- Leading glyph: an 8px filled `--warning` circle (a record dot), `aria-hidden`.
- Label text: **`Dev mode`** at rest. `Recording` is *not* used as the label — the word must be stable
  so it does not flicker as records arrive. Write-activity lives in the tooltip and the Settings
  status card instead (§6), where it can be read rather than glanced at.
- Hit target: the pill is 24px tall inside a 32×32 button box (§3.1 floor satisfied).
- Hover `--bg-2` overlay; pressed `--bg-3`; `:focus-visible` 2px `--accent`, offset 1px.
- `title` / `aria-label`: `Dev mode is on — Bonsai is writing a diagnostic log. Open Developer settings.`
- Click → opens Settings on the `dev` category (same handler shape as the gear).
- **Motion:** the record dot fades between `opacity: 1` and `.55` over **1200ms ease-out** while
  records were written in the last 2s, and is solid otherwise. Opacity only, no transform, no layout,
  not on the graph's compositor path. Under `prefers-reduced-motion: reduce` the animation is removed
  entirely and the dot is solid — the pill's presence, not the pulse, carries "logging is on".
- **The pill does not change during a purge.** Roll-then-purge keeps Dev mode on throughout, so the
  pill stays mounted and stays saying `Dev mode`; flashing it off and on would imply recording had
  stopped, which is exactly the misunderstanding §8.5 is written to prevent.
- Both themes: identical recipe; `--warning` resolves per theme.
- Densities: unchanged (app chrome is density-invariant, §3).

### 5.3 Announcement

On transition off→on and on→off the indicator's mount point contains a visually-hidden
`aria-live="polite"` region that announces exactly once:

- on: `Dev mode on. Bonsai is writing a diagnostic log to this computer.`
- off: `Dev mode off. Logging stopped. Your log files are kept.`

The live region is **permanently present and empty when idle** (§12.3.4 precedent) so the
announcement is not lost to a late-mounted region. It is *not* re-announced on record activity —
that would be a screen-reader denial-of-service. It is reused for the §8 action results (export,
delete) — one region per page, not one per action.

---

## 6. Status feedback — `DevSessionStatus`

A **card, not a settings row**: it is a live readout with five values, and stuffing it into a row's
help cell would violate the "one help line per row" rule. It is therefore catalogued as
`control: 'group'` (`types.ts` AM-2): stamped on a `role="group"` element named by its heading via
`aria-labelledby`, carrying **no** `[data-setting-control]`, with runtime-generated children that are
not individually catalogued. It sits directly under the master switch row, inside the DEV MODE group.

```
margin: 8px 0 0; padding: 12px; border-radius: 6px;
background: var(--bg-1); border: 1px solid var(--border);
```

Row 1 (12px, `--text-2`, `display:flex; gap:8px; flex-wrap:wrap`) — dot-separated facts:

`● Recording · 2 files · 3.1 MB · 41,208 records · 2 flagged`

- `● ` is a 6px `--warning` dot when writing in the last 2s, `--text-3` otherwise; **the word beside
  it carries the state** (`Recording` / `Idle`), never the colour alone.
- `n flagged` renders only when `anomalies > 0`, in `--text-1` 600 weight with a
  `title="Possible problems Bonsai noticed and marked in the log"`.
- `dropped > 0` appends `· n records dropped` in `--text-1` with
  `title="The log was written faster than the disk could keep up. Timing in the file may be incomplete."`

Row 2 (12px `--text-2` — `--text-3` is forbidden for read text; mono font, `text-overflow: ellipsis`,
`title` = full name): the current session file name. Truncates, never wraps.

Row 3, **only while `includeRawNames` is on**: the §4.2 leading-bar warning line repeated here, so a
user who scrolled past the toggle still sees it next to the file they are about to export.

Polling: `log_session_info()` on page mount, on every settings change, **immediately after a delete
completes (success, partial or failure)**, and on a **2s interval while the Settings Developer page
is visible and `dev.enabled` is true**. Never polls when the overlay is closed or the category is not
selected — this must not become background load. Formatting: bytes via the existing house formatter
(`3.1 MB`), counts with thousands separators from `Intl.NumberFormat`.

**After a roll-then-purge with Dev mode on**, the card describes the **new** session, not the deleted
one: `● Recording · 1 file · 0 bytes · 0 records`, with row 2 showing `LogsDeleteResult.activeFile`
(a file **name**, never a path). The record count resetting to zero is the user-visible proof that
the old file is gone and a fresh one is being written — it must not carry the old counts forward for
even one poll cycle, so the post-delete refresh is driven by the delete's own completion, not by the
2s timer.

`aria-live` is **not** applied to the card — it changes every 2s. The group is reachable in
screen-reader browse mode; the one-shot transitions are announced by §5.3 instead.

**After Dev mode is switched off** the card does not disappear: it renders one line —
`Idle · last session: 2 files · 3.1 MB · kept on this computer` — so the persistence of the files is
visible at the moment the user turns recording off. With no logs ever recorded, or after a purge with
Dev mode off, it renders `No logs yet.` in `--text-2`.

---

## 7. The privacy statement (frozen microcopy)

Rendered by `SettingsDevPrivacySection.tsx`, catalogued as `control: 'group'` for the same reason as
§6 (it is a prose block, not a value row): a `role="group"` named by its heading via
`aria-labelledby`, no `[data-setting-control]`. **Always visible, never inside the disabled
fieldset.** Group title: `WHAT A LOG FILE CONTAINS`. Body: 12px `--text-2`, `max-width: 56ch`, 8px
paragraph gap. The `contains` / `never contains` lists are `<ul>`s with 4px item gap.

> Bonsai writes log files **only to this computer**. It never uploads them, and nothing here sends
> anything over the network.
>
> **A log file contains:** what you clicked, which commands the app ran, how long they took, whether
> they succeeded, how often the screen redrew, and problems Bonsai flagged along the way.
>
> **A log file never contains — in any mode:** your commit messages, the contents of your files or
> diffs, your name or email address, your search text, or any password, access token or key.
>
> **Names are replaced by default.** Your branches, tags, files and repository appear as `ref#3`,
> `path#7`, `repo#1`. The same name keeps the same number inside one file, so a problem can still be
> followed — and the numbers change next time, so two files cannot be matched up.
>
> **If you turn on "Include raw repository names"**, that replacement stops: new log files contain
> your real branch, tag, file and repository names. Everything in the "never contains" list above
> stays excluded.
>
> **Log files stay on this computer until you delete them.** They are kept when you turn Dev mode
> off, so you can export one afterwards. Bonsai keeps the 10 most recent sessions and removes the
> oldest automatically, and "Delete all log files" below removes every one of them — including the
> session being recorded right now.
>
> Log files are plain text, one record per line. You can open one in any text editor and read it
> before you send it anywhere.

Last line changes with the live mode (a `.settings-group-note`, value-tracking):

- strict → `Right now: names are replaced.`
- raw → `Right now: raw names are included.` (leading `--warning` bar, per §4.2)

**Copy rules applied:** no "redaction", no "telemetry", no "IPC", no "hashing", no "salt". "Replaced
by default" beats "conservatively redacted" for the reader who has to decide whether to send it. The
"cannot be matched up" sentence is the honest, jargon-free rendering of the per-session salt. The
persistence paragraph names the delete action **and its completeness** — "including the session being
recorded right now" is load-bearing: a user reading this paragraph while raw names are on is asking
exactly whether the file they are worried about is covered, and the answer must be on the page, not
discovered in a dialog.

---

## 8. Actions — reveal, export, delete

### 8.1 Rows

The LOG FILES group holds **two** rows, in this order:

1. `dev.logs` — `.settings-row--stacked`, label `Log files`, two secondary buttons side by side
   (`display:flex; gap:8px`), plus a call-site `.settings-row-note`.
2. `dev.delete-logs` — a standard (non-stacked) row, label `Delete all log files`, one danger button
   in the control cell, plus a call-site `.settings-row-note`. **Last row on the page.**

| Control | Row | Class | Label | Icon (§13 SVG chrome) |
|---|---|---|---|---|
| reveal | `dev.logs` | `.btn-secondary` | `Show in folder` | folder-open, 14px, leading |
| export | `dev.logs` | `.btn-secondary` | `Export session…` | box-arrow-down, 14px, leading |
| delete | `dev.delete-logs` | `.btn-danger` | `Delete logs…` | trash, 14px, leading |

The two benign actions and the destructive one are **in different rows, separated by the standard 1px
`--border`** — never shoulder-to-shoulder in one flex line. `Export session…` and `Delete logs…`
would otherwise be adjacent 100px buttons with the same visual weight and opposite consequences,
which is a misclick generator. Row separation plus `--danger` fill plus the confirm dialog are three
independent brakes.

`dev.logs` note (value-tracking, so no catalog `help`):

- has logs → `Kept in {dir} after you turn Dev mode off. Bonsai keeps the 10 most recent sessions and deletes older ones automatically.` (`{dir}` truncated with `title`, mono)
- no logs → `No logs yet. Turn on Dev mode, reproduce the problem, then export the session.`

`Show in folder` label rationale: it is what Windows Explorer, Finder and most Linux file managers
call it, and it is honest when the folder exists but is empty. Not "Reveal in Finder" (macOS-only
phrasing), not "Open logs directory" (jargon).

**This page still has no primary action.** All three buttons are secondary-weight controls; the
master switch is what the user came for. `.btn-danger` is a *destructive* style, not a *primary*
one — it must not be the only filled button competing for the eye, which is another reason it sits
in its own row at the very bottom.

### 8.2 States — reveal & export

| State | `Show in folder` | `Export session…` |
|---|---|---|
| Dev mode off, **logs exist** | enabled | enabled |
| Dev mode off, **no logs ever** | **disabled**, `title="No logs yet."` | **disabled**, same title |
| Dev mode on, session started | enabled | enabled |
| Working | `busy` — label unchanged, 14px spinner replaces the icon, `aria-busy="true"`, non-activatable | same |
| Failed | button returns to rest; failure surfaces as a toast (§8.3) | same |

Disabled buttons keep `.55` dim (spent once, §2 dimming budget) and remain in the tab order via
`aria-disabled` **not** `disabled`, so a keyboard user can reach them and hear why — the `title`
string is duplicated into `aria-describedby` on a hidden span, since `title` is not reliably
announced.

### 8.3 Export flow

1. Click `Export session…`
2. **Content-statement dialog** (architect §10 requires re-showing it). Existing `ConfirmDialog`,
   `confirmVariant: 'primary'`:
   - Title: `Export this session's log`
   - Body:
     > `You will get a .zip containing this session's log files ({n} files, {size}).`
     >
     > `{strict}` → `Branch, file and repository names are replaced with placeholders. Commit messages, file contents, names and email addresses, passwords and tokens are not in the file.`
     > `{raw}` → `Raw names are on: this log contains your real branch, tag, file and repository names. Commit messages, file contents, names and email addresses, passwords and tokens are still not in the file.` *(leading `--warning` bar block + shield glyph)*
     >
     > `Open the file and read it before sending it to anyone.`
   - Primary: `Choose location…` · Secondary: `Cancel`
3. Native save dialog (`log_export_session(dest)`).
4. Cancelled save dialog → **no toast**, no error; the user chose nothing.
5. Success → success toast (§10.2 recipe): `Session log exported.` with action button
   **`Show in folder`** → reveals the export's parent directory. This is the "find the file" step of
   the workflow, so it must not require a second hunt.
6. Failure → danger toast, dedupe key `dev-export`:
   - generic: `Couldn't export the log. {reason}` where `{reason}` is a mapped sentence, never raw
     libgit2/OS text.
   - disk full → `Not enough space to write the export. Free some space and try again.`
   - permission → `Bonsai isn't allowed to write there. Choose a different folder.`
   - folder missing / logs pruned mid-flight → `Those log files are no longer there. Turn on Dev mode and reproduce the problem again.`

`Show in folder` failure (directory missing): danger toast, dedupe key `dev-reveal` —
`Couldn't open the logs folder. It may have been moved or deleted.`

### 8.4 Disk-write error while recording

If the sink reports a write failure, `LogSessionInfo` surfaces it and the UI shows it in **two**
places (never only a transient toast — the user may be mid-repro and not looking):

1. `DevSessionStatus` row 1 replaces `● Recording` with `▲ Not writing` (`--danger` triangle glyph +
   the words; `--danger` glyph on `--bg-1` = **4.4:1** dark / **4.6:1** light, clears the graphics
   bar), and row 2 is replaced by:
   `Bonsai stopped writing the log. {reason} Turn Dev mode off and on to try again.`
2. One danger toast, dedupe key `dev-sink`, same first sentence.

The §5 header pill in this state swaps its dot to a `--danger` triangle glyph and its label to
`Not logging`, with `aria-label="Dev mode is on but Bonsai stopped writing the log. Open Developer
settings."` — the pill must never claim to be recording when it is not.

### 8.5 `Delete all log files` — roll-then-purge (v1, decision DR-1)

The one destructive action on this page. Ships in v1 because §7 promises the user control over files
that otherwise sit on disk indefinitely, and a promise the UI cannot keep is worse than no promise.

**Backend contract:** `logs_delete_all()` per `P91-observability.md` §6/§6.1, returning

```ts
interface LogsDeleteResult {
  deletedFiles: number;
  deletedBytes: number;
  failedFiles: number;
  /** NAME of the NEW session file (never a path). Meaningful when `rolled`. */
  activeFile: string;
  /** true ⇒ Dev mode was on: the writer rolled to a fresh file and recording continues. */
  rolled: boolean;
}
```

**The model, in one line: roll, then purge.** With Dev mode on, the writer flushes and closes the
current file (releasing the handle), opens a fresh session file whose header carries
`afterPurge: true`, and only then every `*.jsonl` in the folder is sized and deleted. **Everything the
user had is erased — including the session they were recording.** With Dev mode off there is no
writer, everything is deleted, and `rolled` is `false`.

#### 8.5.1 Why the UI is written around roll-then-purge

This is the whole design, so it is stated rather than assumed:

- **There is no open handle at delete time.** No Windows sharing violation, no Unix unlinked-inode
  writing invisibly into a deleted file. Behaviour is byte-for-byte identical on all three platforms,
  and the partial-failure path is reserved for genuinely *external* locks (a text editor holding the
  file open) rather than firing every time on one OS.
- **It is the only model that satisfies the feature's purpose.** The file being written right now is
  precisely the one holding the raw-names records a worried user is trying to erase. Any design that
  spares it would show a success toast while the exposure survives on disk — the exact failure this
  action exists to prevent.
- **Consequently the UI never mentions "keeping" anything, and never asks the user to turn Dev mode
  off first.** That instruction would be both false and unnecessary.

#### 8.5.2 Row & control

- Row id `dev.delete-logs`, label **`Delete all log files`**, last row of the LOG FILES group.
- Button `.btn-danger`, label **`Delete logs…`** (ellipsis: it opens a confirm), trash glyph. The
  button label is shorter than the row label deliberately — the row label is the accessible context,
  and repeating `Delete all log files` inside the row would read twice in a row to a screen reader.
  The button carries `aria-describedby` pointing at the row note, so the count is announced with it.
- The button is **never** the visually dominant control on the page (§8.1).
- `.settings-row-note` (value-tracking, so **no** catalog `help`), four texts:

  | Condition | Note |
  |---|---|
  | logs exist, Dev mode **off** | `Removes all {n} log files ({size}) from this computer.` |
  | logs exist, Dev mode **on** | `Removes all {n} log files ({size}), including the one being recorded now. Recording continues in a new file.` |
  | no logs | `No log files to delete.` |
  | deleting | `Deleting…` |

  **`{n}` and `{size}` include the active session's file in both cases.** The user is never shown a
  number smaller than what will actually be deleted.

#### 8.5.3 No-logs state — **disabled, not hidden**

**Decision: disabled** (`aria-disabled="true"`, `.55` dim, note reads `No log files to delete.`),
matching the reveal/export treatment two rows above.

1. **Consistency inside one group beats local optimality.** Reveal and export are already
   disabled-not-hidden in this exact state; a third control that vanishes instead would make the
   group's foot jump by 44px on the same transition that leaves its siblings in place.
2. **The affordance must be learnable before it is needed.** A user reading the §7 privacy statement
   ("'Delete all log files' below removes every one of them") must be able to *see* the control the
   sentence refers to, even with nothing to delete yet.
3. **No layout jump after use.** Deleting is precisely the action that empties the state; hiding the
   button would make it disappear under the user's cursor the instant it succeeded — the classic
   destructive-action-disappears-on-success defect.
4. **Catalog simplicity:** no new `SettingsRowRequirement`, no coverage-test branch (§2.2).

It stays in the tab order via `aria-disabled` (not `disabled`) so a keyboard user hears why.

Note the state is genuinely reachable **with Dev mode on**: a purge that rolls into a fresh 0-byte
file leaves exactly one log file on disk. The row does **not** go empty in that case — the new file is
itself deletable — so the note returns to the Dev-mode-on variant with `{n} = 1`.

#### 8.5.4 Confirm dialog — **final copy**

Existing `ConfirmDialog`, **`confirmVariant: 'danger'`** — unlike the raw-names toggle (§4.3), this
*is* data loss and it *is* irreversible, which is exactly the distinction ui-reference §12.11 draws.

- Title: `Delete all log files?`
- Body:
  > `Delete {n} log files ({size})? This cannot be undone.`
  >
  > *(only when Dev mode is on)* `This includes the session being recorded right now. Bonsai starts a new, empty log file and keeps recording.`
  >
  > `Deleting logs does not change any of your repositories.`
- Primary: `Delete logs` (verb + object, never `OK`) · Secondary: `Cancel`
- **Default focus: `Cancel`.** Esc / backdrop / Cancel → nothing happens. Enter activates the focused
  control, which is Cancel — a destructive dialog must not be dismissible-into-action by a stray
  Return.
- `{n}` / `{size}` are **re-read from `log_session_info()` when the dialog opens**, not carried from
  the last poll, so the number the user consents to is the number that gets deleted.
- The second paragraph does two jobs in two sentences: it confirms the scope the user is most likely
  to be uncertain about (*is the file I'm worried about included?* — yes), and it pre-empts the fear
  that deleting will stop their in-progress capture. It never tells them to turn Dev mode off.
- The third paragraph exists because this page sits inside a Git client: a user nervous enough to read
  a red dialog deserves to be told that nothing in their repository is at stake.

#### 8.5.5 States

| State | Button | Note | Elsewhere |
|---|---|---|---|
| No logs | `aria-disabled`, `.55` dim | `No log files to delete.` | — |
| Idle, logs exist | enabled `.btn-danger` | `Removes all {n} log files ({size})…` | — |
| Confirm open | focus ring retained underneath; dialog traps focus | unchanged | — |
| Deleting | `aria-busy="true"`, non-activatable, label **unchanged** (`Delete logs…`), trash glyph replaced by a 14px spinner | `Deleting…` | reveal + export also `aria-disabled` for the duration — the folder is being mutated |
| Success, `rolled: false` (Dev mode off) | returns to the disabled/no-logs state | `No log files to delete.` | success toast + live announcement; status card → `No logs yet.` |
| Success, `rolled: true` (Dev mode on) | **stays enabled** | `Removes all 1 log file (0 bytes), including the one being recorded now. Recording continues in a new file.` | success toast + live announcement; status card immediately re-polls to `● Recording · 1 file · 0 bytes · 0 records` + `activeFile` |
| Partial failure (`failedFiles > 0`) | returns to enabled | recomputed from the fresh `LogSessionInfo` | danger toast |
| Total failure | returns to enabled | unchanged | danger toast |

**Toasts** (§10.2 recipe), dedupe key `dev-delete` — **final copy**:

- `rolled: false` → success: `Deleted {n} log files. {size} freed.`
- `rolled: true` → success: `Deleted {n} log files. {size} freed. Still recording — Bonsai started a new log file.`
- partial (`failedFiles > 0`) → danger: `Deleted {n} of {total} log files. {failedFiles} could not be deleted — they may be open in another program.` with an action button `Show in folder` so the user can finish the job manually.
- total failure → danger: `Couldn't delete the log files. {reason}` — mapped sentences only
  (`Bonsai isn't allowed to delete files in that folder.` / `The logs folder is no longer there.`),
  never raw OS text.

The `rolled: true` success string is the one piece of copy that must not be shortened: without
"Still recording", a user who just deleted the file they were capturing into will reasonably assume
capture stopped and go re-toggle Dev mode — which would roll *again* and cost them the records
gathered since the purge.

Failure never leaves the UI stale: `log_session_info()` is re-polled after **every** outcome,
including failures, so the row note and the status card always describe what is actually on disk.

#### 8.5.6 Accessibility

- **Focus destination after the dialog closes** — three cases, all explicit:
  - **Cancel / Esc / backdrop** → focus returns to the `Delete logs…` button (standard
    `ConfirmDialog` restore).
  - **Confirmed, button remains enabled** (`rolled: true`, partial failure, total failure) → focus
    returns to the `Delete logs…` button.
  - **Confirmed and the button becomes disabled** (`rolled: false`, everything deleted) → focus must
    **not** be lost to `<body>` (it would strand the tab ring inside the modal settings overlay).
    Because the button is `aria-disabled` rather than `disabled` it remains focusable, so focus still
    returns to it and the screen reader hears the button's name plus its updated `aria-describedby`
    note (`No log files to delete.`). This is the concrete payoff of the `aria-disabled` choice in
    §8.2 and is not optional.
- **Live-region announcement** reuses the single §5.3 polite region (one per page), announced once
  per outcome:
  - `Deleted {n} log files. {size} freed.`
  - `Deleted {n} log files. {size} freed. Still recording in a new log file.`
  - `Deleted {n} of {total} log files. {failedFiles} could not be deleted.`
  - `No log files were deleted.`
  The region is `polite`, never `assertive` — the user initiated this and is not in danger.
- The dialog is the existing `ConfirmDialog`: `role="dialog" aria-modal="true"`, labelled by its
  title, focus-trapped, Esc closes, focus restored per above.
- Hit target: the button is 28px tall in a 44px row — above the §3.1 floor.
- Colour is not the sole carrier: the word `Delete`, the trash glyph and the `.btn-danger` fill all
  agree. `.btn-danger` text on `--danger` is the existing house pair and is unchanged here.
- `prefers-reduced-motion: reduce` → the busy spinner becomes a static glyph with `aria-busy`.
- Both themes: `.btn-danger` and `ConfirmDialog` danger variant are existing components; no new
  colour pairs are introduced by this row.

---

## 9. Increment 4 — zero visual change (hard constraint)

Increment 4 adds `useRenderCount` / `useTracedEffect` / `useStateTransitionLog` to **six** surfaces:

1. `RepoWorkspace` container + its refresh/subscription hooks
2. `DiffBrowser`
3. `GraphCanvas`
4. the right-panel tab container
5. the PR panel
6. **the left sidebar** — `src/components/sidebar/` (the sidebar container, `rows.tsx`,
   `TagsSection.tsx`) — **added by the user**, because the left pane is one of the places the
   flickering was actually observed

**Constraint:** the instrumentation must produce **no** change to rendered output, DOM structure,
class names, inline styles, computed styles, layout, focus order, scroll position, or ARIA — with Dev
mode **on** or **off**. It adds no wrapper elements, no `<Profiler>`, no extra provider that
re-renders children, and no dev-only badge/overlay of any kind. Hook order stays unconditional
(architect §9). It must not change *when* a component renders — instrumentation observes, it never
schedules: no `setState` from a logging hook, no extra effect that can loop, no new memo boundary
that changes referential identity for children.

### 9.1 ui-designer-side verification (harness, `VITE_MOCK_IPC=1`)

**Signature** = per instrumented root,
`{ nodes: el.querySelectorAll('*').length, cls: [...el.querySelectorAll('*')].map(n=>n.className), box: el.getBoundingClientRect(), scroll: el.scrollTop, focus: document.activeElement?.dataset }`,
read in **one batched** `javascript_tool` call.

**A. Idle snapshot parity — all six surfaces.**

1. With Dev mode **off**: `read_page` (`filter: 'all'`) + the signature call.
2. Toggle `dev.enabled` on (no reload — architect §10 requires immediate effect). Re-capture.
3. **Assert identical** node counts, class lists, boxes, scroll offsets; `read_page` trees equal. Any
   delta is a MUST-FIX.

**B. High-churn parity — mandatory for the sidebar.** The sidebar re-renders on **every ref change**,
so an idle snapshot proves almost nothing about it.

1. Drive a scripted churn sequence with Dev mode off: checkout across 5 branches, then a mocked fetch
   introducing ~40 new remote refs at once, then a mocked ref deletion, then a section
   collapse/expand.
2. **Capture the sidebar signature at awaited step boundaries only** — each mock operation resolves
   and React has flushed before the capture. Sub-step transients are deliberately *not* asserted:
   two runs can legitimately interleave differently, and a check that compares un-awaited
   intermediate frames would flake and get deleted. What the transients look like is exactly what the
   log records; whether they *flicker* is the USER CHECKPOINT below.
3. Repeat the identical sequence with Dev mode on.
4. **Assert the two boundary-signature sequences are equal step-for-step** (not just the end state),
   including the selected-row identity and keyboard focus at each boundary.
5. Repeat A+B once in light theme and once at 720px width (the rail-collapse breakpoint) — the
   sidebar is the pane most affected by narrow widths.

**C. Screenshot** one final proof per theme, only if A and B pass.

**USER CHECKPOINT (not verifiable here):** whether the observed flickering is *gone* or merely
*recorded*; frame-timing parity with Dev mode off (§11 budget: ≤1%); and whether rapid branch
switching still feels smooth — the harness is headless, `requestAnimationFrame` does not fire, so no
frame measurement taken here is meaningful.

### 9.2 Pinned surface map (increment 4) — resolved to the current tree

§9 above and architect §9.3 name the six surfaces generically ("the tab host", "PR panel
container", "the workspace container"). This section pins each to the **exact file, component
identity, `component:` string label, and render mode** as the code stands on
`feat/p91-observability` (verified 2026-08-28). senior-dev instruments exactly these; the
`component:` labels are the stable, greppable record keys and must be used verbatim.

| # | Surface | File | Component (identity) | `component:` label | Mode |
|---|---------|------|----------------------|--------------------|------|
| 1 | Workspace container | `src/components/RepoWorkspace.tsx` | `RepoWorkspace` (l.127) | `'RepoWorkspace'` | `each` |
| 1a | — refresh hook | `src/components/repoWorkspace/useCoalescedRefresh.ts` | `useCoalescedRefresh` | `'RepoWorkspace'` (`useTracedEffect` effect name `'coalescedRefresh'`) | n/a (effect) |
| 1b | — subscription hook | `src/components/repoWorkspace/useRepoChangeSubscription.ts` | `useRepoChangeSubscription` | `'RepoWorkspace'` (effect name `'repoChangeSub'`) | n/a (effect) |
| 2 | Diff browser | `src/components/DiffBrowser.tsx` | `DiffBrowser` (l.67) | `'DiffBrowser'` | `each` |
| 3 | Graph canvas | `src/graph/GraphCanvas.tsx` | `GraphCanvas` (l.158, `forwardRef`) + `frameStats` routing | `'GraphCanvas'` | `each` |
| 4 | Right-panel tab host | `src/components/WorkspaceRightPanel.tsx` | `WorkspaceRightPanel` (l.171) | `'WorkspaceRightPanel'` | `each` |
| 5 | PR panel | `src/components/PrPanel.tsx` | `PrPanel` (l.61) | `'PrPanel'` | `each` |
| 6 | Sidebar container | `src/components/Sidebar.tsx` | `Sidebar` (l.120) | `'Sidebar'` | `each` |
| 6a | Branches section | `src/components/sidebar/BranchesSection.tsx` | `BranchesSection` (l.53) | `'BranchesSection'` | `aggregate` |
| 6b | Remotes section | `src/components/sidebar/RemotesSection.tsx` | `RemotesSection` (l.36) | `'RemotesSection'` | `aggregate` |
| 6c | Tags section | `src/components/sidebar/TagsSection.tsx` | `TagsSection` (l.91) | `'TagsSection'` | `aggregate` |

**Row components — one shared tally key per component, NEVER per row instance** (architect §9.3).
Each renders once per ref/stash/worktree/submodule and would flood `each` mode; all use
`aggregate`, keyed by the component label below (identical key across every instance):

| Row component | File | `component:` label | Mode |
|---------------|------|--------------------|------|
| `BranchRow` | `src/components/sidebar/rows.tsx` (l.42) | `'BranchRow'` | `aggregate` |
| `RemoteRow` | `src/components/sidebar/rows.tsx` (l.103) | `'RemoteRow'` | `aggregate` |
| `ConfiguredRemoteRow` | `src/components/sidebar/rows.tsx` (l.149) | `'ConfiguredRemoteRow'` | `aggregate` |
| `StashRow` | `src/components/sidebar/rows.tsx` (l.191) | `'StashRow'` | `aggregate` |
| `WorktreeRow` | `src/components/sidebar/rows.tsx` (l.259) | `'WorktreeRow'` | `aggregate` |
| `DetachedHeadRow` | `src/components/sidebar/rows.tsx` (l.307) | `'DetachedHeadRow'` | `aggregate` |
| `SubmoduleRow` | `src/components/sidebar/SubmoduleRow.tsx` (l.9) | `'SubmoduleRow'` | `aggregate` |
| `TagRow` | `src/components/sidebar/TagsSection.tsx` (l.26, **module-local**) | `'TagRow'` | `aggregate` |

**Explicitly NOT instrumented** (stated so the omission is deliberate, not an oversight):
- `SkeletonRows` (`rows.tsx` l.327) — transient loading placeholder, carries no churn signal.
- `AheadBehindBadge` (`rows.tsx` l.28) — its render is covered by its parent `BranchRow` tally.
- `SectionHeader`, `SectionRollupBadge`, `TagSyncBadge`, `RefFilterMarker` — chrome, not the
  flicker surface; covered by the owning section/container record.
- `useSidebarTreeItem.ts` / `useSidebarTreeNav.ts` — per-item hooks, excluded per architect §9.3
  (their signal is already carried by the row tally).
- `prPanel/PrDetailContainer.tsx` and its children (`PrChangesSection`, `PrFileRow`, …) — outside
  the six surfaces; architect §9.3 instruments "nothing outside these six" in v1.

### 9.3 Drift from architect §9.3, flagged

None of these change the design; they correct paths/attribution so senior-dev instruments the
real tree. None is a MUST-FIX.

1. **Workspace container path.** §9.3 row 1 says `src/components/repoWorkspace/*` *container*. The
   container is `src/components/RepoWorkspace.tsx`; the `repoWorkspace/` subdirectory holds **only
   hooks and tests**, no container. Pinned to the real path above.
2. **Sidebar has six sections, only three are components.** Branches/Remotes/Tags are extracted
   section components (aggregate). **Stashes, Submodules, and Worktrees are rendered inline inside
   `Sidebar.tsx`** (`SectionHeader` + `StashRow`/`SubmoduleRow`/`WorktreeRow` directly, ~l.321-434).
   There is therefore no `StashesSection`/`SubmodulesSection`/`WorktreesSection` to instrument —
   the render signal for those three inline sections is carried by the `Sidebar` `each` record; the
   individual rows carry their own aggregate tallies. §9.3's "three section components" matches
   reality; this note just records where the other three sections live.
3. **`TagRow` lives in `TagsSection.tsx`, not `rows.tsx`.** §9.3 attributes all rows to `rows.tsx`;
   `TagRow` is a module-local component inside `TagsSection.tsx` (l.26). It still gets its own
   shared aggregate tally key (`'TagRow'`) — the file location differs, the rule does not.
4. **`GraphCanvas` is a `forwardRef` component.** Hooks go inside the render function body (after
   the `function GraphCanvas(props, ref)` signature); no structural change, named here so senior-dev
   does not hesitate over the `forwardRef` wrapper.

### 9.4 The ≤8-record sidebar budget — arithmetic and the fixture it constrains

Architect increment-4 acceptance (d) and §11 require **≤8 react records** for one ref change on a
500-ref fixture. The pinned set hits the bound exactly for a branch/remote/tag change:

`Sidebar` (each, 1) + `BranchesSection` + `RemotesSection` + `TagsSection` (3 tallies)
+ `BranchRow` + `RemoteRow` + `ConfiguredRemoteRow` + `TagRow` (4 tallies) = **8**.

This holds **only if the 500-ref fixture contains no stashes, submodules, worktrees, or a detached
HEAD** — otherwise `StashRow`/`SubmoduleRow`/`WorktreeRow`/`DetachedHeadRow` tallies push the count
to 11-12. **Contract-consistency note for the orchestrator/tester (not a MUST-FIX):** the `sidebar-churn`
/ 500-ref budget fixture must be refs-only (branches + remotes + tags, no stash/worktree/submodule/detached
rows) for the ≤8 assertion to be meaningful. If a broader fixture is wanted, raise the asserted bound
to match the enumerated row set rather than relaxing the per-instance rule.

### 9.5 Zero-visual / zero-a11y impact — confirmed, with one implementation caution

Confirmed for all ten instrumented components: `useRenderCount`/`useTracedEffect`/
`useStateTransitionLog` are **inert observers**. Per architect §9.1 they early-return
`if (!obsEnabled())`, allocate nothing when off, and add no wrapper element, `<Profiler>`,
provider, memo boundary, `setState`, or scheduling effect. `frameStats` routing in `GraphCanvas`
is log-plumbing on an existing per-window callback — no new frame is scheduled, no draw changes.
Nothing inspected contradicts the zero-visual constraint already asserted in §9's opening
paragraph; the §9.1(A/B/C) harness parity checks remain the gate.

**One caution — MUST-FIX guard for senior-dev, not a design change.** `useRenderCount` and
`useTracedEffect` are hooks: hook order must stay unconditional (§9.1). Several of these containers
early-return before their main render (e.g. `RepoWorkspace`/`PrPanel`/`DiffBrowser` guard on a
missing repo/selection/data). **Every instrumentation hook must be placed above any conditional
`return`** in the component body. A hook added below an early return would fire inconsistently
across renders and violate the rules-of-hooks — the one place careless insertion could change
behavior. senior-dev: verify hook placement precedes every early return in each of the ten targets.

### 9.6 Verdict — increment 4

**No visual or design changes are required for increment 4.** The instrumentation produces zero
rendered output, DOM, style, layout, focus, or ARIA change (§9 constraint holds). senior-dev may
**proceed** against the pinned §9.2 mapping and labels above, honouring the §9.4 fixture note and
the §9.5 hook-placement guard. No new tokens, states, or `ui-reference.md` edits.

---

## 10. Statistics page & `metrics_reset` — reserved, not designed

No screens, no rows, no buttons, and **no affordance of any kind** is designed or shipped in P91.
Both are resolved-out by the user.

- **When the Statistics page exists**, it belongs as its own rail category `'statistics'`, label
  `Statistics`, placed **between `About` and `Developer`** — durable local aggregates are a
  user-facing read, not a diagnostic, so it must not live inside Developer.
- The only thing reserved now: the rail's `dividerBefore: true` currently sits on `dev`; when
  Statistics lands it moves to `statistics`, so Developer stays visually last-and-separate.
- **`metrics_reset` gets no UI in v1** — there must be no way to destroy history from a surface that
  cannot yet display it. When it does surface, it belongs on the Statistics page behind a danger
  `ConfirmDialog`, never on the Developer page. Note the deliberate asymmetry with §8.5: log files
  are deletable in v1 because the page that owns them *is* shipping and the privacy statement
  promises it; metrics are not, because their page is not.

---

## 11. Decision records & open questions

- **DR-1 — RESOLVED (user, 2026-08-27): `Delete all log files` ships in v1, on the architect's
  roll-then-purge model.** Raised here as a concern that logs persisting after Dev mode is off, with
  no in-app delete, leaves a raw-names session's real branch/tag/file/repo names on disk
  indefinitely. **Approved.** An earlier draft of this contract proposed *excluding* the active
  session file to dodge Windows sharing violations and Unix unlinked-inode writes; **that was
  superseded by the architect's roll-then-purge**, which dissolves both platform hazards (the handle
  is closed before any delete, so behaviour is identical everywhere) *and* erases the file that
  actually holds the raw-names exposure. Fully designed in §8.5. Command is `logs_delete_all()`
  returning `LogsDeleteResult { deletedFiles, deletedBytes, failedFiles, activeFile, rolled }`, already
  specified in `P91-observability.md` §6/§6.1 — no backend addition is requested by this contract.
- **OQ-1 (open; recommend: skip for now).** Should leaving Dev mode on for a long time escalate
  beyond the header pill — e.g. after 24h a §10 App-notice-bar row with a `Turn off` action? The pill
  is always visible while recording, which I judge sufficient, and the notice bar is currently
  defined as a *global fault* channel. **Recommendation: ship the pill only; revisit if a user
  actually leaves it on for a week.** Flagged rather than decided because "don't let someone leave it
  on unknowingly" was an explicit requirement.
- **OQ-3 (open).** The status card polls every 2s while the page is open. If the orchestrator prefers
  zero polling, the fallback is a manual `↻` refresh next to the file name — worse for the "is it
  actually recording?" question this card exists to answer. **Recommendation: keep the 2s poll,
  scoped to the visible page.**

---

## 12. Mock-IPC fixture states (`src/ipc/mock/obs.ts`)

The Settings Developer page must be fully verifiable in the browser harness. Required fixtures,
selectable by a mock scenario key:

| Fixture | `LogSessionInfo` / behaviour | Verifies |
|---|---|---|
| `dev-off-never` | `files: []`, all zero, never enabled | off state; fieldset disabled + lead sentence; all three buttons disabled; `No logs yet.` / `No log files to delete.`; no header pill |
| `dev-off-kept` | off, 2 files, `bytes: 3_248_112` | **the post-off persistence state** — all three buttons enabled, `Idle · last session … kept on this computer` |
| `dev-empty` | on, `files: []`, `records: 0` | just-enabled state; `● Idle` |
| `dev-active` | on, 2 files, `bytes: 3_248_112`, `records: 41_208`, `anomalies: 2`, `redaction:'strict'` | the normal on state, `n flagged`, formatting, header pill |
| `dev-raw` | as above, `redaction:'raw'`, `includeRawNames:true` | sensitive-row on-state, group note, status row 3, raw variant of the export dialog + privacy last line |
| `dev-trace` | on, `level:'trace'` | `Frame timing` forced checked+disabled with its note |
| `dev-dropped` | `dropped: 1_204` | the dropped-records line + tooltip |
| `dev-sink-error` | on, write failure flag set | `▲ Not writing`, danger toast, header pill danger variant |
| `dev-export-fail` | `log_export_session` rejects with each mapped code | all four failure toasts |
| `dev-reveal-fail` | `log_reveal_dir` rejects | reveal failure toast |
| **`dev-delete-off`** | **off**, 5 files, 15.8 MB; `logs_delete_all` → `{deletedFiles:5, deletedBytes:16_567_501, failedFiles:0, activeFile:'', rolled:false}` after 400ms | the Dev-mode-off purge: two-paragraph confirm (no roll sentence), busy state, `Deleted 5 log files. 15.8 MB freed.`, row collapsing to the disabled/empty state, focus staying on the button, status card → `No logs yet.` |
| **`dev-delete-rolled`** | **on**, 5 files incl. the active one, raw names on; → `{deletedFiles:5, deletedBytes:16_567_501, failedFiles:0, activeFile:'bonsai-2026-08-27T15-02-40-b71c.jsonl', rolled:true}` | **the roll-then-purge path** — `{n}` includes the active file in note *and* dialog, the three-paragraph confirm, `Still recording — Bonsai started a new log file.`, status card immediately showing `● Recording · 1 file · 0 bytes · 0 records` + the new `activeFile`, header pill never unmounting, delete button staying enabled |
| **`dev-delete-partial`** | 5 files; → `{deletedFiles:4, failedFiles:1, rolled:true}` | externally-locked-file path: partial danger toast with `Show in folder`, note recomputed from fresh info |
| **`dev-delete-fail`** | `logs_delete_all` rejects with permission / folder-missing codes | both mapped total-failure toasts; button returns to enabled; no stale note |
| `dev-pathological` | 8 files, `bytes: 268_435_456`, `records: 9_999_999`, dir = a 240-char nested path, `activeFile` at full length | truncation with `title`, no wrapping, no row growth, both densities; the delete note's large `{n}`/`{size}` formatting |
| `sidebar-churn` | mock ref-set driver with **awaitable** steps: 5 checkouts, a fetch adding ~40 remote refs, a ref deletion, a section toggle | **§9.1(B)** — the sidebar high-churn no-visual-change comparison |

`log_reveal_dir` in mock is a no-op that resolves, and mock `logs_delete_all` mutates only the mock's
in-memory file list, synthesising a new `activeFile` when `dev.enabled` is true (there is no OS folder
in a browser) — **the actual folder opening, the native save dialog, and real on-disk deletion are
USER CHECKPOINT items**, matching architect §12's checkpoint (a)/(b). Add one: **(e) with Dev mode ON,
`Delete logs…` leaves the logs folder holding exactly one new file, the old files are gone from disk,
and that new file grows as you keep using the app.** Everything else above is AI-gate verifiable.

---

## 13. Catalog entries (`src/components/settings/catalog/dev.ts`)

All rows carry `category: 'dev'` and searchable `keywords`. Groups exactly as rendered:
`Dev mode` / `What is captured` / `What a log file contains` / `Log files`.

| id | group | label | control | reset |
|---|---|---|---|---|
| `dev.enabled` | Dev mode | `Dev mode` | `switch` | `resetField('dev','enabled','Off')` |
| `dev.session-info` | Dev mode | `Logging status` | `group` | — |
| `dev.level` | What is captured | `Detail level` | `segmented` | `resetField('dev','level','Debug')` |
| `dev.capture-ipc` | What is captured | `App requests` | `switch` | `resetField('dev','captureIpc','On')` |
| `dev.capture-react` | What is captured | `Screen updates` | `switch` | `resetField('dev','captureReact','On')` |
| `dev.capture-frames` | What is captured | `Frame timing` | `switch` | `resetField('dev','captureFrames','Off')` |
| `dev.include-raw-names` | What is captured | `Include raw repository names` | `switch` | `resetField('dev','includeRawNames','Off')` |
| `dev.privacy-note` | What a log file contains | `What a log file contains` | `group` | — |
| `dev.logs` | Log files | `Log files` | `button` | — |
| `dev.delete-logs` | Log files | `Delete all log files` | `button` | — |

Keyword set (minimum): `debug logging diagnostics troubleshoot verbose trace log jsonl privacy
redact anonymous export zip folder reveal flicker performance report bug`.

`dev.delete-logs` keywords: **`delete remove clear purge erase wipe clean up logs disk space privacy
free space`** — a user who wants the files gone will search any of *delete*, *remove*, *clear* or
*clean*, and one who is worried about disk will search *space*; all must land on this row. Its
`label` (`Delete all log files`) is unique in the dialog and is the accessible name the search index
matches, per §12.2.

- `dev.session-info` and `dev.privacy-note` are **`control: 'group'`** (`types.ts` AM-2), not
  `readonly`: each is an aggregate block on a `role="group"` element named by its heading via
  `aria-labelledby`, with **no** `[data-setting-control]` and runtime-generated children that are not
  individually catalogued. Neither carries catalog `help` — their content *is* the value. Using
  `readonly` would make the coverage guard look for a single value control that does not exist.
- `dev.logs` is one `button` row carrying two buttons — the row is the search unit (§12.2). The
  architect's contract names `dev.reveal-logs` / `dev.export-session` as row ids; here they are
  **controls within `dev.logs`**, so they are not separate catalog entries. Their accessible names
  (`Show in folder`, `Export session…`) are unique in the dialog.
- `dev.delete-logs` is a **separate** row rather than a third control in `dev.logs`, for the misclick
  and hierarchy reasons in §8.1 — and because it must be independently findable by search: a user
  searching "delete" should get a row whose label says `Delete all log files`, not a row labelled
  `Log files`.

---

## 14. Accessibility summary

- **Tab order** on the page: master switch → (fieldset, if enabled) level radios (arrow keys within,
  one tab stop) → 4 switches → privacy statement (not focusable; browse-mode reachable) →
  `Show in folder` → `Export session…` → `Delete logs…`. The destructive control is **last**, so no
  keyboard user passes through it to reach anything else. Disabled fieldset removes its 5 controls
  from the tab order in one place (§12.3.3); the three action buttons use `aria-disabled` and stay
  reachable.
- Every switch: native `<input type="checkbox" role="switch">` named by the row label,
  `aria-describedby` = the row's help **or** note id (and both ids when both exist, §12.2).
- Segmented: `role="radiogroup"` div named by the row label via `aria-labelledby`; never `tablist`.
- Status card and privacy block: `role="group"` + `aria-labelledby` on their headings; no `aria-live`
  on the card (§6).
- **One** polite live region per page (§5.3), used for: Dev mode on/off, export succeeded/failed,
  logging stopped, and all four delete outcomes (§8.5.6). Never `assertive`.
- Both dialogs are the existing `ConfirmDialog` (`role="dialog" aria-modal="true"`, labelled by
  title, focus-trapped, Esc closes). Default focus is **Cancel** in both. Focus restore after the
  delete dialog is specified per-outcome in §8.5.6.
- Icon-only buttons: none on this page (all three actions are labelled). The header pill has a text
  label plus an `aria-label` that adds the action.
- Hit targets: header pill 32×32 box; settings rows 44px min-height; buttons 28px tall inside 44px
  rows — all clear the §3.1 floor.
- Colour is never the sole carrier: `Recording`/`Idle`/`Not writing` are words; the sensitive row uses
  bar + glyph + sentence; the destructive button uses the word `Delete` + trash glyph + fill;
  `n flagged` is a number, not a hue.
- `prefers-reduced-motion: reduce` removes the record-dot fade and all button-spinner rotation
  (spinners become static glyphs with `aria-busy`).
- Contrast, pairs used here, both themes: `--text-1` on `--bg-0` 13.5/15.4:1; `--text-2` on `--bg-0`
  7.9/4.9:1; `--warning` bar/glyph on `--bg-0` 7.3/4.5:1; `--danger` glyph on `--bg-1` 4.4/4.6:1;
  `--text-1` over a `--warning` 14% tint 9.24–10.30 / 11.68–12.00:1 (measured on `--bg-2` — see the
  §5.2 re-measure note for the header's `--bg-1`). `.btn-danger` is an existing component pair,
  unchanged. All ≥4.5:1 for text, ≥3:1 for graphics.

---

## 15. New tokens

**None.** Every colour, radius and font on this surface is an existing custom property from
`src/styles.css`; `.btn-danger` and the `danger` `ConfirmDialog` variant are existing components.

`ui-reference.md` is already current for this contract: the orchestrator applied the §1
header-toolbar order, the new **§12.11** (Developer settings, the sensitive row, and the logging
indicator), and the *"Sensitive is not destructive"* clause. **Nothing further is staged** — the
roll-then-purge change is a behaviour decision inside §8.5 and introduces no new house pattern.

CSS lives in a new `src/styles/settings-dev.css` imported after `settings-primitives.css` (do not
reorder the existing import list), plus the pill rule in the existing header-toolbar stylesheet.
