# P56 — Local AI changelog / release-notes — USER CHECKPOINT checklist (native-only)

These items require the native Tauri window, a **real `claude` CLI**, a **real repo with tags**, and
human judgement — they CANNOT be self-declared by the orchestrator. The AI gate only proves the
**structure** (unit tests: range resolution, resolve-last-tag reachability, pre-CLI bails, wire
shapes, payload grounding) and the **mock-driven** UI wiring (browser harness with canned
`aiHandlers`). The native checkpoint is about **real-model prose quality, real range resolution,
and privacy** — not whether the dialog/menu exists.

P56 is **READ-ONLY: it WRITES NOTHING to git** (no ref move, no commit, no index/worktree change, no
`repo-changed` event). So — unlike P55 — there is **no destructive-mutation risk**; the surface to
judge is entirely (a) notes quality, (b) correct range resolution, (c) gating + local-only privacy.

Run via `pnpm tauri dev` against a **real repo that has at least two tags** with the real,
authenticated `claude` binary on PATH and **AI consent enabled** in Settings. Entry points
(contract §6):
- **Primary (one-click):** right-click a **tag pill** → context menu → **"Release notes since
  previous tag"** → `aiChangelog(repoId, { kind:'sinceLastTag', target: <tag> })` — "what shipped in
  `<tag>`" (notes for the range previous-tag..`<tag>`, OQ7).
- **General:** the **"Release notes…"** action opens `ChangelogDialog` — a radio pick of **Between
  refs** (`from`/`to`, `to` seeded to the current branch/HEAD) vs **Since last tag** (optional
  target, default HEAD).
- Results render in the **`AiOutputPanel`** titled `Release notes: <fromRef>..<toRef>` (the resolved
  refs — for since-last-tag, `fromRef` is the resolved previous-tag name), with **Copy** and an
  **editable textarea**.

## Already proved by the AI gate (do NOT re-verify manually)

- **Range resolution + reachability, unit-tested** (`crates/bonsai-core` `ai_changelog`, 10 tests):
  - `resolve_last_tag_finds_previous` — along a tagged chain (`v1`@A, `v2`@C, `v3`@E),
    `resolve_last_tag(E)` = `v2` (**excludes** `v3`@E itself), `resolve_last_tag(A)` = `None`, and a
    target between tags picks the nearest reachable earlier tag.
  - `since_last_tag_maps_to_previous_tag` — `SinceLastTag{target:v3}` resolves `fromRef=="v2"`,
    `toRef=="v3"`.
  - `between_refs_commit_set` + **CLI oracle** (`tests/`, degrade-skips if `git` absent) — the listed
    commit set == `git log --format=%h <from>..<to>` (membership **and** order) for both a tag range
    and `SinceLastTag`.
- **Fail-closed BEFORE any CLI call, unit-tested** (a fake `claude` bin **panics if spawned**):
  - `empty_range_fails_before_cli` — `from==to` / no changes → `AiFailed`, CLI never invoked.
  - `no_earlier_tag_fails_before_cli` — untagged repo → `SinceLastTag` → `AiFailed`, CLI never
    invoked.
- **Grounding payload, unit-tested** (stub harness) — the CLI stdin carries a `COMMITS:` block and a
  `NET CHANGES (diffstat):` section (reuses `render_commit_list` / `render_headers`); commit list is
  capped at `MAX_CHANGELOG_COMMITS` with a `(+N more)` note.
- **Wire / schema, unit-tested:** `changelog_range_deserializes_each_variant`
  (`{"kind":"betweenRefs",…}`, `{"kind":"sinceLastTag"}`, `{"kind":"sinceLastTag","target":"HEAD"}`),
  `changelog_wire_shape_is_camel_case` (`text`/`fromRef`/`toRef`/`commitCount`/`costUsd`; `None`→`null`),
  `prompts_are_single_line`.
- **Consent gate (structural):** `ai_changelog_inner` returns `AiUnavailable` unless
  `ai_enabled && ai_consented`, gated **before** the repo path is resolved / any CLI spawn. Read-only
  ⇒ does **not** emit `repo-changed`. Command delta **+1** (`aiChangelog`).
- **`tsc` + `pnpm build` clean; `pnpm test` (vitest) green.**
- **Browser harness (`VITE_MOCK_IPC=1`, canned `aiHandlers`):** tag-pill "Release notes since
  previous tag" → grouped Markdown in `AiOutputPanel` (with a working Copy button and a "Copied"
  confirmation); `ChangelogDialog` between-refs submit → notes; the panel offers the editable
  textarea variant; `?ai=off` → the panel's error banner ("Claude Code CLI not found on PATH").

So below is strictly what a **live model + real repo** must confirm.

## A. Tag-pill one-click — "Release notes since previous tag" (real model + real repo)

Set up: a repo with **≥ 2 tags** and real commits between them (e.g. `v1.2.0` then `v1.3.0` with a
handful of feature/fix commits in between).

- [ ] Right-click the **`v1.3.0` tag pill** → context menu shows **"Release notes since previous
      tag"** (enabled; disabled/greyed only when AI is not eligible).
- [ ] Selecting it opens the `AiOutputPanel` titled **`Release notes: v1.2.0..v1.3.0`** — i.e. the
      resolved **previous tag** (`v1.2.0`) is correct, NOT `v1.3.0` itself and NOT HEAD.
- [ ] The notes are **sensible grouped Markdown**: a one-sentence summary, then `### Features` /
      `### Fixes` / … headings **in the fixed order**, empty groups omitted; each bullet is a
      human-readable description followed by a **real short hash** that actually exists in
      `v1.2.0..v1.3.0`. Merge commits / pure version bumps are omitted. Spot-check 2–3 hashes with
      `git show <hash>`.
- [ ] The changes cited **actually shipped in `v1.3.0`** (cross-check against
      `git log --oneline v1.2.0..v1.3.0`) — no invented commits, nothing from before `v1.2.0`.

## B. General ranges via the "Release notes…" dialog (real model + real repo)

- [ ] **Between refs:** open `ChangelogDialog`, pick **Between refs**, enter `from = v1.2.0`,
      `to = v1.3.0` → notes for exactly that range (matches A). The `to` field is pre-seeded to the
      current branch/HEAD; leaving it as that default and entering only `from` also produces notes
      up to the current tip.
- [ ] **Since last tag from HEAD:** pick **Since last tag**, leave **Target empty** → notes since the
      most recent tag reachable from HEAD; the panel header shows `<latest-tag>..HEAD` with the
      correct resolved latest tag.
- [ ] **Since last tag from a target:** pick **Since last tag**, set **Target = `v1.3.0`** → resolves
      the tag before `v1.3.0` (i.e. `v1.2.0..v1.3.0`) — same result as the tag-pill one-click.

## C. Clear messages on the edge cases (real repo)

- [ ] **Empty range:** a range with no commits/changes between the two refs (e.g. `from == to`, or two
      refs at the same commit) → a clear message like *"no changes between `<from>` and `<to>`"* in the
      panel's error banner. **No** empty/garbage notes are produced.
- [ ] **No earlier tag:** "Since last tag" against the **very first tag** (or an untagged repo / a
      target with no earlier tag) → a clear message like *"no earlier tag found before `<to_ref>`"*.
      Confirm the CLI is **not** consulted (the message is instant — no model round-trip / no cost).
- [ ] **Bad ref:** a Between-refs range with a nonexistent ref (e.g. `from = does-not-exist`) → a
      clear git error, not a crash.

## D. Copy + editable textarea — the "tweak before paste" flow (native clipboard)

- [ ] **Copy** yields **pasteable Markdown**: click Copy (button shows a transient "Copied"), then
      paste into a `CHANGELOG.md` / a GitHub release draft / any editor → the exact Markdown source
      lands (headings, bullets, hashes intact). (The browser harness clipboard is unreliable — this
      must be checked in the **native** window.)
- [ ] **Edit before Copy:** the output renders in an **editable textarea**; tweak a bullet / reword the
      summary, then Copy → the **edited** text is what gets copied (Copy grabs the live draft, not the
      original model output).
- [ ] **Edits are ephemeral (expected, by design):** the draft is **not** persisted — re-running the
      notes, switching selection, or closing/reopening the panel **discards** your edits. Copy is the
      only persistence. Confirm this is the behaviour (so nobody expects saved drafts).

## E. Consent gate + privacy (real model)

- [ ] **Consent gate:** turn **AI consent OFF** in Settings → the tag-pill "Release notes since
      previous tag" and the "Release notes…" entry are disabled / error via the consent gate with a
      clear message; **nothing spawns the CLI**. Re-enable → it works again without restarting.
- [ ] **CLI missing:** remove/rename `claude` from PATH (consent ON) → a clear
      `aiUnavailable`-style message ("Claude Code CLI not found …"), not a crash or silent no-op.
- [ ] **Read-only guarantee:** while a notes panel is open (and after Copy/edits), `git status`,
      `git reflog`, and branch tips are **unchanged** — P56 writes nothing to git.
- [ ] **Local CLI only (no code leaves the device):** the only egress is the local `claude` child
      process you already authenticated; grounding (the commit list + diffstat) is passed via
      **stdin**, identical to running `claude` yourself. Bonsai opens **no** network connection to any
      AI endpoint. (Optional: confirm with a process/network monitor.)

## Sign-off
- [ ] A (tag-pill one-click: correct resolved previous tag; sensible grouped Markdown citing real
      hashes that shipped in the tag)
- [ ] B (dialog: between-refs + since-last-tag-from-HEAD + since-last-tag-from-target all resolve and
      generate correctly)
- [ ] C (empty range + no-earlier-tag + bad-ref all show clear messages; no-earlier-tag never hits the
      CLI)
- [ ] D (Copy is pasteable Markdown; editing before Copy copies the edited draft; edits are ephemeral)
- [ ] E (consent-off + CLI-missing gated with clear messages; read-only verified; local-CLI-only
      privacy)
