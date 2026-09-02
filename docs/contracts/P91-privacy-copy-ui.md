# P91 — Dev-mode privacy consent copy (satisfies `P91-raw-args-privacy.md` AC12)

**Status:** SPEC — ui-designer, 2026-09-03. Branch `feat/p91-observability`.
**Amends** `docs/contracts/P91-observability-ui.md` §7 (frozen privacy statement) and §4.3 (raw-names
confirmation), and §8.3's export content statement. Where this file and §7/§4.3/§8.3 disagree,
**this file wins**; everything else in those sections (geometry, roles, live-region policy, states) is
unchanged.

**Input contracts:** `P91-raw-args-privacy.md` §A (the ruling) and §F (the flag); `P91-observability.md`
§7.1 (as replaced by A26), §7.2, §7.3, §6.1, §6.2, §8 (metrics).
**Implementer:** `senior-dev`. **Files touched:** `src/components/settings/DevConfirmDialogs.tsx`,
`src/components/settings/SettingsDevPrivacySection.tsx`. Copy only — no layout, no new component,
no new token, no `src/styles/**` change.

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

Structure, `dev-warning-bar`, glyph, labels and the trailing two lines are **unchanged**. Replace only
the two content statements:

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

## 6. `usage.json` disclosure — **PENDING USER DECISION, do not implement unflagged**

Audit finding F6. This is a product call about what the app admits to keeping, so it is the user's,
not mine. Below is the copy to use **if** disclosure is chosen, the honest statement of today's
behaviour, and my recommendation.

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

### 6.2 Option A — disclose, no button (**my recommendation**)

Add as **¶7**, between the "Log files stay on this computer" paragraph and the "plain text"
paragraph. One paragraph, same `<strong>` lead-in pattern, no warning bar:

> **Bonsai also keeps a small usage count.** Whether or not Dev mode is on, Bonsai counts how often
> its own actions run and how long they take, and keeps daily totals for about a year. It stores only
> Bonsai's own action names — never a repository, branch, file, or anything you typed — and it never
> leaves this computer. It is a separate file, so "Delete all log files" does not remove it and it is
> not included in an export. To clear it, delete the `metrics` folder next to your log files.

Why this and not the alternatives: it costs one paragraph, it is the smallest true statement, and the
final sentence gives the user a real remedy without contradicting §10's ratified "no reset button on
the Developer page". `metrics` (the folder) rather than `usage.json` (the file) is deliberate and
load-bearing — deleting only `usage.json` leaves `usage.json.bak` to be restored on next load, so
naming the file would be advice that does not work.

### 6.3 Option B — disclose and add a `Reset usage counts` row

Same paragraph, plus a fourth action row calling `metrics_reset` behind a danger `ConfirmDialog`.
**I do not recommend it:** it reverses `P91-observability-ui.md` §10 in the same milestone that
ratified it, adds a destructive control to a page that cannot show what it destroys, and buys the user
nothing that the folder-delete sentence does not already give them. It becomes right when the
Statistics page ships — that is where it belongs.

### 6.4 Option C — say nothing (status quo)

Defensible only if `usage.json` is considered out of the panel's subject ("what a log file contains").
I judge it not defensible for **this** panel, because the panel is the app's single privacy
disclosure surface and the delete button's promise reads wider than it is. If the user picks C, no
edit is needed and F6 stays open as a known disclosure gap.

**Until the user rules: implement §2–§5 only.** ¶7 ships in a follow-up commit if Option A is chosen.

---

## 7. Layout, themes, densities, states

Copy-only. No geometry, spacing, colour or token changes; §7's `12px`/`--text-2`/`max-width: 56ch`/
`8px` paragraph gap and §4.3's dialog geometry stand as written.

- **Dark / light:** identical strings, identical tokens. `.mono` spans and `.dev-warning-bar` are
  unchanged, so no contrast pair changes and no ratio needs re-checking.
- **`cozy` / `compact`:** the privacy block is prose inside `SettingsGroup`; density affects only the
  group's own padding, which is untouched. ¶3–¶5 each grow by roughly one line at `56ch` — the
  Settings page already scrolls, and nothing is clipped.
- **States:** the panel is static prose outside the disabled fieldset — no hover, pressed, disabled,
  loading or error variant, and no focusable element is added. The only variable is the trailing
  `Right now:` note, whose two strings are unchanged.
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

Existing fixtures suffice — this change adds no IPC field, so `src/ipc/mock/obs.ts` needs no edit.
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
6. §6's ¶7 is **absent** unless the user has chosen Option A.
