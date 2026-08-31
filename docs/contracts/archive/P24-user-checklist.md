# P24 — USER CHECKPOINT checklist (AI-asset management: inventory + drift + context profiles + AI translate)

Run these in the **native app** (`pnpm tauri dev`) against a **scratch repo you create** — NOT a
real repository you care about (activation and the AI helper write real instruction files into the
workdir). These are exactly the items the AI gate could NOT self-verify: the native AI Assets panel
driven by real filesystem hashing, drift chips that track a real edit, the safety-gated
diff-preview activation writing real files to disk, `.bonsai/profiles.json` being created, and
(P24e) the real `claude` CLI translation path with the consent gate.

Keep a **second terminal** open in the scratch repo for the disk/`git` verifications below.

The AI Assets surface opens from the **🤖 button in the top header** (`title` / `aria-label` =
"AI Assets"); it is only shown when a repo is open.

---

## 0. Prepare a scratch repo with several instruction files (throwaway — safe to delete)

In a terminal, in a throwaway folder under `D:\Temp` (never C:):

```
cd /d D:\Temp
git init -b main p24-scratch
cd p24-scratch
git config user.name "P24 Tester" && git config user.email "p24@example.com"
git config core.autocrlf false

REM --- canonical + a few sibling instruction files (some in sync, one drifted) ---
printf "# Project\nShared guidance for all agents.\n" > CLAUDE.md
printf "# Project\nShared guidance for all agents.\n" > AGENTS.md
mkdir .github
printf "# Project\nShared guidance for all agents.\n" > .github/copilot-instructions.md

REM --- a Cursor rules dir with two members (detected, not drift-compared) ---
mkdir -p .cursor/rules
printf "---\ndescription: a\n---\nrule a\n" > .cursor/rules/a.mdc
printf "---\ndescription: b\n---\nrule b\n" > .cursor/rules/b.mdc

git add -A && git commit -m "seed instruction files"
```

Open **`D:\Temp\p24-scratch`** in Bonsai. (Adjust the `printf` calls for your shell — the point is
that `CLAUDE.md`, `AGENTS.md`, and `.github/copilot-instructions.md` have *byte-identical* bodies to
start, and `.cursor/rules/` holds two `*.mdc` members.)

---

## 1. AI Assets panel lists the instruction files with correct exists / drift chips

1. Click the **🤖 "AI Assets"** button in the top header. A panel titled **AI Assets** opens.
2. In the **Managed instruction files** group, confirm one row each for `CLAUDE.md`, `AGENTS.md`,
   `copilot-instructions.md`, `GEMINI.md`, `.windsurfrules`, `.cursorrules`, with an **exists**
   indicator: the three you created show as present; `GEMINI.md` / `.windsurfrules` / `.cursorrules`
   show as **missing**.
3. Confirm the sync chips: `CLAUDE.md` is the **canonical** reference; `AGENTS.md` and
   `copilot-instructions.md` show **in sync** (identical bodies); missing rows show **missing**.
4. Confirm the header badge reads **In sync** (green) because every existing comparable file matches
   the canonical.
5. In the **Detected (not managed)** group, confirm `.cursor/rules/` appears with a **member count
   of 2**, plus `.mcp.json` / `.claude/` listed read-only with a "managed in a later release" style
   note. These have no drift chip.

## 2. A real edit that drifts AGENTS.md flips the chip on Refresh

6. In the second terminal, introduce a genuine content change (not just whitespace) to `AGENTS.md`:
   ```
   printf "# Project\nDIFFERENT guidance now.\n" > D:\Temp\p24-scratch\AGENTS.md
   ```
7. In the panel, click **Refresh** (or refocus the window). The `AGENTS.md` row chip must flip to
   **drifted** (amber), and the header badge must change to **N file(s) drifted**.
8. Click the drifted `AGENTS.md` row: a read-only **diff / two-pane compare** of the canonical
   (`CLAUDE.md`) vs `AGENTS.md` opens, showing the changed line. Confirm it reflects the real edit.
9. (Optional sanity) Revert the edit (`git checkout -- AGENTS.md`) + Refresh → the chip returns to
   **in sync** and the badge to green.

## 3. Create a context profile with two targets (incl. "Load from current file")

10. Open the **Profile manager** region. Confirm the store-hint line reads:
    *"Profiles live in `.bonsai/profiles.json` — commit it to share."*
11. Create a new profile: give it a **name** (e.g. `opus-rich`), optional description/model label.
12. Add a **target** → pick a single-file asset from the dropdown (only managed single-file
    descriptors appear: claude/agents/copilot/gemini/windsurf/cursorLegacy — NOT rules-dirs or
    `.mcp.json`). For the `claude` target, click **Load from current file** and confirm the textarea
    prefills with the current `CLAUDE.md` content.
13. Add a **second target** (e.g. `agents`) with different content (type a distinct body, or Load +
    edit). **Save** the profile. Confirm it appears in the profile list.
14. Try an invalid name (blank, or containing `/` or `\`) → confirm Save is rejected with a clear
    inline error, and the store is not corrupted.

## 4. Activation is gated behind a confirm + accurate diff preview, and writes real files

15. Click **Activate** on the `opus-rich` profile. A dialog opens that first loads a **per-target
    diff preview**: for each target it shows **current** (left) vs **proposed** (right). Confirm the
    diffs are accurate — a target whose file already matches shows no change; a target with new
    content shows the exact edit; a target for a not-yet-existing file is flagged as a **new file**.
16. Confirm the **"Activate & write files"** button is only enabled after the preview has loaded.
17. Click **Cancel**. Verify **nothing was written**: in the second terminal, `git status` shows no
    change from the activation (the files are untouched by Cancel).
18. Click **Activate** again → **Activate & write files**. A success toast appears
    (e.g. *"Activated 'opus-rich' — wrote N file(s)"*). Verify the real files on disk now hold the
    profile content:
    ```
    git -C D:\Temp\p24-scratch status
    type D:\Temp\p24-scratch\CLAUDE.md
    type D:\Temp\p24-scratch\AGENTS.md
    ```
    Confirm the mapped files match what you entered, and they show up as modified/untracked in
    Bonsai's normal **status panel** (activation does not stage or commit — you commit them through
    Bonsai's usual flow).
19. **Re-activate the same profile.** Confirm the toast now reads *"No changes — files already match
    the profile"* (info) and nothing is rewritten.

## 5. `.bonsai/profiles.json` is created on first save and is sensible, commit-able JSON

20. Confirm the store file exists and is human-readable / diffable:
    ```
    type D:\Temp\p24-scratch\.bonsai\profiles.json
    ```
    It must be pretty-printed JSON with `version: 1`, a `profiles` array containing `opus-rich` (name,
    optional description/model, targets with `assetId` + `content`), and `activeProfile: "opus-rich"`
    after step 18. Confirm it is reasonable to `git add` and commit (it is meant to be shared).
21. Confirm no `*.bonsai-tmp` files linger anywhere in the workdir (the atomic temp+rename cleaned up):
    ```
    dir /s /b D:\Temp\p24-scratch\*.bonsai-tmp
    ```
    should print nothing.

## 6. Drift semantics after a canonical-rewriting activation (expected, NOT a bug)

22. Note the intended semantics: activating a profile that rewrites the **canonical** (`CLAUDE.md`)
    will make other previously-in-sync files (e.g. `copilot-instructions.md`) show as **drifted** on
    the next Refresh — because they no longer match the new canonical. This is **correct**, not a
    bug: the panel is faithfully reporting that those files now differ from the new canonical. To
    bring them back in sync, add them as targets in the profile (or edit them) and re-activate.

## 7. (P24e) AI "Translate for <agent>" — consent-gated

This exercises the real `claude` CLI path; it requires the CLI installed and AI enabled + consented.

23. In **Settings**, with **AI disabled** (or consent not given): in the profile target editor the
    **"Translate for &lt;agent&gt;"** button must be **disabled** (or the action blocked) with a clear
    message pointing to Settings — it must NOT silently do nothing and must NOT call out to any CLI.
24. Enable AI + grant consent in Settings (and ensure the `claude` CLI is installed/on PATH). Back in
    the target editor, pick a source asset and click **"Translate for &lt;agent&gt;"**. Confirm it
    fills the target textarea with a sane, agent-flavored instruction file (same guidance, adapted
    tone/format). Confirm the helper **writes nothing on its own** — the proposed text only lands in
    the profile target you then Save/Activate.
25. (Optional) With AI enabled but the `claude` CLI missing / failing, confirm the action surfaces a
    clear error toast (aiUnavailable → info pointing to Settings; aiFailed → error) and changes
    nothing.

---

**Sign-off:** every numbered item behaves as described. In particular: drift chips track a real edit
(steps 7–8), Cancel in the activation dialog writes nothing while Confirm writes the exact byte
content to disk (steps 17–18), re-activation reports "no changes" (step 19), `.bonsai/profiles.json`
is created as commit-able JSON with no temp remnants (steps 20–21), the canonical-rewrite drift
behavior is understood as correct (step 22), and the AI translate button is hard-gated on the
consent flags with no silent CLI calls (steps 23–25). Report any deviation to the orchestrator with
the failing step number and the exact `git` / file output.
