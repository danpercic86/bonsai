# P91 — Dev-mode privacy consent copy (satisfies `P91-observability.md` §7.4.3 AC12)

**Status:** SPEC — ui-designer, 2026-09-03. **§6 RULED and re-specced 2026-09-11** (user ruling, see
`TODO.md` → `USER DECISION LEDGER — 2026-09-11` row 3). Branch `feat/p91-observability`.
**Amends** `docs/contracts/P91-observability-ui.md` §7 (frozen privacy statement) and §4.3 (raw-names
confirmation), and §8.3's export content statement. Where this file and §7/§4.3/§8.3 disagree,
**this file wins**; everything else in those sections (geometry, roles, live-region policy, states) is
unchanged.

**Input contracts:** `P91-observability.md` §7.4 (the ruling) + §13 row 26; `P91-observability.md`
§7.1 (as replaced by A26), §7.2, §7.3, §6.1, §6.2, §8 (metrics).
**Implementer:** `senior-dev`. **Files touched:** `src/components/settings/DevConfirmDialogs.tsx`,
`src/components/settings/SettingsDevPrivacySection.tsx`, and — added by §6's ruling —
`src/components/settings/SettingsDevLogsSection.tsx` and `src/components/settings/catalog/dev.ts`.
Copy only — no layout, no new component, no new token, no `src/styles/**` change.
**§6 additionally depends on a backend change that is NOT mine** (retention 400→90 and putting the
`metrics` directory inside the delete action's scope): see §6.6.

---

## 1. What this change is, and is not

The leak is fixed (`c0abbe1`), so the shipped copy is **true**. This pass makes it **complete**, in
one direction only:

> **Raw mode widens *identifier* fidelity and never *content* fidelity.**
> It adds real names and full IDs. It adds nothing you typed.

Every edit below either (a) replaces the vague word "arguments" with the identifier list, or
(b) closes a gap in the "never" list. No warning is added, no tone is escalated, no hierarchy moves.
The dialog stays 3 paragraphs; the panel keeps its paragraph order and its `<strong>` lead-ins.

---

## 2. Surface A — `RawNamesConfirmDialog` (`DevConfirmDialogs.tsx:16-50`)

Title, labels, variant, focus behaviour: **unchanged** (`Include raw repository names in logs?` /
`Include raw names` / `primary` / default focus Cancel). Replace the three body paragraphs with:

> New log files will contain the real names in your repository: the folder it lives in, your
> branches, tags and files, your remote addresses, and full commit IDs — instead of placeholders like
> `ref#3` and `path#7`. A folder path may include your computer account name.
>
> It never adds anything you typed. Commit messages, search text and other text you write stay out of
> the log in every mode — as do the contents of your files, the name and email address you commit
> under, and any password, access token or key.
>
> Bonsai starts a new log file now, so the file you are recording into does not mix the two settings.
> Read an exported log before sending it to anyone.

Implementation notes:
- `ref#3` and `path#7` keep the existing `<span className="mono">` treatment. No other inline styling.
- Paragraph 2 is the load-bearing sentence pair; keep it as **one** `<p>` so it is read as a unit.
- "It never adds anything you typed" is the ruling in user language and must not be softened to
  "generally"/"normally" in any later revision.

---

## 3. Surface B — `SettingsDevPrivacySection` (`SettingsDevPrivacySection.tsx:21-66`)

Seven paragraphs plus the live `Right now:` note, in the existing order. **Paragraphs 1, 2, 6 and 7
and the note are unchanged**; paragraphs 3, 4 and 5 are replaced. `<strong>` lead-ins and `.mono`
spans keep their current pattern.

> **Superseded in part by §6 (2026-09-11).** The block becomes **eight** paragraphs: §6.3 amends ¶6's
> quoted button label, §6.2 inserts a new ¶7, and today's ¶7 ("plain text, one record per line")
> becomes ¶8 with its text unchanged. ¶1, ¶2 and the note remain byte-identical. Where this section
> and §6 disagree on the paragraph count or on ¶6, **§6 wins**.

**¶3 — the "never" list (replaces `:31-34`)**

> **A log file never contains — in any mode:** your commit messages, your search text or anything
> else you type, the contents of your files or diffs, the name and email address you commit under, or
> any password, access token or key. Bonsai drops these where the file is written, so the rule holds
> in both modes.

**¶4 — placeholders (replaces `:36-41`)**

> **Names are replaced by default.** Your branches, tags, files, remotes and repository appear as
> `ref#3`, `path#7`, `remote#1`, `repo#1`, and commit IDs are shortened. The same name keeps the same
> number inside one file, so a problem can still be followed — and the numbers change next time, so
> two files cannot be matched up.

**¶5 — what raw mode changes (replaces `:43-46`)**

> **If you turn on "Include raw repository names"**, those placeholders stop: new log files contain
> the folder your repository lives in, your real branch, tag and file names, your remote addresses
> (with any username or password removed) and full commit IDs. That is the whole difference — raw
> names add identifiers, never anything you typed, and the "never contains" list above is unchanged.

Implementation notes:
- `remote#1` is a **new** `.mono` span in ¶4; `repo#1`, `ref#3`, `path#7` already exist there.
- The ordinals stay in their short form (`ref#3`, not `ref#3(branch)`; `path#7`, not `path#7.ts`).
  Deliberate: the suffixes are a reader's detail and cost the sentence its scannability. A user who
  opens the file sees the richer form and is not misled by the simpler one.
- ¶4's "commit IDs are shortened" is the plain-language form of §7.1's "first 7 chars kept". Do not
  print the number 7 — it invites the reader to think the count is the point.

---

## 4. Surface C — `ExportConfirmDialog` (`DevConfirmDialogs.tsx:52-98`) — same defect, same fix

Not named in the brief, but this is the dialog shown at the moment the user creates the file they
will mail. It carries the same two content statements with the same two gaps, and leaving it stale
would put two different "never" lists in one flow. In scope for AC12.

Structure, `dev-warning-bar`, glyph and labels are **unchanged**. Replace only the two content
statements.

> **Corrected 2026-09-14 (§6.8 R1).** The trailing two lines were specced as unchanged. The second of
> them **quotes the delete control**, so §6.2's byte-identical-pair rule reaches it: it now reads
> `Exports you saved elsewhere are not removed by “Delete logs and usage counts”.` This dialog is
> therefore **not** "unchanged" — see §6.8 R1.

**raw branch (replaces `:84-87`, keeps the `dev-warning-bar` + `⚠` glyph):**

> Raw names are on: this log contains the folder your repository lives in, your real branch, tag and
> file names, your remote addresses and full commit IDs. Commit messages, search text and anything
> else you typed are not in the file, and neither are file contents, the name and email address you
> commit under, or any password or access token.

**strict branch (replaces `:89-92`):**

> Branch, file, remote and repository names are replaced with placeholders, and commit IDs are
> shortened. Commit messages, search text and anything else you typed are not in the file, and
> neither are file contents, the name and email address you commit under, or any password or access
> token.

---

## 5. The "never" list — canonical wording and provenance

One list, five clauses, same order on every surface. Any future surface reuses this order verbatim.

| # | Clause (user words) | What it covers | Enforced by |
|---|---|---|---|
| 1 | your commit messages | `commit`/`commitMerge` message param | `rawArgPolicy.json` `null` + `isFreeTextParam` + writer W4 |
| 2 | your search text or anything else you type | `searchCommits` query, notes, prompts, descriptions, titles, bodies, reasons | `isFreeTextParam` vocabulary (producer) **and** `raw_args::enforce` W4/W5 (writer, both modes) |
| 3 | the contents of your files or diffs | diff/blame/file bodies — never a logged field | §7.1 row 914; no producer emits them |
| 4 | the name and email address you commit under | git author/committer identity | §7.1 row 915 |
| 5 | any password, access token or key | PATs, `Authorization`, SSH keys | `isSensitiveParam` + `scrub::is_sensitive_key` + W3 (numeric keys dropped) + `scrub_value` |

**Added by this pass:** clause 2's generalisation — the panel already said "your search text", but
nothing on any surface said **other free text**, and the dialogs said neither. Clause 2's "or
anything else you type" is what makes the list match the enforced vocabulary
(`message|msg|query|search|text|body|prompt|descri|note|content|comment|title|subject|summary|patch|diff|blurb|input|reason`)
rather than one example from it.

**Wording changed, not added:** "your name or email address" → "the name and email address you commit
under". The old phrasing collides with the new, honest admission that a raw repository path can
contain the OS account name; the new phrasing names the git identity precisely and removes the
apparent contradiction.

**The one enforcement claim, and its limit.** ¶3's second sentence ("Bonsai drops these where the file
is written") is the only structural promise made anywhere in this copy. It is warranted by
`raw_args::enforce` running unconditionally in `writer.rs::append_record`, in **both** modes, before
the scrubber, dropping the whole `args` object on any denied key, numeric key, or non-scalar value —
i.e. a frontend bug or a bad allow-list row cannot leak. Do not extend this sentence to claim
anything is "impossible", "guaranteed" or "never can be": the writer defends the `args` object and
the credential vocabulary, and that is what the sentence says.

---

## 6. `usage.json` disclosure — **RULED 2026-09-11. Implement all of §6.**

Audit finding F6. **The user rejected options A, B and C as specced** (they are kept, struck, in
§6.9 so nobody re-proposes one) and chose a fourth shape:

| | Ruling |
|---|---|
| Collection | **stays always-on**, independent of Dev mode, from first launch — a future Statistics page stays viable |
| Retention | **400 days → 90** |
| Deletability | **deletable from the app**, by the **existing** delete action — not by a new row |

My 2026-09-03 recommendation (option A — "disclose, no reset button") is **superseded on the
deletability point**. It is no longer a live recommendation and must not be cited as one.

Three consequences follow, and all three are in scope here:

1. The copy must disclose the usage count (§6.2) **and** stop implying the delete button is
   log-only (§6.3).
2. The delete action's own labels and confirmation must name what it now destroys (§6.4). A
   destructive action whose label understates its scope is the same defect as the one being fixed,
   pointed the other way.
3. The group heading "What a log file contains" no longer describes the block (§6.5).

### 6.1 What the panel implies today, by omission

The panel is headed **"What a log file contains"** and its persistence paragraph ends with
`"Delete all log files" below removes every one of them`. A reasonable reader concludes that Dev mode
is the only thing Bonsai records, and that the delete button clears everything Bonsai kept about
them. Both are false:

- `metrics/usage.json` is written **always**, independently of Dev mode, from first launch.
- It is **not** in the scope of `logs_delete_all` (`P91-observability.md` §8, `metrics_keys.rs`
  rationale) and **not** in an export zip.
- It keeps **daily buckets for 400 days**, then folds them into a lifetime total that is never aged
  out, plus a `usage.json.bak` copy.
- `metrics_reset` exists in the IPC surface but has **no UI** (ratified: `P91-observability-ui.md`
  §10 — no destroy affordance on a page that cannot display the data). So there is no in-app way to
  clear it.

Mitigating, and the reason this is a disclosure question and not a privacy one: its keys are
shape-guarded to `<domain>.<action>` / `cmd.<name>` / error codes by `metrics_keys.rs`, so no
user-derived string can become a key. It structurally cannot contain repo content.

#### 6.1.1 What is actually in the file — read from the source, not from §8

The copy below is written against `src-tauri/src/obs/metrics.rs`, checked 2026-09-11. Anyone
revising the copy must re-check this table first; **one row of it is not what the board's F6 note
said**, and getting it wrong would put an overclaim in the app's only privacy surface.

| Field (`metrics.rs`) | Written when | Disclosed as |
|---|---|---|
| `first_seen` (`:51`) | set on the first launch that writes metrics; **re-set to today after a delete** | "the date the count started" — deliberately *not* "the date you first used Bonsai", which a delete makes false |
| `sessions` (`:53`) | **every launch**, at init (`:210`, `:235`) | "how many times you have opened the app" |
| `days[].totals.counters` | **every launch** — `perf.*` and `<domain>.<action>` counts | "a day-by-day tally of what it did" |
| `days[].totals.durations` | **Dev mode only** — the `span`/`ipc.result` records that feed the `op.*`/`cmd.*` histograms exist only then (`:148-152`) | "with Dev mode on it also records how long its own actions took" — a **separate sentence**, because the always-on claim does not cover it |
| `days[].totals.errors` | same source as `durations` | folded into the same Dev-mode sentence; not named separately |
| `lifetime` (`:57`) | everything older than the window, folded and never aged out | "and a running total after that" |

**The trap this table exists to stop.** The board's F6 note described the file as an always-on
per-day profile of "which operations ran **and how long they took**". The durations half is
Dev-mode-only (`metrics.rs:148-152`). A single sentence claiming both would be an **overclaim in a
privacy disclosure** — the worst place in the app to have one. Hence two sentences in ¶7, and hence
this row.

### 6.2 ¶7 — the disclosure paragraph (SIGNED, implement verbatim)

Insert as **¶7**, between the "Log files stay on this computer…" paragraph (today's ¶6) and the
"plain text, one record per line" paragraph (which becomes ¶8). One `<p>`, the same `<strong>`
lead-in pattern as ¶3–¶6, **no** warning bar and **no** glyph — this is a disclosure, not a hazard,
and `ui-reference.md` §12.11's "sensitive is not destructive" line reserves the `--warning` bar for
a control the user can set.

> **Bonsai also keeps a small usage count.** From the first time you open it, and whether or not Dev
> mode is on, it records the date the count started, how many times you have opened the app, and a
> day-by-day tally of what it did — how many repositories it opened, how many commits it made. With
> Dev mode on it also records how long its own actions took. The day-by-day detail is kept for 90
> days, and a running total after that. It holds only Bonsai's own action names — never a repository,
> branch, file, or anything you typed — it stays in a `metrics` folder beside your log files, and it
> never leaves this computer or goes into an export. &ldquo;Delete logs and usage counts&rdquo; below
> clears all of it.

Implementation notes:

- `metrics` is a **new `.mono` span**, matching `ref#3` / `repo#1` in ¶4. It is the only `.mono` span
  in ¶7.
- `&ldquo;Delete logs and usage counts&rdquo;` must be **byte-identical to the row label in §6.4**.
  These two strings are a pair; if a later revision changes one it changes both, or the panel points
  at a control that does not exist.
- **Naming the folder, not the file, is load-bearing and must survive every future revision.**
  Deleting `usage.json` alone leaves `usage.json.bak` to be restored on next load, so the word
  `usage.json` must appear nowhere in user-facing copy. The same fact drives AC 6.6.3.
- "the date the count started" not "the date you first used Bonsai": `first_seen` is re-set after a
  delete (`metrics.rs:207`, `:232`), so the friendlier phrasing becomes false the moment the new
  delete action is used. This is the only place the two readings diverge, and the ruling created the
  divergence.
- "a running total after that" is true **only while `lifetime` survives the window change** — see
  §6.6 flag 3. If the architect decides 90 days means *everything* ages out, strike that clause and
  nothing else.
- No `aria-live`, no re-derived string: §8's stickiness rule applies to ¶7 exactly as to ¶3–¶6.

### 6.3 ¶6 — the one-clause amendment that closes the by-omission defect

Today's ¶6 ends `…and "Delete all log files" below removes every one of them`. Only the quoted label
changes; the sentence's meaning ("every one of them" = every log file) is already correct and ¶7 now
supplies the rest.

> **Log files stay on this computer until you delete them.** They are kept when you turn Dev mode
> off, so you can export one afterwards. Bonsai keeps the 10 most recent sessions and removes the
> oldest automatically, and &ldquo;Delete logs and usage counts&rdquo; below removes every one of
> them — including the session being recorded right now.

That is the whole fix to ¶6. **Do not** add "and your usage counts" here: it would duplicate ¶7 and
make "every one of them" ambiguous about its antecedent, which is the defect in a new costume.

### 6.4 The delete action — labels, enabled state, confirmation

The ruling routes deletion through the **existing** action, so that action's promise has to widen
with its scope. Four copy changes, one behavioural change, no new control, no new row.

| Site | Today | Signed |
|---|---|---|
| `SettingsDevLogsSection.tsx:105` row label | `Delete all log files` | `Delete logs and usage counts` |
| `SettingsDevLogsSection.tsx:118` button | `Delete logs…` | `Delete all…` |
| `DevConfirmDialogs.tsx:127` dialog title | `Delete all log files?` | `Delete logs and usage counts?` |
| `DevConfirmDialogs.tsx:128` confirm label | `Delete logs` | `Delete all` |
| `catalog/dev.ts:110` row (`dev.delete-logs`) | label mirrors the row title | ~~must mirror the **new** row label~~ — **corrected 2026-09-14 (§6.8 R2): the label is the BUTTON text `Delete all…`.** The DOM↔catalog guard asserts `getByRole('button', { name: entry.label })` (`settingsCatalog.coverage.test.tsx:324`), so a catalog label that is not the button's accessible name would fail the guard and would make search match text the user cannot reach. This follows the documented `dev.logs` / `about.welcome-tour` precedent. The row-title vocabulary lives in `keywords` instead |

`confirmVariant="danger"` and `btn-danger` are **unchanged** — the action still loses data
irreversibly, and this is precisely the case the danger hue is reserved for (see the D3 ruling in
`ui-reference.md` §2).

**Row hint (`deleteNote`, `SettingsDevLogsSection.tsx:24-30`).** It currently returns
`'No log files to delete.'` when there are no logs, which after this change is both false and the
reason the row would look dead to the user who most needs it. Three branches, replacing three:

| State | String |
|---|---|
| logs present, Dev mode **on** | `Removes all {N} log file{s} ({size}) and Bonsai's usage counts, including the log being recorded now. Recording continues in a new file.` |
| logs present, Dev mode **off** | `Removes all {N} log file{s} ({size}) and Bonsai's usage counts from this computer.` |
| **no logs** | `No log files yet. Removes Bonsai's usage counts.` |

**Enabled state — the hole the ruling opens, and its fix.** The row is gated on
`hasLogs = count > 0` (`:48`, `:91`, `:111`). A user who has never turned Dev mode on has **zero
log files and up to 90 days of usage counts**, and would find the only control that clears them
permanently disabled — i.e. exactly the user for whom deletability was granted cannot use it.

**Fix: drop the gate. The row is always enabled.** No new IPC field is needed, and this is not a
judgement call — `metrics.rs:210`/`:235` bump `sessions` **at init on every launch**, so by the time
any human can see this row `usage.json` exists and has something in it. There is no reachable
"nothing to delete" state. Consequences:

- `aria-disabled` is dropped from the button, as is the `title={hasLogs ? undefined : 'No logs yet.'}`
  tooltip. `anyBusy` still disables it during an in-flight operation — that gate stays.
- `hasLogs` remains in use for `logsNote` (`:53-66`) and for the "No logs yet." export-row tooltip.
  **Do not delete the variable**; only the delete row stops consulting it.
- `ui-reference.md` §12.11's "disabled-not-hidden when it has nothing to act on" is **satisfied, not
  violated**: the row now never has nothing to act on. Keep `aria-disabled` (not `disabled`) on the
  busy state so focus survives the operation's own completion, per the same paragraph.

**Confirmation body (`DevConfirmDialogs.tsx:134-143`).** `n === 0` is now a reachable state, so the
lead line branches. Everything else is additive; `This cannot be undone.` stays where it is.

> *(`n > 0`)* `Delete {N} log files ({size}) and clear Bonsai's usage counts? This cannot be undone.`
>
> *(`n === 0`)* `Clear Bonsai's usage counts? This cannot be undone.`
>
> *(always, new — names the exact target, per the destructive-action rule)*
> Usage counts are how often Bonsai's own actions ran. They live in a `metrics` folder beside your
> log files, and Bonsai removes that whole folder.
>
> *(existing, when `exportFiles > 0`)* `This includes {archives}.`
>
> *(existing, when Dev mode on)* This includes the session being recorded right now. Bonsai starts a
> new, empty log file and keeps recording.
>
> *(existing)* Exports you saved elsewhere are not removed.
>
> *(amended — "logs" → "these", because the scope widened)* Deleting these does not change any of
> your repositories.

`metrics` is a `.mono` span here too. The "whole folder" sentence is the only place the mechanism is
stated, and it is stated **because** it is the destructive scope — a confirmation that names a
smaller target than it destroys is the defect this milestone is fixing.

There is **no undo**, so the dialog says so plainly and does not offer one. Nothing here is
recoverable from another surface: unlike logs, usage counts are not exported.

`ExportConfirmDialog` (§4) is **unchanged** *in its two content statements*. Usage counts are not in
an export, so neither becomes false; adding "and usage counts are not included" would be noise in the
one dialog whose job is to describe the file being created.

> **Corrected 2026-09-14 (§6.8 R1).** "Unchanged" was too broad. Its **trailing line quotes the
> delete control**, and §6.2's byte-identical-pair rule governs every user-facing quote of that
> control, not just the ones in the privacy panel. The quoted label updates with the row label.

### 6.5 The group heading — `What a log file contains` → `What Bonsai records`

Ruled, and in scope: the heading is **half the defect**. ¶7 is not about a log file, so under the old
heading the honest paragraph reads as if it were filed in the wrong place — and the board's own
statement of the defect names the heading first ("the panel is headed *What a log file contains*
and ends with *removes every one of them*").

`What Bonsai records` is the smallest true heading. It still heads ¶1–¶6 correctly (a log file is
something Bonsai records), and it is the only phrasing tried that does not either lengthen into a
sentence or re-introduce a word the §7 copy rules ban ("telemetry", "captured" — `What is captured`
is already a *different* group's title on the same page, so it is unavailable).

`title` and the catalog `group` string must change **atomically** — settings search matches the group
string, and a mismatch makes the block unreachable by search while still rendering. Sites:

| File | Line | What |
|---|---|---|
| `SettingsDevPrivacySection.tsx` | `14` | `<SettingsGroup … title=…>` |
| `SettingsDevPrivacySection.tsx` | `1` | header comment names the group |
| `catalog/dev.ts` | `96`, `97` | `group` **and** `label` |
| `catalog/dev.ts` | `5` | header comment's group list |
| `P91-observability-ui.md` | `1023`, `1034` | §12 catalog table — **architect's file, flagged in §6.6, not edited here** |

No test asserts the heading literal (checked: `settingsCatalog.coverage.test.tsx`,
`settingsCatalogRows.test.ts` key off `dev.privacy-note`, not the string).
`DevCategory.test.tsx:113` asserts the **dialog** title and does move — it is in §6.4's blast radius,
not this one.

### 6.6 Flags — what §6 needs that is NOT mine to change

Copy cannot make any of these true. All five go to `architect`; the first three are hard blockers for
§6.2's honesty.

1. **`P91-observability.md` §10 must be amended.** It ratifies "ship `metrics_reset`, expose no UI…
   no way to destroy history from a surface that cannot yet display it" (`:952-954`, and row 6 of
   §13's table at `:1936`). The ruling reverses that premise. **I have not edited it** — it is the
   architect's file. Note the reversal is *narrower* than §10's wording suggests: no `metrics_reset`
   **row** is added, so §10's "no destroy affordance of its own" survives; what changes is that the
   existing `logs_delete_all` affordance now covers metrics.
2. **The delete scope must widen, and it must remove the directory.** `P91-observability.md` §6.1
   (`:713-726`), §8 (`:1425` "not covered by `logs_delete_all`") and the `metrics_keys.rs` rationale
   (`:44`, "durable, is not covered by `logs_delete_all`… a user-derived key there is permanent,
   un-deletable") all assert the opposite of the ruling and all three need amending. Two
   implementation facts the copy depends on: it must remove the **`metrics` directory recursively**
   (`usage.json` **and** `usage.json.bak` — deleting the file alone is undone by the next load), and
   the **in-memory `MetricsState` must be reset in the same operation**, or the next flush writes the
   deleted data straight back and the button silently does nothing.
3. **`RETAIN_DAYS` 400 → 90** (`src-tauri/src/obs/metrics.rs:40`), plus §8's prose at `:1404`,
   `:1494-1495`, `:1575`, `:1597-1601`. **Decide explicitly whether `lifetime` survives.** ¶7 says
   "and a running total after that", which is true under today's fold-into-`lifetime` behaviour and
   is what I recommend keeping — `first_seen` and `sessions` are inherently lifetime figures anyway,
   and a Statistics page wants the total. If the architect rules that 90 days means everything ages
   out, **strike that one clause from ¶7 and change nothing else.**
4. **`P91-observability-ui.md` §12's catalog table** (`:1023`, `:1034`) carries the old group heading.
   Architect's file.
5. **Mock parity.** `src/ipc/mock/obs.ts` must clear its metrics fixture on `logsDeleteAll` and honour
   a 90-day window, or the harness cannot show the new copy telling the truth. See §9's added rows.

### 6.7 Why no `Reset usage counts` row, even now

The ruling says the existing delete action clears it, and that is also the better design: one
destructive control on the page instead of two, no second confirm dialog, and no destructive control
on a surface that still cannot display what it destroys. §10's actual principle survives intact. When
the Statistics page ships, a reset belongs **there**, next to the data — not here.

### 6.8 Design review 2026-09-14 — rulings and the copy §6 still owes

Reviewed against the shipped working tree and verified in the harness (`pnpm dev:mock`, port 1420,
both themes). §6's copy landed **verbatim** — ¶6, ¶7, the row label, the hint's three branches, the
dialog title/confirm label and the `metrics`-folder paragraph all match. Five things the
implementation surfaced that the contract had not.

**R1 — `ExportConfirmDialog`'s quoted label: RATIFIED.** §6.2's byte-identical-pair rule is the
higher-order constraint and §4/§6.4's "unchanged" was written before I noticed that dialog quotes the
control. A dialog that points at `"Delete all log files"` would name a control that no longer exists,
which is the §6 defect in miniature. Both sites corrected above; AC7 rewritten to five sites.

**R2 — `catalog/dev.ts` label = the BUTTON text: RATIFIED.** The guard
(`settingsCatalog.coverage.test.tsx:324`) asserts the catalog label IS the control's accessible name,
so search can only match text the user can reach. `dev.logs` and `about.welcome-tour` are the
precedent. The row-title vocabulary is preserved in `keywords` (`delete … logs usage counts metrics`),
so AC11's two searches still pass. **NIT:** `catalog/dev.ts:16`'s header comment claims
`delete all log files` is among the lost vocabulary living in keywords — it is not there, and per
AC7 it must not be. Strike that phrase from the comment.

**R3 — the dead click is fixed, but its replacement understates scope. MUST-FIX.** `deleteOpen` as
its own flag is right and the defect the implementer found was real. But `openDelete` routes a
**failed read** into the `n === 0` branch (`DevCategory.tsx:134`, `fresh ?? info`, both null when the
IPC read fails and no poll ever succeeded). The dialog then says *"Clear Bonsai's usage counts?"* and
proceeds to delete however many log files exist. That is verbatim the defect §6.4 exists to prevent:
**a confirmation that names a smaller target than it destroys.** `info === null` (unknown) and
`info.totalFiles === 0` (known zero) are different states and must read differently.

**Reachability, stated honestly:** this needs *every* `logSessionInfo` read on the page to fail,
including the one on mount, so it is rare. It is ranked MUST-FIX anyway because the rule it breaks is
categorical — a destructive confirm may never understate its scope — not because it is frequent.

**The fix is in the dialog, not in `openDelete`.** `fresh ?? info` is correct as written: falling back
to the last good poll is better than discarding it. What must change is `DeleteLogsConfirmDialog`
collapsing `info === null` to `0` via `info?.totalFiles ?? 0`.

Three lead lines, replacing two. `DeleteLogsConfirmDialog` takes the count as
`number | null` (null = not known) rather than collapsing it to `0`:

| State | Lead line |
|---|---|
| `n > 0` | `Delete {N} log file{s} ({size}) and clear Bonsai's usage counts? This cannot be undone.` |
| `n === 0` | `Clear Bonsai's usage counts? This cannot be undone.` |
| **`n === null`** | `Delete all log files and clear Bonsai's usage counts? Bonsai could not count the log files first. This cannot be undone.` *(revised by §6.10 R11 — as first shipped this said "count **them** first", whose "them" binds to "usage counts". One signed version of this line, and it is this one.)* |

The null line states the scope at its **widest**, which is the only safe direction for a destructive
confirm, and the dialog stays open and actionable rather than dying silently.

**R4 — `Delete 1 log files`. SHOULD-FIX, and the bad string is mine.** §6.4's signed `n > 0` lead line
hard-codes `log files`. `n === 1` is now a common state (it is what the harness shows the moment Dev
mode is switched on), and the row hint 8px above it pluralises the **same count** correctly via
`deleteNote`'s `log file${count === 1 ? '' : 's'}`. Use that same form in the dialog — see the `{s}`
in R3's table. Observed in the harness as
`Delete 1 log files (0 B) and clear Bonsai's usage counts? This cannot be undone.`

**R5 — the outcome copy, now that the scope is wider.** Three strings, none of which §6.4 specced.

- **Ratified as written:** ` Usage counts cleared.` / ` Usage counts were not cleared.`
  (`devLogMessages.ts:27-29`). Driving the clause off `metricsCleared` and never off `deletedMetrics`
  is correct and the reasoning in that comment should stay.
- **SHOULD-FIX — `logParts === 0` has no branch.** With zero log files the real backend yields
  `Deleted 0 log files. … Usage counts cleared.` — "0 log files" reads as a bug in the exact state
  the ruling exists for. Branch it on `logParts === 0`:

  | `exports` | Toast | Announcement |
  |---|---|---|
  | `0` | `Usage counts cleared. {freed} freed.` | `Usage counts cleared.` |
  | `> 0` | `Deleted {N} export{s}. {freed} freed. Usage counts cleared.` | `Usage counts cleared.` |

  The second row matters: zero logs **with** saved export zips is reachable (exports outlive the logs
  they came from), and a branch that assumed no exports would drop them from the outcome copy.
- **SHOULD-FIX — total-failure copy names only logs.** `deleteErrorText`'s fallback
  (`"Couldn't delete the log files."`) and `DevCategory.tsx:150`'s announce
  (`'No log files were deleted.'`) both describe a scope narrower than the action's, and tell the
  zero-log user nothing about the thing they asked for. Widen to `Couldn't delete the logs and usage
  counts.` and `Nothing was deleted.` respectively. The permission and folder-missing branches stay
  as they are — both are true statements about a real cause.
- **NIT — the partial-failure toast offers no next step for the counts.** It ends `Usage counts were
  not cleared.` with no remedy, against the house rule that an error says what to do next. Append
  `Try again.` to that clause in the `failedFiles > 0` path only.

**R6 — the destructive button's accessible name. SHOULD-FIX.** `SettingsRow` renders `hint` as a bare
node (`SettingsRow.tsx:158`) and wires **no** `aria-describedby`; the row title is a `<span>`, not a
label. Verified in the harness: the button's accessible name is `Delete all…` and `aria-describedby`
is `null`. A screen-reader user navigating by button hears *"Delete all…, button"* for a
data-destroying control — "all" of **what** is carried only by sighted proximity. The label shortened
from `Delete logs…` to `Delete all…` in this very change, so this got worse here, not merely stayed
bad. Fix at the call site, not in `SettingsRow`: give the hint `<p>` an id and point the button's
`aria-describedby` at it. Shape only — `deleteText` is the existing local in
`SettingsDevLogsSection`, and the id must be interpolated per row if this pattern is ever reused:

```
<p className="settings-row-note" id="dev-delete-logs-hint">{deleteText}</p>
<button … aria-describedby="dev-delete-logs-hint">Delete all…</button>
```

This is additive, changes nothing visually, needs no token, and keeps the catalog guard green (the
guard reads the accessible **name**, which `aria-describedby` does not touch).

**R7 — harness fidelity gap (fixture, §6.6 flag 5). MUST-FIX for the mock.**
`mock/handlers/obs.ts:205` returns `deletedFiles: 5, deletedExports: 1` unconditionally. In the
zero-log state the harness therefore shows **`Deleted 3 log files and 1 export. 1.2 MiB freed.`** to a
user who had none — the harness actively contradicts the copy it exists to verify, and it hides R5's
`logParts === 0` branch from view. Derive the result from the fixture's own log state: with
`totalFiles === 0`, return `deletedFiles: 1` (the metrics file), `deletedExports: 0`,
`deletedBytes` small. Same for the `?obsDeleteFail=1` path, which today claims `3 of 4`.

### 6.9 Superseded — the three options as originally specced (kept so none is re-proposed)

- ~~**Option A — disclose, no button.** My 2026-09-03 recommendation. Its copy ended "To clear it,
  delete the `metrics` folder next to your log files" — hand-deletion as the only remedy. Superseded:
  the ruling makes it deletable in-app, so that sentence would now be worse advice than the button.~~
- ~~**Option B — disclose and add a `Reset usage counts` row** calling `metrics_reset` behind a
  danger `ConfirmDialog`. Superseded: the ruling routes deletion through the existing action instead.
  See §6.7.~~
- ~~**Option C — say nothing.** Superseded: the user ruled for disclosure.~~

**§2–§5 were implemented pending this ruling; the ruling is in, so §6 implements in full.** The
"implement §2–§5 only" instruction that stood here is withdrawn.

### 6.10 Round 2 — R8–R11, after harness verification of the shipped §6.8 fixes

Copy only; no geometry, token, colour or motion change, so §7 is untouched and both themes and both
densities are unaffected. R8 and R10 are the substantive ones: each is a string that describes a
**smaller or different scope than the command acts on**, which is the same defect class as R3.

**R8 — R3 fixed one of the unknown state's three strings. MUST-FIX.** I specced the third *lead
line* and stopped there. Two strings adjacent to it still collapse `info === null` into "known
zero", and R6 made the first of them worse by wiring it to the destructive button.

- **8a — the danger button's accessible description.** `SettingsDevLogsSection.tsx` derives
  `hasLogs` from `info?.totalFiles ?? 0`, so a failed read yields *"No log files yet. Removes
  Bonsai's usage counts."* — and R6's `aria-describedby` now reads that sentence out as the
  description of a control that is about to delete however many log files exist. The screen-reader
  user gets the understating confirmation that R3 removed from the dialog, relocated verbatim into
  the description. Change `deleteNote`'s first parameter from `hasLogs: boolean` to
  `count: number | null` (null = not known) and derive it at the call site as
  `info === null ? null : info.totalFiles`. `hasLogs` **stays** as it is for the reveal/export
  buttons above — collapsing null to "no logs" there only disables two harmless actions, which is
  the safe direction.

  | `count` | Dev mode | Hint (= the button's accessible description) |
  |---|---|---|
  | **`null`** | off | `Bonsai could not count the log files. Removes all of them and Bonsai's usage counts from this computer.` |
  | **`null`** | on | `Bonsai could not count the log files. Removes all of them and Bonsai's usage counts, including the log being recorded now. Recording continues in a new file.` |
  | `0` | either | unchanged — `No log files yet. Removes Bonsai's usage counts.` |
  | `> 0` | either | unchanged |

  The caveat leads rather than trails: it is the exceptional fact, and a screen-reader user hearing
  this as a description needs the scope uncertainty before the detail. "all of them" binds to "the
  log files" in the sentence before it.

- **8b — the dialog's archives line disappears when the count is unknown.**
  `DevConfirmDialogs.tsx:172` is a two-way `exportFiles > 0 && …` over `info?.exportFiles ?? 0`, so
  on a failed read the archives line vanishes while line 179's *"Exports you saved elsewhere are not
  removed."* still renders — the reassurance survives and the thing it qualifies does not, which
  reads as "no archives are involved". Make it three-way, parallel to `archives()`'s vocabulary:

  | State | Line |
  |---|---|
  | `info === null` | `This includes any exported log archives in Bonsai's exports folder.` |
  | `exportFiles > 0` | unchanged — `This includes {N} exported log archive{s}.` |
  | `exportFiles === 0` | omitted, as now |

  "in Bonsai's exports folder" is load-bearing: it is what the *"saved elsewhere"* line one
  paragraph below contrasts against.

**R9 — `Deleted 1 log files` is now the DEFAULT success toast. MUST-FIX, and the bad string is mine
twice over.** R4 pluralised the dialog and I reviewed `devLogMessages.ts` under R5 without noticing
the same hard-coded `log files` at lines 68–69. Since R7 made the mock report the fixture's single
file, this is the ordinary Dev-ON success path, not an edge case — observed below. Pluralise off
`logParts`, exactly as R4 did off `n`, in **both** the toast and the announcement:

```
text:     `Deleted ${NUM.format(logParts)} log file${logParts === 1 ? '' : 's'}${exportsClause}. ${freed} freed.${usage}`
announce: `Deleted ${NUM.format(logParts)} log file${logParts === 1 ? '' : 's'}. ${freed} freed.${usage}`
```

No test cements the bug: `obsDeleteCounts.test.ts:80` matches the prefix `/^Deleted 1 log file/`.

**R10 — the partial-failure toast calls the usage-counts file a log file. MUST-FIX on the copy;
costs three test updates.** Not previously specced anywhere in this contract — the comment calling
it "externally-locked" is the implementer's, not a ruling of mine, so it is mine to correct.

The success branch carefully decomposes `deletedFiles` (`logParts = deletedFiles - deletedExports -
deletedMetrics`) precisely because `deletedFiles` is a **category total**. The `failedFiles > 0`
branch skips that step and feeds the raw totals into the noun *"log files"*, so the failed
`metrics/usage.json` is reported to the user as a log file that would not delete. With Dev mode off
— zero log files on disk — the harness shows *"Deleted 0 of 1 log files."*, which invents a log file
and misattributes the failure. The `X of Y` framing cannot be salvaged: `LogsDeleteResult` carries
no per-category failure attribution, so `Y` is unknowable per category. Report the successes the
same way the success branch does, and give the failed count the only noun that is certainly true —
`file`:

Discriminate on the **same `logParts`/`exports` pair the success branch uses** — never on
`deletedFiles`, which is what caused this. Three rows, exhaustive:

| State | Toast |
|---|---|
| `logParts === 0 && exports === 0` | `{F} file{s} could not be deleted — {it/they} may be open in another program.{usage}` |
| `logParts > 0` | `Deleted {logParts} log file{s}{exportsClause}. {freed} freed. {F} file{s} could not be deleted — {it/they} may be open in another program.{usage}` |
| `logParts === 0 && exports > 0` | `Deleted {N} export{s}. {freed} freed. {F} file{s} could not be deleted — {it/they} may be open in another program.{usage}` |

Row 1 keyed on the pair, not on `deletedFiles === 0`, deliberately: **metrics deleted fine while a
log file failed** (Dev ON, the active log held open by an editor — plausibly the commonest partial
failure on Windows) gives `deletedFiles > 0` with `logParts === 0` and `exports === 0`, and keying
row 1 the other way would leave that state with no string at all. Row 1's text is already correct
there — `{usage}` resolves to ` Usage counts cleared.` and the sentence reports the real failure.

`{usage}` is unchanged: ` Usage counts cleared.` / ` Usage counts were not cleared. Try again.`
(the R5 NIT clause, correct as shipped). The announcement is each row minus the ` — {it/they} may be
open in another program` clause, matching the existing pattern. Singular agreement fixes the shipped
*"1 could not be deleted — **they** may be open"* at the same time.

Dev OFF then becomes: *"1 file could not be deleted — it may be open in another program. Usage
counts were not cleared. Try again."* — true, and it no longer claims a log file existed.

**Test impact — hand these to `tester`, not `senior-dev` alone.** Three assertions cement the old
noun. Expected replacements below are **derived from each fixture**, not guessed
(`logParts = deletedFiles - deletedExports - deletedMetrics`):

| Assertion | Fixture | `logParts` | Replace with |
|---|---|---|---|
| `obsDeleteCounts.test.ts:97` | `deletedFiles 1`, exports `0`, metrics `0` | 1 | `Deleted 1 log file.` + `1 file could not be deleted` |
| `dev.test.tsx:209` | `base` = `deletedFiles 4`, exports `0` (absent), metrics `0` | 4 | `Deleted 4 log files.` + `1 file could not be deleted` |
| `DevCategory.test.tsx:131` | `deletedFiles 4`, exports `0`, metrics `0` | 4 | `Deleted 4 log files.` + `1 file could not be deleted` |

`obsDeleteCounts.test.ts:98`'s `not.toContain('of 4')` guard goes vacuous — replace it with a
Dev-**OFF** fixture asserting `not.toContain('log file')`, which guards the actual R10 defect (the
zero-log user must never be told a log file failed). **R11 additionally moves
`DevCategory.test.tsx:245`,** which asserts the null lead line verbatim; that is the only assertion
on it (`DevConfirmDialogs.tsx:157` is the only other occurrence in `src/`).

**R11 — NIT, my R3 line's dangling "them".** *"Delete all log files and clear Bonsai's usage counts?
Bonsai could not count them first."* — "them" binds to the nearest plural, *usage counts*, not to the
log files it means. Apply only because `DevConfirmDialogs.tsx` is open for 8b anyway:
`…Bonsai could not count the log files first. This cannot be undone.` "first" stays here (a dialog
does have a "before asking you"); it is deliberately absent from R8a's standing row hint, which has
no such moment.

#### Harness observations (`VITE_MOCK_IPC=1`, 2026-09-14, post-R7 mock) — verbatim

| State | Dialog lead line | Toast | Announcement |
|---|---|---|---|
| Dev ON | `Delete 1 log file (1.1 KiB) and clear Bonsai's usage counts? This cannot be undone.` | `Deleted 1 log files. 5.1 KiB freed. Usage counts cleared. Still recording — Bonsai started a new log file.` | `Deleted 1 log files. 5.1 KiB freed. Usage counts cleared. Still recording in a new log file.` |
| Dev OFF | `Clear Bonsai's usage counts? This cannot be undone.` | `Usage counts cleared. 4.0 KiB freed.` | `Usage counts cleared.` |
| `?obsDeleteFail=1`, Dev OFF | `Clear Bonsai's usage counts? This cannot be undone.` | `Deleted 0 of 1 log files. 1 could not be deleted — they may be open in another program. Usage counts were not cleared. Try again.` | `Deleted 0 of 1 log files. 1 could not be deleted. Usage counts were not cleared. Try again.` |

R3, R4, R5's `logParts === 0` branch, R5's `Try again.` NIT, R6's `aria-describedby` and R7's
fixture all verify correct. R9 and R10 are the two defects above, both visible on the default paths.
Read from the DOM (`innerText` of the dialog and of the `aria-live` regions) rather than from a
screenshot — for copy, the text node *is* the evidence and pixels are strictly weaker. Toast motion
and dismissal timing were not judged: the harness is headless, so they stay a USER CHECKPOINT.

### 6.11 Round 3 — R12–R14: announcement parity, and per-category failure copy (conditional)

Copy only. No geometry, token, colour or motion change: §7 is untouched, no new CSS custom property is
introduced, and both themes and both `panelDensity` values are unaffected. Every string below lives in
`devLogMessages.ts`'s `deleteResultToast`; nothing moves between files.

**Read the conditionality before implementing.**

| Subsection | Status |
|---|---|
| §6.11.1 **R12** — announcement parity | **Unconditional.** Implements against the fields that ship today |
| §6.11.2 **R13a/b/c** — three adjacent result-copy fixes | **Unconditional.** Same |
| §6.11.3 — the signed string table for today's fields | **Unconditional.** This is what the app must say now — **amended 2026-09-14 (A-T2):** T2 carries `{rolled?}`, plus one missing resolved row |
| §6.11.4 **R14** — per-category failure copy | **CONDITIONAL on `failedLogs`/`failedExports` shipping in `LogsDeleteResult`.** Until they do, §6.10 R10's generic copy is the live spec and §6.11.4 describes nothing that exists |
| §6.11.5 — harness states | Split: the R12/R13 states are needed now, the R14 states only if R14 ships |
| §6.11.6 — delivery surface | **RESOLVED 2026-09-14 — superseded by `docs/contracts/P113-settings-inline-notes.md`.** The orchestrator call was made (user ruling #24): all ten Settings-surface toasts become inline notes, not just this one. §6.11.6 stays readable, with its two errata marked in place |

§6.11.4 is written ahead of its fields **so the Rust/TS/IPC/mock change and the copy land in one
increment**, not two. It is not a description of shipped behaviour and must not be read as one.

**Constraint boundary — state it once, do not erode it.** §6.11 governs the **returned** path only:
`logsDeleteAll` resolved, the metrics clear ran, and individual files may have failed. The **thrown**
path is different in kind — `obs_delete.rs:85-103`'s second `?` returns *before* the metrics clear, and
the writer thread's only in-thread `Err` is the `roll(true)` that runs before `purge_scope`, so on every
reachable rejection **zero files were removed and the counts were never touched**. `deleteErrorText`'s
`Couldn't delete the logs and usage counts.` plus `DevCategory.tsx:157`'s `Nothing was deleted.` are
therefore literally true and a retry is idempotent. **No one may "improve" the thrown path by appending
a usage clause to it** — there is nothing to report, and a clause there would claim a clear that
deliberately did not run.

#### 6.11.1 R12 — one string for the delete outcome. MUST-FIX, unconditional

The success announcement drops the exports clause (`devLogMessages.ts:100`); the failure announcement
keeps it (`:77`); the `logParts === 0` announcements drop the freed clause as well (§6.8 R5's
Announcement column). One deletion, three shapes.

**Ruling: the success announcement GAINS the clause — and the general form, which is stronger than the
one-word fix: `announce` is byte-identical to `text` for this builder.**

```
return { tone, text, announce: text };
```

Four reasons, in order of weight:

1. **It is the §6 defect class pointed at the outcome instead of the confirmation.** R3, R8 and R10 each
   removed a string that named a smaller scope than the command acted on. An announcement that omits a
   count of files *actually removed* tells the one user who cannot read the toast that less was
   destroyed than was. Direction of error matters: understating a destruction is the unsafe direction.
2. **The house criterion for trimming a live-region string is "not actionable by voice", not
   brevity.** That is the stated reason the export toast drops the folder location
   (`DevCategory.tsx:113-116`) — and it is the *only* sanctioned trim in this file. Nothing in the
   delete outcome meets it: the counts are the scope, `— it may be open in another program` is the most
   actionable sentence in the whole message, and `Try again.` is the remedy.
3. **Structural enforcement beats a test.** `announce: text` cannot drift. Three rounds (R5, R9, R10)
   each fixed one of these strings and left its twin; one string ends the class permanently.
4. **§6.11.6.** On this surface the toast is **unreachable**, not merely dimmed and partly clipped
   (corrected 2026-09-14 — see §6.11.6's measurement note). The live region was the only channel that
   delivered the message intact, so it must carry all of it. Still true after P113 moves the message
   inline: the note and the announcement are written from one call, from one string.

`DevToast.announce` **stays on the interface** — the export path's divergence is legitimate under
reason 2, and the field is shared. Only `deleteResultToast` collapses. The `rolled` announcement's
separate wording (`Still recording in a new log file.`) goes away with it: the toast's
`Still recording — Bonsai started a new log file.` wins, and it reads correctly aloud (the em dash is a
pause).

#### 6.11.2 R13 — three adjacent fixes, surfaced by enumerating every branch

Each is a result message that misstates its own effect — the same family as R12, not new scope. Ranked,
and each is independently deferrable without touching the others or R12.

**R13a — a failed clear ships a success-toned toast. MUST-FIX.** Tone is `error` iff something the row
promised did not happen: `failedFiles > 0` **or** `metricsCleared === false`. Today tone keys on
`failedFiles` alone, and `metrics_purge.rs:52-57` makes `metricsCleared === false` with
`failedFiles === 0` reachable — an unreadable-but-present `metrics/` fails `read_dir`, counts nothing,
and reports `dir_removed: false`. That state currently renders a **green** toast reading *"Usage counts
were not cleared."* with no remedy. Consequently ` Try again.` attaches to the not-cleared clause in
**every** branch, not only the `failedFiles > 0` one — see the amendment to §6.8 R5's NIT in §6.11.7.
The clause pair is otherwise unchanged: ` Usage counts cleared.` / ` Usage counts were not cleared. Try
again.`, still driven by `metricsCleared` and never by `deletedMetrics`.

**R13b — `0 B freed.` SHOULD-FIX.** `formatBytes(0)` is `0 B` (`utils/format.ts:6`), and
`deletedBytes === 0` is reachable on a *success*: a launch younger than the first 60 s flush clears a
live aggregate with nothing on disk, so `metricsCleared === true` with zero bytes — the §F6 state this
row exists to serve. It currently says `Usage counts cleared. 0 B freed.`, advertising a benefit that
did not occur. **The freed clause is omitted when `deletedBytes === 0`, and it never accompanies a bare
failure sentence** (a byte count beside "could not be deleted" is noise, and the shipped failure rows
already omit it). Encoded in §6.11.3's templates so there is nothing to infer.

**R13c — a partial failure never says recording continues. NIT.** The `failedFiles > 0` branch
`return`s at `devLogMessages.ts:72-78`, before `:102`'s `rolled` suffix, so Dev ON + a locked log file
reports failures and stays silent about the new file. The roll genuinely happened (`obs_delete.rs:114`
sets `rolled: true` before the purge), and this is the user most likely to conclude logging has
stopped. Append the same suffix on both branches. Deferrable: its absence is a missing reassurance, not
a false statement — the confirm dialog already promised it.

#### 6.11.3 The signed strings — today's fields. Verbatim; announcement identical to the toast

Three templates, exhaustive. `{L}` = `logParts`, `{E}` = `exports`, `{F}` = `failedFiles`, `{freed}` =
`formatBytes(deletedBytes)`, counts formatted through `Intl.NumberFormat` as now.

| # | When | `text` — and `announce`, identical |
|---|---|---|
| T1 | something was deleted (`L > 0 \|\| E > 0`) | `{deleted} {freed?}{failure?}{usage}{rolled?}` |
| T2 | nothing deleted, nothing failed | `{usage}{ freed?}{rolled?}` |
| T3 | nothing deleted, something failed | `{failure-lead}{usage}{rolled?}` |

Every optional clause carries its own single leading space, so the templates above are read as
concatenation, not as literal spacing. **Where the two disagree, the resolved rows below are
authoritative: exactly one space between sentences, none trailing.**

Clauses:

- `{deleted}` — `Deleted {L} log file{s} and {E} export{s}.` / `Deleted {L} log file{s}.` /
  `Deleted {E} export{s}.` Pluralised off each count independently (R4/R9).
- `{freed?}` — ` {freed} freed.`; **omitted when `deletedBytes === 0`** (R13b). In T2 it trails the
  usage clause and is omitted whenever the usage clause is the not-cleared form.
- `{failure?}` — ` {F} file{s} could not be deleted — {it/they} may be open in another program.`
  (`it` iff `F === 1`). `{failure-lead}` is the same sentence without the leading space.
- `{usage}` — ` Usage counts cleared.` / ` Usage counts were not cleared. Try again.` (R13a).
- `{rolled?}` — ` Still recording — Bonsai started a new log file.` when `rolled` (R13c).
  **AMENDED 2026-09-14 (A-T2): all THREE templates, not two.** T2 as first signed omitted the clause,
  which was wrong twice over: the shipped code already appended it after the `logParts === 0` branch
  (`devLogMessages.ts:116`, `${usage.trimStart()}${freed}${rolled}`), and omitting it would have
  regressed R13c's own complaint — the roll is true whenever `rolled` is set, and a Dev-ON user whose
  delete removed nothing but the usage counts is precisely the user most likely to conclude logging
  has stopped. The implementer appended it to all three and flagged it as their one interpretive
  call; the removed-hunk receipt confirms it. **The code is right; this table was wrong.**

Resolved rows, verbatim, for every state the harness and the tests reach:

| State | `text` = `announce` | tone |
|---|---|---|
| Dev ON, 1 log file, counts cleared | `Deleted 1 log file. 5.1 KiB freed. Usage counts cleared. Still recording — Bonsai started a new log file.` | success |
| Dev ON, 4 log files + 2 exports | `Deleted 4 log files and 2 exports. 3.4 MiB freed. Usage counts cleared. Still recording — Bonsai started a new log file.` | success |
| Dev OFF, no logs, counts cleared, bytes on disk | `Usage counts cleared. 4.0 KiB freed.` | success |
| **Dev ON**, no logs, counts cleared, bytes on disk — `?obsLogFiles=0` (**added 2026-09-14, A-T2**) | `Usage counts cleared. 4.0 KiB freed. Still recording — Bonsai started a new log file.` | success |
| Dev OFF, no logs, counts cleared, **nothing on disk yet** | `Usage counts cleared.` | success |
| no logs, 2 exports removed | `Deleted 2 exports. 1.2 MiB freed. Usage counts cleared.` | success |
| unreadable `metrics/`, nothing else to do | `Usage counts were not cleared. Try again.` | **error** (R13a) |
| Dev ON, 4 log files deleted, 1 file failed, counts not cleared | `Deleted 4 log files. 3.4 MiB freed. 1 file could not be deleted — it may be open in another program. Usage counts were not cleared. Try again. Still recording — Bonsai started a new log file.` | error |
| Dev OFF, nothing deleted, 1 file failed, counts not cleared | `1 file could not be deleted — it may be open in another program. Usage counts were not cleared. Try again.` | error |
| metrics deleted, active log held open (Dev ON) | `1 file could not be deleted — it may be open in another program. Usage counts cleared. Still recording — Bonsai started a new log file.` | error |

The last two are R10's rows 1 and 3 with R13 applied; the generic noun `file` stays until R14's fields
exist. **Nothing here contains a path or a file name** — `activeFile` is never interpolated into copy.

**A-T2, second half:** the Dev-ON T2 row above was **missing** when this table was signed, so its
claim to cover "every state the harness and the tests reach" was false by exactly one row — the one
`?obsLogFiles=0` with Dev mode ON produces, asserted at
`src/ipc/mock/obsDeleteCounts.test.ts:203-205`. It is the row the omitted `{rolled?}` would have got
wrong, which is why the two halves of this amendment are one finding: a template gap and a coverage
gap at the same state. With the row added the claim holds.

#### 6.11.4 R14 — per-category failure copy. CONDITIONAL on `failedLogs`/`failedExports`

Assumes `LogsDeleteResult` gains per-category failure attribution. **If those fields do not ship, this
subsection does not apply and §6.10 R10 remains the live spec.**

**Only two of the three proposed fields are needed by the copy, and `failedMetrics` is not one of
them** — flag this to the architect. The metrics half is already reported, completely, by the usage
clause: `metricsCleared` is its sole and correct driver, and a count could never drive it because
`metrics_purge.rs:52-57` reaches `dir_removed: false` with `failed_files: 0`. Worse, a count *would*
misreport: `merge_metrics_counts` (`obs_delete.rs:162-166`) adds a **sentinel `1`** when the clear
returns `Err`, and `metrics_purge.rs:63-65` counts a *subdirectory* as a failure — neither is "a file
that could not be deleted". Ship `failedMetrics` only if diagnostics want it; the copy must not read
it.

Two further notes for the architect: `PurgeCounts`/`PurgeReply` (`sink.rs:60-67`) carry a single
`failed_files` covering log parts **and** export zips, so the split has to happen inside
`writer::purge_scope`, not in the command layer. And the existing `failedFiles` total stops driving any
string — keep it for tests and diagnostics, but the branch selector and every clause read the
per-category fields, so the copy stays correct even if the total and the parts ever disagree.

**Branch selector: `failedLogs + failedExports > 0`, never `failedFiles > 0`.** With the total, a
metrics-only failure enters the failure branch and renders a failure sentence over a count of zero.

`{failure}` becomes, with `FL` = `failedLogs` and `FE` = `failedExports`:

| FL | FE | Clause |
|---|---|---|
| `> 0` | `0` | ` {FL} log file{s} could not be deleted — {it/they} may be open in another program.` |
| `0` | `> 0` | ` {FE} export{s} could not be deleted — {it/they} may be open in another program.` |
| `> 0` | `> 0` | ` {FL} log file{s} and {FE} export{s} could not be deleted — they may be open in another program.` |
| `0` | `0` | omitted — and the branch is not entered |

`it` iff the clause names exactly one item in exactly one category; the compound subject is always
`they`, even at 1 + 1. Templates T1–T3 and the `{deleted}`/`{freed?}`/`{usage}`/`{rolled?}` clauses
carry over from §6.11.3 unchanged. **Tone reads the per-category fields too:** `error` iff
`failedLogs + failedExports > 0 || !metricsCleared`. Equivalent to §6.11.3's `failedFiles > 0` while the
total and the parts agree, and correct rather than merely equivalent if they ever do not.

Every case the increment must cover, verbatim. **The announcement is identical to the toast in every
row — that is R12, and it is the answer to "do they differ": no, in no row.**

| Case | `text` = `announce` | tone |
|---|---|---|
| **Logs only failed** — partial (5 log files, 3 deleted, 2 locked; Dev ON; counts cleared) | `Deleted 3 log files. 2.1 MiB freed. 2 log files could not be deleted — they may be open in another program. Usage counts cleared. Still recording — Bonsai started a new log file.` | error |
| **Logs only failed** — none deleted (1 log file, locked; Dev OFF; counts cleared) | `1 log file could not be deleted — it may be open in another program. Usage counts cleared.` | error |
| **Exports only failed** — partial (4 logs deleted, 1 of 2 exports failed; counts cleared) | `Deleted 4 log files and 1 export. 3.4 MiB freed. 1 export could not be deleted — it may be open in another program. Usage counts cleared.` | error |
| **Metrics only failed** (Dev ON, 1 log file deleted, clear failed) | `Deleted 1 log file. 5.1 KiB freed. Usage counts were not cleared. Try again. Still recording — Bonsai started a new log file.` | error |
| **Metrics only failed, nothing else to delete** (Dev OFF, no logs, no exports, clear failed) | `Usage counts were not cleared. Try again.` | error |
| **Two failed** — logs + exports, nothing deleted, counts cleared | `2 log files and 1 export could not be deleted — they may be open in another program. Usage counts cleared.` | error |
| **All three failed** — nothing deleted at all | `2 log files and 1 export could not be deleted — they may be open in another program. Usage counts were not cleared. Try again.` | error |
| **Pathological** (9,998 of 10,010 logs, 126 of 129 exports, Dev ON, clear failed) | `Deleted 9,998 log files and 126 exports. 4.3 GiB freed. 12 log files and 3 exports could not be deleted — they may be open in another program. Usage counts were not cleared. Try again. Still recording — Bonsai started a new log file.` | error |

Both metrics-only rows carry **no** failure sentence, by design: the usage clause is the report. This is
the state `?obsDeleteFail=1` produces today (`mock/handlers/obs.ts:222-232` fails the metrics file
only), so it is the row the harness shows by default — see §6.11.5.

**The second metrics-only row is a real behaviour change, and the clearest proof that `failedMetrics`
must drive no clause.** With nothing deleted and only the clear failing, `failedLogs + failedExports`
is 0, so the branch is **T2, not T3** — `Usage counts were not cleared. Try again.` where R10 says
`1 file could not be deleted — it may be open in another program. Usage counts were not cleared. Try
again.` The R10 string double-reports one failure and attributes it to another program holding a file,
which for a `metrics` clear is false: `merge_metrics_counts` invented that `1`. `obsDeleteCounts.test.ts`
asserts the old string with `toBe`, so this row is a required test update, not a passing rename.

The pathological row is ~250 characters. `.toast` sets `overflow-wrap: anywhere` with no line clamp
(`toasts-and-overlays.css:22-30`), so it wraps to roughly seven lines in the 360px stack and **nothing
is truncated** — correct, because a result message may never be clipped, and also the clearest argument
for §6.11.6's inline home.

**The R10 regression guard becomes structural.** A user with no log files has `failedLogs === 0`, so no
string in this table can name a log file. The fabricated *"Deleted 0 of 1 log files."* is now
unreachable by construction rather than by a careful branch.

**`X of Y` stays rejected, even though the new fields make `Y` true.** Kept here so it is not
re-proposed:

1. `Deleted 0 of 1 log files.` — true under attribution, still the string §6.10 condemned, and still
   reading as a bug. R5's "never lead with `Deleted 0`" forces a second shape for `X === 0`, so
   `X of Y` cannot be the single grammar anyway.
2. With two categories failing it double-states both counts: `Deleted 3 of 5 log files and 1 of 2
   exports.` plus a failure sentence repeating `2` and `1`.
3. The denominator is not actionable. What is: **what could not be deleted, why, and what to do** — all
   three stated above, with `Y` available by adding two printed numbers.
4. It is a new sentence grammar for every row; the table above is a one-noun change from the shipped
   R10 grammar. Consistency beats local optimality.

`devLogMessages.ts:26-29`'s comment asserts `X of Y` "cannot be salvaged" *because* attribution does
not exist. When the fields land that premise is false while the conclusion stands. **Rewrite the
comment to reasons 1–4; do not delete it** — it is the only place the rejection is recorded in the
code.

#### 6.11.5 Harness states (`VITE_MOCK_IPC=1`)

Needed for R12/R13 (now):

- **default Dev ON / Dev OFF** — rows 1 and 3 of §6.11.3. Already derived from fixture state per R7.
- **`?obsDeleteFail=1`** — unchanged, and under R14 it is the metrics-only row. Keep the param name as
  an alias for back-compat with the existing tests.
- **`?obsMetricsUnreadable=1`** — new, and **the only way to see R13a**: returns `failedFiles: 0`,
  `deletedMetrics: 0`, `metricsCleared: false` with the log half succeeding. Today unreachable, and it
  is the state that ships a green toast reading "were not cleared".
- **`?obsDeleteFail=throw`** — new: rejects `logsDeleteAll` so the thrown path (`deleteErrorText` +
  `Nothing was deleted.`) can be seen at all. Currently unexercisable in the harness.
- **zero-bytes success** — R13b needs `deletedBytes: 0` with `metricsCleared: true`; reachable by
  making `?obsMetricsFresh=1` report the pre-first-flush state (aggregate live, nothing on disk).

Needed only if R14 ships:

- **`?obsDeleteFail=logs` / `=exports` / `=all` / `=partial`**, each deriving its counts from the
  fixture's own log/export state per R7's rule.
- **`?obsLogFiles=N` and `?obsExports=N`**, driving **both** `logSessionInfo` (`totalFiles`,
  `exportFiles`) and `logsDeleteAll`. **Without them two whole families of copy are invisible in the
  harness:** the fixture reports exactly one log file when Dev is ON (`obs.ts:216`) and
  `exportFiles: 0` in both states (`obs.ts:202-203`), so the `3 of 5` partial row and **every** export
  row cannot be reached — including §6.10 8b's confirm line `This includes {N} exported log
  archive{s}.`, which has never been seen rendered. That is R7's fidelity gap one field over, and it is
  MUST-FIX for the mock in the same increment.
- **pathological**: `?obsLogFiles=10010&obsExports=129&obsDeleteFail=partial` — exercises
  `Intl.NumberFormat` grouping, `GiB` formatting, and the seven-line toast wrap.

There is no empty state for this row (§6.4 dropped the `hasLogs` gate) and no loading state beyond
`anyBusy`, which is unchanged. **Verify by reading the text node**, as §6.10 did — for copy the string
*is* the evidence. Toast motion, dwell and dismissal stay USER CHECKPOINT items: the harness is
headless.

#### 6.11.6 Delivery surface — flagged MUST-FIX, deliberately not specced here

The task asked whether any string here can fire while a dialog is open. **The confirm dialog is not the
problem; the Settings card is.**

*Confirm dialog — clean.* `runDelete` pushes the toast at `DevCategory.tsx:149` and closes the dialog in
the `finally` at `:162`, with no `await` between them, so React 18 batches both into one commit: the
toast appears as the dialog unmounts. **Keep it that way** — an `await` inserted between the push and
the close would park the toast behind `.dialog-overlay` for the whole gap.

*Settings card — broken, and it is a house rule already.* `.toast-stack` is `z-index: 90`
(`toasts-and-overlays.css:11`); `.dialog-overlay` is `z-index: 100` with `background: rgba(0,0,0,0.45)`
over the full viewport (`dialogs.css:10-14`); `SettingsPanel.tsx:1` renders Settings *inside* that
overlay, with `.dialog-card.settings-card` 880px wide, `min(660px, 100vh − 64px)` tall and centred both
ways (`settings-shell.css:13-16`). So every toast raised from a Settings category is **always under the
45% black scrim**, and is **clipped by the card only when the two overlap vertically** — stated
precisely, because a flag that claims clipping where there is none will not survive first contact with
the harness:

- **Horizontal overlap** when they do collide: `812 − viewportWidth/2` px of the toast's 360 — 172px at
  1280 wide, 92px at 1440, none at ≥1624.
- **Vertical overlap** requires the card's top edge, `(viewportHeight − cardHeight)/2`, to be above the
  toast's bottom. The toast sits at `top: 52`. A two-line toast therefore collides only below roughly
  880px of height (at 1280×800 the card top is 70px: clipped). At **1920×1080 the card top is 210px and
  a short toast clears it entirely — washed out, not clipped.** The seven-line pathological toast of
  §6.11.4 reaches ~240px and collides even at 1080.

`ui-reference.md:2377` already rules this exact case: *a Settings-surface error is inline, never a
toast.* The dimming alone violates it; the clipping is the aggravating case, not the whole finding. The
delete and export toasts predate that ruling.

**MEASURED 2026-09-14 — it is worse than this subsection reported, and the correction matters for how
anyone verifies this surface.** `document.elementFromPoint` at the toast's **own centre** returns
`dialog-overlay <DIV>` (`onTopIsToast: false`), so the toast is **not the hit-test target anywhere on
its box: it is unclickable and its ✕ dismiss button cannot be reached at any width.** The 172 px
horizontal overlap derived above was confirmed exactly. Everything this subsection says about dimming
and clipping stands; "dimmed and partly clipped" simply understated it, and the reason it understated
it is the same reason two reviews missed the whole thing — geometry was inferred, not measured.
**Verification of this surface must use `elementFromPoint`/bounding boxes; `innerText` is blind to
stacking.**

**Why two harness reviews and two code reviews missed it:** §6.10's observations were read from
`innerText` (line 674-675, and correctly — for copy the text node is stronger evidence than pixels),
and `innerText` is blind to stacking. On top of that, on the likeliest developer monitor the toast is
fully on screen and merely dim, so a screenshot would have looked plausible too.

**Recommended fix, ~~reuse only~~ — CORRECTED 2026-09-14: this was true of the design and false of
the code.** `.settings-row-note` exists and is widely used (`settings-primitives.css:180`), but
**`.settings-row-note--warn` has zero users and no CSS rule anywhere in `src/`** — naming it "the
signed P107 recipe" made a rule that had only ever been *written down* sound like a rule that had been
*implemented*. It is built for the first time in `P113-settings-inline-notes.md` §4, with measured
ratios. The recommendation below is otherwise unchanged, and P113 supersedes it in full (the user
ruled on 2026-09-14 to sweep all ten call sites on this surface, not just this one). The shape of it:
the delete outcome lands in the delete row's note slot — the `.settings-row-note--warn` recipe (12%
`--warning` tint, `inset 3px 0 0 var(--warning)`, `--text-1`
ink) for the error tone, plain `.settings-row-note` for success — sitting *beside* the row's state note
per §12.2, with the button's `aria-describedby` composing both ids (R6's id is already there). **Every
string in §6.11 is channel-independent**: the copy does not change, only its home. One constraint from
`ui-reference.md:2386`: if that note becomes a live region, `DevCategory`'s single announcer must not
also fire for the same event or AT queues two utterances — **recommendation: keep the announcer live and
leave the note description-only.**

Not specced here because it changes a component, which is outside this task's copy mandate. **Needs the
orchestrator's call and its own increment.**

#### 6.11.7 Supersessions, the noun NIT, and test impact

**Superseded / amended** — per the convention, the old passages stay readable with a pointer:

| Passage | Status |
|---|---|
| §6.8 R5's **Announcement** column (both rows) | **SUPERSEDED by §6.11.1** — the announcement equals the toast |
| §6.10 R10's sentence *"The announcement is each row minus the ` — {it/they} may be open in another program` clause, matching the existing pattern."* | **SUPERSEDED by §6.11.1** |
| §6.10 R10's three-row failure table and its `file` noun | **CONDITIONALLY SUPERSEDED by §6.11.4** — R10 stays the live spec until `failedLogs`/`failedExports` ship |
| §6.8 R5's NIT (*"Append `Try again.` … in the `failedFiles > 0` path only"*) | **AMENDED by §6.11.2 R13a** — it attaches wherever `metricsCleared` is false. R5 did not consider the success-shaped path, which `metrics_purge.rs:52-57` makes reachable |
| §6.8 R5's `logParts === 0` **Toast** column | **AMENDED by §6.11.2 R13b** — the freed clause is omitted when `deletedBytes === 0`, and when the usage clause is the not-cleared form |
| §6.11.3's **T2 template** (`{usage}{ freed?}`) and its resolved-row table | **AMENDED 2026-09-14 (A-T2)** — T2 carries `{rolled?}` too, and the Dev-ON `?obsLogFiles=0` row was missing. Found by the code review of the implementation: **the code was right and this table wrong.** Receipt: `devLogMessages.ts:116`, `obsDeleteCounts.test.ts:203-205` |
| §6.11.8's **AC5** (*"can produce no string containing `log file`"*) | **REWORDED 2026-09-14 (A-AC5)** — unsatisfiable as signed: Dev ON ⇒ `rolled` ⇒ `…started a new log file.`, which R13c requires. Now scoped to a **counted** log file, `/\d+ log file/`. Reasoning: `obsDeleteCounts.test.ts:198-202` |
| §6.11.6's *"the toast is dimmed and partly clipped"* (and reason 4 of §6.11.1) | **CORRECTED 2026-09-14** — measured unclickable; `elementFromPoint` at the toast's centre returns `dialog-overlay`. The 172 px figure was exact |
| §6.11.6's *"Recommended fix, **reuse only**"* | **CORRECTED 2026-09-14** — `.settings-row-note--warn` had zero users and no CSS rule; it is written for the first time in `P113-settings-inline-notes.md` §4. **SUPERSEDED in full by P113**, which sweeps all ten call sites on this surface per user ruling #24 |

**Not** superseded, and unchanged: §6.4 in full; §6.10 R8, R9 and R11; the `{usage}` clause wording;
`deleteErrorText` and `Nothing was deleted.` (the thrown path).

**Noun NIT — ratified, not changed.** The confirm dialog says `exported log archive{s}`
(`DevConfirmDialogs.tsx:13`) and the outcome says `export{s}`. Deliberate: the dialog *defines* the
object for a user who may not know what is in the exports folder, while the outcome *counts* it inside a
compound sentence that already carries "log files", where the long noun pushes the toast past two lines
for no gain. `ExportConfirmDialog` (`:105`) already uses the short plural. **Guard: the short noun is
permitted only in the outcome sentence** — anywhere the object is introduced rather than counted, it is
`exported log archive`.

**Test impact — hand these to `tester`, not `senior-dev` alone** (R10's rule: assertions cement copy,
and the expected strings below are derived from each fixture, not guessed).

Unconditional (R12/R13):

| Assertion | Fixture | Breaks because | Replace with |
|---|---|---|---|
| `obsDeleteCounts.test.ts:123-124` | Dev OFF, `failedFiles 1`, not cleared | parity | `expect(t.announce).toBe(t.text)` |
| `dev.test.tsx:173` | `deletedFiles 1, deletedMetrics 1` | parity | `expect(t.announce).toBe(t.text)` |
| `dev.test.tsx:185` | `deletedFiles 3, deletedExports 2, deletedMetrics 1` | parity | `expect(t.announce).toBe(t.text)` |
| `dev.test.tsx:254` | `deletedFiles 1, deletedMetrics 1, failedFiles 1` | parity | `expect(t.announce).toBe(t.text)` |
| `dev.test.tsx:199-200` | `L 0, E 0, metricsCleared false, deletedMetrics 0` | R13a adds ` Try again.`; R13b drops the freed clause | `expect(t.text).toBe('Usage counts were not cleared. Try again.')`, `expect(t.announce).toBe(t.text)`, `expect(t.tone).toBe('error')` |
| `dev.test.tsx:156` | rolled success | unaffected | keep |
| `dev.test.tsx:151`, `:224`, `obsDeleteCounts.test.ts:58,81,100-102` | — | `toContain`/prefix matches survive | keep |

Add **one invariant test** covering every fixture in the file: `expect(t.announce).toBe(t.text)`. It is
two lines and it closes the class that took R5, R9, R10 and R12 to chase.

Conditional (R14 only) — each of these asserts the generic `{F} file{s} could not be deleted` noun and
must gain per-category fields in its fixture. Note the first one **changes meaning**: the mock's own
comment says the failing file is the metrics one, which under R14 yields *no failure sentence at all*.

| Assertion | Fixture today | Honest per-category fixture | Expected clause |
|---|---|---|---|
| `obsDeleteCounts.test.ts:100-101`, `:121` (a `toBe` on the whole string) | Dev OFF, `failedFiles 1`, metrics failed, nothing deleted | `failedLogs 0, failedExports 0` | no failure sentence; the whole string becomes `Usage counts were not cleared. Try again.` for both `text` and `announce`, tone `error` |
| `dev.test.tsx:218` | `base`, `failedFiles 1`, not cleared | `failedLogs 1` (a held-open log part) | `1 log file could not be deleted — it may be open in another program.` |
| `dev.test.tsx:240` | exports row, `failedFiles 1` | `failedExports 1` | `1 export could not be deleted — it may be open in another program.` |
| `dev.test.tsx:252` | log held open | `failedLogs 1` | `1 log file could not be deleted — it may be open in another program.` |
| `DevCategory.test.tsx:139` | `failedFiles 1`, `deletedFiles 4` | `failedLogs 1` | `1 log file could not be deleted` |
| `dev.test.tsx:219` (`not.toContain('of 5')`) | — | — | keep: `X of Y` stays rejected (§6.11.4) |

`obsDeleteCounts.test.ts:107-124`'s Dev-OFF guard keeps its point under R14 and gets stronger: assert
`not.toContain('log file')` with `failedLogs: 0`, which is now structurally guaranteed.

#### 6.11.8 Acceptance criteria

1. `deleteResultToast` returns `announce` byte-identical to `text` for every input; an invariant test
   asserts it across all fixtures.
2. `tone` is `error` whenever `metricsCleared === false`, including when no file failed.
3. No string contains `0 B freed`, and no failure sentence is followed by a freed clause.
4. With `?obsMetricsUnreadable=1` the harness shows an **error**-toned `Usage counts were not cleared.
   Try again.` and the same text in the live region.
5. **REWORDED 2026-09-14 (A-AC5) — as first signed this criterion was unsatisfiable.** A user with
   **no log files on disk** (`totalFiles === 0` — which is not the same as Dev mode off, since logs
   outlive the toggle) can produce no string naming a **counted** log file — i.e. no match for
   `/\d+ log file/` — with and without a forced failure.
   *Why the original wording could never pass:* it forbade the substring `log file` outright and
   explicitly included the Dev-ON case, but Dev ON ⇒ `rolled` ⇒ ` Still recording — Bonsai started a
   new log file.`, which R13c **requires**. The two clauses contradicted each other, and no
   implementation could satisfy both. The defect this criterion exists to catch is a *false
   statement* — a count of log files deleted from a user who had none (§6.10 R10's
   `Deleted 0 of 1 log files.`) — and `Bonsai started a new log file.` is not that: it is true, names
   no count, and is about a file being **created**. The implementation tests `/\d+ log file/` and is
   correct; the reasoning is already written into `src/ipc/mock/obsDeleteCounts.test.ts:198-202`.
   **The code is right; this criterion was wrong.**
6. *(R14 only)* The failure sentence names the true category, is absent for a metrics-only failure, and
   `failedFiles` is read by no string.

**The unknown (`info === null`) state cannot be seen in the harness — USER CHECKPOINT or a new
fixture flag.** Nothing in `src/ipc/mock/handlers/obs.ts` ever fails `logSessionInfo`; it returns a
zero-filled record when Dev mode is off and never throws, so neither R8a's hint nor R8b's archives
line nor R3/R11's lead line is reachable in a browser. Two options, **recommendation: (a)** —
R7 already set the precedent that a state named in a copy ruling must be visible in the harness, and
without it the whole unknown-state family stays unverifiable by anyone but the user:

- **(a)** `senior-dev` adds `?obsInfoFail=1` to the mock, making `logSessionInfo` reject with a
  mapped error so the page's `info` stays `null`. Fixture-only, in scope of R7.
- **(b)** R8/R11 are covered by unit tests only, and the visual check moves to USER CHECKPOINT.

#### Follow-ups, not this increment

- **`logsNote` and the `title="No logs yet."` tooltips collapse `null` → "no logs"** the same way 8a
  did (`SettingsDevLogsSection.tsx:62-75, 90, 102`). Left alone deliberately: they only disable two
  non-destructive actions, so the collapse errs safe. SHOULD-FIX whenever that row is next touched.
- **`LogsDeleteResult` has no per-category failure counts** (`failedLogs` / `failedExports` /
  `failedMetrics`). R10 works around it with an honest generic noun; an architect follow-up could
  make the partial-failure toast say which kind of file failed. Not worth an IPC change on its own.
- **The row hint can be one poll stale after the Dev-mode toggle** — observed as
  `Removes all 1 log file (0 B)…` immediately after switching Dev mode off. Self-corrects on the
  next poll, and opening the delete dialog re-reads, so the *consented* count is never the stale
  one. Cosmetic; log it, do not chase it.

---

## 7. Layout, themes, densities, states

Copy-only. No geometry, spacing, colour or token changes; §7's `12px`/`--text-2`/`max-width: 56ch`/
`8px` paragraph gap and §4.3's dialog geometry stand as written.

- **Dark / light:** identical strings, identical tokens. `.mono` spans and `.dev-warning-bar` are
  unchanged, so no contrast pair changes and no ratio needs re-checking.
- **`cozy` / `compact`:** the privacy block is prose inside `SettingsGroup`; density affects only the
  group's own padding, which is untouched. ¶3–¶5 each grow by roughly one line at `56ch`, and §6's ¶7
  adds about four lines at `56ch` (~110 words) — the Settings page already scrolls, and nothing is
  clipped. Both densities are unaffected because nothing here has a row height.
- **States:** the panel is static prose outside the disabled fieldset — no hover, pressed, disabled,
  loading or error variant, and no focusable element is added. The only variable is the trailing
  `Right now:` note, whose two strings are unchanged.

**Added by §6.** Still copy-only, but §6.4 touches a *control*, so its states are specced here:

- **The delete row (`dev.delete-logs`).** Default / hover / active / `:focus-visible` / busy are
  `.btn-danger`'s and are unchanged. The **disabled** state is *removed* (§6.4) — the row is always
  enabled, so `aria-disabled` now carries only `anyBusy`. No hue, geometry or hit-target change; the
  button's label grows from `Delete logs…` to `Delete all…`, which is **shorter**, so no row can
  reflow.
- **Row label overflow.** `Delete logs and usage counts` is 27 characters against the label column's
  existing worst case on this page (`Include raw repository names`, 28) — no new truncation case, and
  `ui-reference.md` §12.2's label ellipsis + `title` behaviour is unchanged.
- **Dialog long-content.** `DeleteLogsConfirmDialog` gains one paragraph (§6.4). Its body already
  scrolls; with 5–6 short paragraphs it stays under the `ConfirmDialog` max height at 720px viewport.
  The interpolated `{N}` / `{size}` are the only variable-length values and are bounded (≤10 files,
  bytes formatted).
- **Both themes.** `.mono` on `metrics` is the same span treatment as `ref#3`; no new token, no new
  contrast pair, nothing to re-measure in either theme.
- **Overflow / long content:** no interpolated value appears in any string specified here, so there is
  no pathological-length case to design for. This is deliberate; do not introduce one.
- **Dialogs:** all states (`busy`, Esc/Enter, focus trap, focus restore, default focus on Cancel) are
  `ConfirmDialog`'s and are unchanged.
- **Motion:** none added.

## 8. Accessibility

- **Do not add `aria-live` to the privacy block.** §14 and §5.3 stand: the block is a `role="group"`
  named by its heading, and the strict↔raw transition is announced **once per transition** by the
  single page-level polite region, not by this panel. The panel's `Right now:` note must remain a
  plain node.
- The block re-renders on every 2 s status poll. Its text does not depend on poll data, so the DOM
  text nodes must be **identical across polls** — no `key` churn, no re-derived string. This is the
  §8.4 stickiness rule applied to copy: announce on transition, never on poll.
- Dialog bodies remain inside `ConfirmDialog`'s described region; all three paragraphs are read as the
  description. No paragraph may be moved outside it.
- No new focusable target, so no new hit-target or focus-ring consideration.

## 9. Harness verification (`pnpm dev:mock`, port 1420)

§2–§5 need no fixture change and add no IPC field. **§6 does need mock work** — `src/ipc/mock/obs.ts`
must make the new copy verifiable, otherwise the harness shows a button whose promise cannot be
checked:

| Fixture state | Why it is needed |
|---|---|
| metrics present with `sessions > 0` and a few day buckets (the **default**) | ¶7 and the `n > 0` dialog branch; this is the normal case |
| **zero log files, metrics present** | the state that used to disable the row (§6.4). Verifies the row is enabled, the hint reads `No log files yet. Removes Bonsai's usage counts.`, and the dialog takes its `n === 0` branch. **This is the state the ruling exists for** — without this fixture the fix is unverified |
| `logsDeleteAll` clears the metrics fixture too | otherwise the harness contradicts the copy: the button would claim to clear usage counts and leave them |
| day buckets spanning **> 90 days** | proves the window is 90 and not 400 in the mock as well as in Rust |
| a delete that **fails** (`failedFiles > 0`) | the existing warning result state, re-checked because the confirm body changed |

Verify at 1440×900 in both themes:

1. Settings → Developer, Dev mode **off**, `rawNames: false` → panel ¶3/¶4/¶5 read as §3; trailing
   note `Right now: names are replaced.`
2. `rawNames: true` fixture → ¶5 unchanged in wording, warning-bar note `Right now: raw names are
   included.`
3. Toggle "Include raw repository names" on → `RawNamesConfirmDialog` shows §2's three paragraphs.
4. `Export…` with `redaction: 'raw'` and with `'strict'` → §4's two branches.
5. Light theme + 1024px-wide window: no clipping, `56ch` measure holds.

No USER CHECKPOINT item is introduced: every surface here is reachable in the browser harness.

## 10. Inaccuracies found in the existing copy

1. **`DevConfirmDialogs.tsx:36-38` and `SettingsDevPrivacySection.tsx:36-46` omit remotes.** §7.1 puts
   remote URLs in the raw column (`remote#<n>` → full URL). Both surfaces listed only branches, tags,
   files and repository. Fixed in §2/§3/§4.
2. **Full commit SHAs were nowhere disclosed.** Raw replaces the 7-char prefix with the full SHA; no
   surface said so. Fixed.
3. **The repo path's account name was never mentioned.** Nothing masks home directories
   (`scrub.rs` has no home/username rule), so a raw absolute path can carry the OS account name into
   a mailed zip. Now stated in §2, once, without alarm.
4. **"your name or email address" was ambiguous** — read literally it contradicts item 3. Replaced with
   "the name and email address you commit under" everywhere.
5. **`ExportConfirmDialog` carried a second, shorter "never" list** than the panel. Two lists in one
   flow is the exact condition that produced the original defect. Unified in §4/§5.
6. Not a copy defect, recorded so it is not "fixed" later: the panel says the numbers "change next
   time, so two files cannot be matched up". That remains true under A26 (per-session salt-seeded
   counters, §7.2) and must not be weakened.
7. **Added 2026-09-11 — the delete control's own promise was the other half of F6.** The row label
   `Delete all log files`, the hint `Removes all N log files … from this computer.` and the dialog
   title `Delete all log files?` all described a scope narrower than what the user needed cleared,
   and `'No log files to delete.'` disabled the control for the user with no logs and 90 days of
   usage counts. Fixed in §6.4. The general lesson: **a disclosure gap and a destructive control's
   label are the same defect seen from two sides** — fixing the prose without the label leaves the
   app telling the truth in one paragraph and contradicting it on the button below.

## 11. Acceptance criteria

1. `DevConfirmDialogs.tsx` `RawNamesConfirmDialog` body matches §2 verbatim; still 3 `<p>`; `ref#3`
   and `path#7` still `.mono`.
2. `SettingsDevPrivacySection.tsx` ¶3/¶4/¶5 match §3 verbatim; ¶1/¶2/¶6/¶7 and the `Right now:` note
   byte-identical to today; `remote#1` rendered with `.mono`.
3. `ExportConfirmDialog`'s two content statements match §4 verbatim; the `dev-warning-bar` + glyph
   remain on the raw branch only.
4. Grep across the three surfaces: no occurrence of the word "arguments"; every surface that names a
   "never" list contains all five §5 clauses in order.
5. No `aria-live` on `SettingsDevPrivacySection`; no new token, no `src/styles/**` change, no new
   component file.

**Added 2026-09-11 by §6's ruling (6.1–6.9). All are in scope; 6.3 is the one that fails silently.**

6. **Copy.** ¶7 matches §6.2 verbatim and sits between ¶6 and the "plain text" paragraph; ¶6 matches
   §6.3 verbatim; the group heading is `What Bonsai records` in **both** `SettingsDevPrivacySection.tsx`
   and `catalog/dev.ts` (`group` *and* `label`). The panel is 8 `<p>` plus the `Right now:` note.
7. **Label pair — count by SITE, not by number. REWRITTEN 2026-09-14 (§6.8 R1+R2); the original
   four-site list was wrong twice.** `Delete logs and usage counts` must appear at each of these
   **five** sites and nowhere else:

   | # | Site | Form |
   |---|---|---|
   | 1 | `SettingsDevPrivacySection.tsx` **¶6** | `&ldquo;…&rdquo;` (§6.3 puts it here — the original AC omitted it) |
   | 2 | `SettingsDevPrivacySection.tsx` **¶7** | `&ldquo;…&rdquo;` |
   | 3 | `SettingsDevLogsSection.tsx` `rowLabel` prop | bare |
   | 4 | `DevConfirmDialogs.tsx` `DeleteLogsConfirmDialog` title | with a `?` |
   | 5 | `DevConfirmDialogs.tsx` `ExportConfirmDialog` trailing line | `&ldquo;…&rdquo;` (§6.8 R1) |

   **`catalog/dev.ts` is NOT one of them** (§6.8 R2): its `dev.delete-logs` label is the button text
   `Delete all…`, as the DOM↔catalog guard requires. **Do not turn this into a raw-match total:** the
   board's grep-counting rule applies, and an implementer chasing a number would "fix" it by deleting
   the catalog entry, which silently removes the row from settings search. `Delete all log files`
   must appear **nowhere** in `src/`; `usage.json` must appear in **no** user-facing copy.
8. **Delete scope (the silent-failure one).** After the delete action, `metrics/` is gone — both
   `usage.json` and `usage.json.bak` — **and** a subsequent `metrics_snapshot` returns a fresh file
   (`sessions` restarted, `days` empty). A test that only checks `usage.json` is missing passes while
   the feature is broken, because the next flush restores it from memory. Assert the snapshot.
9. **Enabled state.** With **zero** log files and metrics present, the delete row's button is
   enabled, has no `title` tooltip, its hint reads `No log files yet. Removes Bonsai's usage counts.`,
   and the confirm dialog's first line is `Clear Bonsai's usage counts? This cannot be undone.`
10. **Retention.** `RETAIN_DAYS == 90`; no "400" and no "about a year" survives in `src/`, `src-tauri/`
    or in copy.
11. **Regression.** `DevCategory.test.tsx:113`'s dialog name and any settings-search test that keys
    off the old group or row label are updated, not deleted. Settings search for `usage` finds the
    privacy block; search for `delete` finds the delete row.
