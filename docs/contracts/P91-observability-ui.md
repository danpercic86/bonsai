# P91 — Observability UI contract (Dev mode)

**Scope:** increment 4 (React causality instrumentation on **six** surfaces — *no visual change*,
§9 here) and increment 7 (Settings → Developer page + the app-level logging indicator).
**Input contract:** `docs/contracts/P91-observability.md` (§7 redaction, §10 settings surface, §12
increments). All six of the architect's §13 open decisions are **resolved by the user**; the two that
touch this surface are folded in (logs persist after Dev mode is switched off, with no delete button
in v1; `metrics_reset` and the Statistics page get no UI). **Out of scope:** the Statistics page
(§10 here reserves placement only).

The user story this surface exists to serve, in order:

> **enable → reproduce the flicker → turn Dev mode back off → find the file → send it to an AI.**

Every placement decision below is checked against that sequence. In particular the *export path must
keep working after Dev mode is switched off* — that is the step where the user actually needs it
(resolved decision: logs are **not** deleted on disable).

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
deliberate visit to a settings page.

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
  Kept after you turn Dev mode off. Bonsai keeps the 10 most
  recent sessions and deletes older ones automatically.
```

### 2.1 Which rows are gated by the master switch

The split is **by workflow, not by topic**.

| Group | Gated by `dev.enabled`? | Why |
|---|---|---|
| DEV MODE (`dev.enabled` + status card) | n/a | the gate itself |
| WHAT IS CAPTURED (`level`, `capture-ipc`, `capture-react`, `capture-frames`, `include-raw-names`) | **yes — `<fieldset disabled>`** | they configure capture; meaningless with capture off |
| WHAT A LOG FILE CONTAINS | **no** | it is the text that decides whether the user *turns it on*; useless if it only appears afterwards |
| LOG FILES (reveal / export) | **no** | **the workflow's last two steps happen after Dev mode is turned off, and logs deliberately persist.** Gating these breaks the milestone |

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
| `src/components/settings/catalog/dev.ts` | `DEV_ENTRIES: readonly SettingsIndexEntry[]` (§13) | ~120 |
| `src/components/settings/categories/DevPage.tsx` | container: reads `useSettingsValues()/useSettingsActions()`, holds `LogSessionInfo` polling, export/reveal handlers, confirm-dialog state; composes the four sections | ~180 |
| `src/components/settings/SettingsDevModeSection.tsx` | group 1 — master switch + `<DevSessionStatus>` | ~70 |
| `src/components/settings/SettingsDevCaptureSection.tsx` | group 2 — the `<fieldset>` with level + 3 switches + the sensitive raw-names row | ~130 |
| `src/components/settings/SettingsDevPrivacySection.tsx` | group 3 — the frozen §7 statement (pure; props = `redaction`) | ~90 |
| `src/components/settings/SettingsDevLogsSection.tsx` | group 4 — reveal + export buttons, empty/busy/error states | ~120 |
| `src/components/settings/DevSessionStatus.tsx` | the §6 status card (presentational; props = `LogSessionInfo \| null`, `enabled`, `state`) | ~110 |
| `src/components/DevModeIndicator.tsx` | the §5 header chip (presentational + one click handler) | ~60 |

Registration: `SettingsCategoryId` gains `'dev'`; `SETTINGS_CATEGORIES` gains the entry;
`CATEGORY_PAGES` maps `dev → DevPage`; the catalog barrel spreads `DEV_ENTRIES`.
`DevModeIndicator` is rendered by `HeaderToolbar.tsx` (one line, conditional).

**Reuse, not invention.** Everything here is an existing primitive: `SettingsGroup`, `SettingsRow`,
`SettingsSwitchRow`, the §12.3.2 segmented radiogroup, `.btn-secondary`, `ConfirmDialog`, the §10.2
toast recipe, `.settings-row-note`, and the `'group'` control kind (`types.ts` AM-2). The only
genuinely new visuals are the **sensitive-row treatment** (§4.2) and the **header chip** (§5) — both
justified below, both built from existing tokens.

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
  afterwards. `danger` styling is reserved in this app for data loss (§12.7 sign-out, branch delete);
  spending it here devalues it. The **privacy** weight is carried by the `--warning` row treatment,
  the persistent note, and the dialog's explicit consequence copy — not by a red button.
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
- Both themes: identical recipe; `--warning` resolves per theme.
- Densities: unchanged (app chrome is density-invariant, §3).

### 5.3 Announcement

On transition off→on and on→off the indicator's mount point contains a visually-hidden
`aria-live="polite"` region that announces exactly once:

- on: `Dev mode on. Bonsai is writing a diagnostic log to this computer.`
- off: `Dev mode off. Logging stopped. Your log files are kept.`

The live region is **permanently present and empty when idle** (§12.3.4 precedent) so the
announcement is not lost to a late-mounted region. It is *not* re-announced on record activity —
that would be a screen-reader denial-of-service.

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

Polling: `log_session_info()` on page mount, on every settings change, and on a **2s interval while
the Settings Developer page is visible and `dev.enabled` is true**. Never polls when the overlay is
closed or the category is not selected — this must not become background load. Formatting: bytes via
the existing house formatter (`3.1 MB`), counts with thousands separators from `Intl.NumberFormat`.

`aria-live` is **not** applied to the card — it changes every 2s. The group is reachable in
screen-reader browse mode; the one-shot transitions are announced by §5.3 instead.

**After Dev mode is switched off** the card does not disappear: it renders one line —
`Idle · last session: 2 files · 3.1 MB · kept on this computer` — so the persistence of the files is
visible at the moment the user turns recording off. With no logs ever recorded it renders
`No logs yet.` in `--text-2`.

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
> **Log files stay on this computer until Bonsai prunes them.** They are kept when you turn Dev mode
> off, so you can export one afterwards. Bonsai keeps the 10 most recent sessions and removes the
> oldest automatically; you can also delete the files yourself from the logs folder.
>
> Log files are plain text, one record per line. You can open one in any text editor and read it
> before you send it anywhere.

Last line changes with the live mode (a `.settings-group-note`, value-tracking):

- strict → `Right now: names are replaced.`
- raw → `Right now: raw names are included.` (leading `--warning` bar, per §4.2)

**Copy rules applied:** no "redaction", no "telemetry", no "IPC", no "hashing", no "salt". "Replaced
by default" beats "conservatively redacted" for the reader who has to decide whether to send it. The
"cannot be matched up" sentence is the honest, jargon-free rendering of the per-session salt. The
persistence paragraph is required because there is deliberately no delete button in v1 (§11 OQ-2):
if the app will not remove the files, it must at least say so and say where they are.

---

## 8. Actions — reveal & export

### 8.1 Rows

One `.settings-row--stacked` row, id `dev.logs`, label `Log files`, controls side by side
(`display:flex; gap:8px`), plus a call-site `.settings-row-note`.

| Control | Class | Label | Icon (§13 SVG chrome) |
|---|---|---|---|
| reveal | `.btn-secondary` | `Show in folder` | folder-open, 14px, leading |
| export | `.btn-secondary` | `Export session…` | box-arrow-down, 14px, leading |

Both are secondary — **this page has no primary action**; the master switch is the thing the user came
for and a switch is not a button. Ellipsis on `Export session…` per house convention (it opens
further UI).

Note line (value-tracking, so no catalog `help`):

- has logs → `Kept in {dir} after you turn Dev mode off. Bonsai keeps the 10 most recent sessions and deletes older ones automatically.` (`{dir}` truncated with `title`, mono)
- no logs → `No logs yet. Turn on Dev mode, reproduce the problem, then export the session.`

`Show in folder` label rationale: it is what Windows Explorer, Finder and most Linux file managers
call it, and it is honest when the folder exists but is empty. Not "Reveal in Finder" (macOS-only
phrasing), not "Open logs directory" (jargon).

### 8.2 States

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
  `ConfirmDialog`, never on the Developer page.

---

## 11. Open questions / concerns for the user

- **OQ-1 (recommend: skip for now).** Should leaving Dev mode on for a long time escalate beyond the
  header pill — e.g. after 24h a §10 App-notice-bar row with a `Turn off` action? The pill is always
  visible while recording, which I judge sufficient, and the notice bar is currently defined as a
  *global fault* channel. **Recommendation: ship the pill only; revisit if a user actually leaves it
  on for a week.** Flagged rather than decided because "don't let someone leave it on unknowingly"
  was an explicit requirement.
- **OQ-2 — concern, no design produced (the user has resolved this as "no delete button in v1"; I am
  registering the residual risk, not re-opening it).** With logs persisting after Dev mode is
  switched off and no in-app delete, a user who ran one session with **raw names on** has a file
  containing real branch, tag, file and repository names sitting in the app's config directory
  indefinitely — pruned only once 10 newer sessions accumulate, which for an occasional debugger may
  be never. The mitigations specced above are all *informational*: the §7 persistence paragraph, the
  post-off status line, and the note under the buttons naming the folder. **Recommendation for a
  later increment** (not P91): a `Delete all log files` button on this page, danger `ConfirmDialog`,
  copy `Delete {n} log files ({size})? This cannot be undone.` ~15 lines of UI, and it closes the only
  privacy gap this surface still has.
- **OQ-3.** The status card polls every 2s while the page is open. If the orchestrator prefers zero
  polling, the fallback is a manual `↻` refresh next to the file name — worse for the "is it actually
  recording?" question this card exists to answer. **Recommendation: keep the 2s poll, scoped to the
  visible page.**

---

## 12. Mock-IPC fixture states (`src/ipc/mock/obs.ts`)

The Settings Developer page must be fully verifiable in the browser harness. Required fixtures,
selectable by a mock scenario key:

| Fixture | `LogSessionInfo` | Verifies |
|---|---|---|
| `dev-off-never` | `files: []`, all zero, never enabled | off state; fieldset disabled + lead sentence; both buttons disabled; `No logs yet.`; no header pill |
| `dev-off-kept` | off, 2 files, `bytes: 3_248_112` | **the post-off persistence state** — buttons enabled, `Idle · last session … kept on this computer` |
| `dev-empty` | on, `files: []`, `records: 0` | just-enabled state; `● Idle` |
| `dev-active` | on, 2 files, `bytes: 3_248_112`, `records: 41_208`, `anomalies: 2`, `redaction:'strict'` | the normal on state, `n flagged`, formatting, header pill |
| `dev-raw` | as above, `redaction:'raw'`, `includeRawNames:true` | sensitive-row on-state, group note, status row 3, raw variant of the export dialog + privacy last line |
| `dev-trace` | on, `level:'trace'` | `Frame timing` forced checked+disabled with its note |
| `dev-dropped` | `dropped: 1_204` | the dropped-records line + tooltip |
| `dev-sink-error` | on, write failure flag set | `▲ Not writing`, danger toast, header pill danger variant |
| `dev-export-fail` | `log_export_session` rejects with each mapped code | all four failure toasts |
| `dev-reveal-fail` | `log_reveal_dir` rejects | reveal failure toast |
| `dev-pathological` | 8 files, `bytes: 268_435_456`, `records: 9_999_999`, dir = a 240-char nested path, file name at full length | truncation with `title`, no wrapping, no row growth, both densities |
| `sidebar-churn` | mock ref-set driver with **awaitable** steps: 5 checkouts, a fetch adding ~40 remote refs, a ref deletion, a section toggle | **§9.1(B)** — the sidebar high-churn no-visual-change comparison |

`log_reveal_dir` in mock is a no-op that resolves (there is no OS folder in a browser) — **the actual
folder opening and the native save dialog are USER CHECKPOINT items**, matching architect §12's
checkpoint (a)/(b). Everything else above is AI-gate verifiable.

---

## 13. Catalog entries (`src/components/settings/catalog/dev.ts`)

All rows carry `category: 'dev'` and searchable `keywords`. Groups exactly as rendered:
`Dev mode` / `What is captured` / `What a log file contains` / `Log files`.

| id | group | label | control | reset |
|---|---|---|---|---|
| `dev.enabled` | Dev mode | `Dev mode` | `switch` | `resetField('dev','enabled','Off')` |
| `dev.session-info` | Dev mode | `Logging status` | **`group`** | — |
| `dev.level` | What is captured | `Detail level` | `segmented` | `resetField('dev','level','Debug')` |
| `dev.capture-ipc` | What is captured | `App requests` | `switch` | `resetField('dev','captureIpc','On')` |
| `dev.capture-react` | What is captured | `Screen updates` | `switch` | `resetField('dev','captureReact','On')` |
| `dev.capture-frames` | What is captured | `Frame timing` | `switch` | `resetField('dev','captureFrames','Off')` |
| `dev.include-raw-names` | What is captured | `Include raw repository names` | `switch` | `resetField('dev','includeRawNames','Off')` |
| `dev.privacy-note` | What a log file contains | `What a log file contains` | **`group`** | — |
| `dev.logs` | Log files | `Log files` | `button` | — |

Keyword set (minimum): `debug logging diagnostics troubleshoot verbose trace log jsonl privacy
redact anonymous export zip folder reveal flicker performance report bug`.

- `dev.session-info` and `dev.privacy-note` are **`control: 'group'`** (`types.ts` AM-2), not
  `readonly`: each is an aggregate block on a `role="group"` element named by its heading via
  `aria-labelledby`, with **no** `[data-setting-control]` and runtime-generated children that are not
  individually catalogued. Neither carries catalog `help` — their content *is* the value. Using
  `readonly` would make the coverage guard look for a single value control that does not exist.
- `dev.logs` is one `button` row carrying two buttons — the row is the search unit (§12.2). The
  architect's contract names `dev.reveal-logs` / `dev.export-session` as row ids; here they are
  **controls within `dev.logs`**, so they are not separate catalog entries. Their accessible names
  (`Show in folder`, `Export session…`) are unique in the dialog.

---

## 14. Accessibility summary

- **Tab order** on the page: master switch → (fieldset, if enabled) level radios (arrow keys within,
  one tab stop) → 4 switches → privacy statement (not focusable; browse-mode reachable) →
  `Show in folder` → `Export session…`. Disabled fieldset removes its 5 controls from the tab order in
  one place (§12.3.3).
- Every switch: native `<input type="checkbox" role="switch">` named by the row label,
  `aria-describedby` = the row's help **or** note id (and both ids when both exist, §12.2).
- Segmented: `role="radiogroup"` div named by the row label via `aria-labelledby`; never `tablist`.
- Status card and privacy block: `role="group"` + `aria-labelledby` on their headings; no `aria-live`
  on the card (§6).
- One-shot announcements only, via the §5.3 polite live region: Dev mode on/off, export succeeded,
  export failed, logging stopped.
- Icon-only buttons: none on this page (both actions are labelled). The header pill has a text label
  plus an `aria-label` that adds the action.
- Hit targets: header pill 32×32 box; settings rows 44px min-height; buttons 28px tall inside 44px
  rows — all clear the §3.1 floor.
- Colour is never the sole carrier: `Recording`/`Idle`/`Not writing` are words; the sensitive row uses
  bar + glyph + sentence; `n flagged` is a number, not a hue.
- `prefers-reduced-motion: reduce` removes the record-dot fade and any button-spinner rotation
  (spinner becomes a static glyph with `aria-busy`).
- Contrast, pairs used here, both themes: `--text-1` on `--bg-0` 13.5/15.4:1; `--text-2` on `--bg-0`
  7.9/4.9:1; `--warning` bar/glyph on `--bg-0` 7.3/4.5:1; `--danger` glyph on `--bg-1` 4.4/4.6:1;
  `--text-1` over a `--warning` 14% tint 9.24–10.30 / 11.68–12.00:1 (measured on `--bg-2` — see the
  §5.2 re-measure note for the header's `--bg-1`). All ≥4.5:1 for text, ≥3:1 for graphics.

---

## 15. New tokens

**None.** Every colour, radius and font on this surface is an existing custom property from
`src/styles.css`. Two new *patterns* (not tokens) belong in `ui-reference.md`: `§12.11 Developer
settings, the sensitive row, and the logging indicator`, plus the amended header-toolbar order in §1.
The exact patch text is staged at **`docs/contracts/ui-reference-patch-P91.md`** because `Edit` was
unavailable in the authoring session — apply it on the next `ui-designer` invocation, then delete that
file.

CSS lives in a new `src/styles/settings-dev.css` imported after `settings-primitives.css` (do not
reorder the existing import list), plus the pill rule in the existing header-toolbar stylesheet.
