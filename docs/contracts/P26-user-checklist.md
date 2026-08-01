# P26 — USER CHECKPOINT checklist (agent-assets manager: skills / subagents / slash commands)

Run these in the **native app** (`pnpm tauri dev`) against a **scratch / throwaway repo you create**
— NOT a real repository you care about. **Deleting a skill is a recursive directory removal**
(`remove_dir_all` of `.claude/skills/<name>/`), and every save/delete writes real files into the
workdir. These are exactly the items the AI gate could NOT self-verify: the native AI Assets panel
driven by a real on-disk `.claude/` tree, the validation chips, the create/edit form writing real
files, the complex-frontmatter read-only guard, and the skill delete-confirm naming a whole
directory.

Keep a **second terminal** open in the scratch repo for the disk / `git` verifications below.

The agent-assets surface lives in the **AI Assets** panel, opened from the **🤖 button in the top
header** (`title` / `aria-label` = "AI Assets"); it is only shown when a repo is open. The new
**"Agent assets"** section sits below the profiles section.

---

## 0. Prepare a scratch repo with a real `.claude/` bundle (throwaway — safe to delete)

In a terminal, in a throwaway folder under `D:\Temp` (never C:). Adjust the `printf` calls for your
shell — the point is a `.claude/` tree with one of each kind, one deliberately-broken agent, and one
agent with **complex (multi-line YAML) frontmatter**:

```
cd /d D:\Temp
git init -b main p26-scratch
cd p26-scratch
git config user.name "P26 Tester" && git config user.email "p26@example.com"
git config core.autocrlf false

REM --- a valid skill (owns its directory) with a supporting file ---
mkdir -p .claude\skills\code-review
printf -- "---\nname: code-review\ndescription: Reviews a diff for issues\n---\n\n# Code review\n\nReview the changes.\n" > .claude\skills\code-review\SKILL.md
printf "print('helper')\n" > .claude\skills\code-review\helper.py

REM --- a valid subagent ---
mkdir -p .claude\agents
printf -- "---\nname: test-runner\ndescription: Runs the test suite\ntools: Bash\nmodel: inherit\n---\n\nYou run tests.\n" > .claude\agents\test-runner.md

REM --- a DELIBERATELY BROKEN subagent (missing required 'description') ---
printf -- "---\nname: broken\n---\n\nno description here\n" > .claude\agents\broken.md

REM --- a subagent with COMPLEX (multi-line YAML) frontmatter -> read-only ---
printf -- "---\nname: fancy\ndescription: has a list\ntools:\n  - Read\n  - Bash\n---\n\nSystem prompt.\n" > .claude\agents\fancy.md

REM --- a valid slash command ---
mkdir -p .claude\commands
printf -- "---\ndescription: Update the changelog\nargument-hint: <version>\n---\n\nUpdate the changelog for $ARGUMENTS.\n" > .claude\commands\changelog.md

git add -A && git commit -m "seed .claude bundle"
```

Open **`D:\Temp\p26-scratch`** in Bonsai.

---

## 1. The Agent-assets section lists all three kinds with correct chips

1. Click the **🤖 "AI Assets"** button in the top header. Scroll to the **"Agent assets"** section
   (sub-header note: *"Managed `.claude/` skills, subagents, and slash commands — parsed and
   validated. Create, edit, or delete them below."*).
2. Confirm **three groups** — **Skills**, **Subagents**, **Slash commands** — each listing its
   members with the file path in `mono`:
   - **Skills:** `code-review` → green **valid** chip.
   - **Subagents:** `broken` → amber **N issue(s)** chip; `fancy` → **complex — read-only** chip;
     `test-runner` → green **valid** chip.
   - **Slash commands:** `changelog` → green **valid** chip.
3. Hover the amber `broken` row's chip → the tooltip shows the first issue
   (*"agent requires frontmatter field 'description'"* or similar).
4. Cross-check reality: the chips match the files you wrote — `broken.md` has no `description`,
   `fancy.md` has the YAML block list, the other three are complete.

## 2. Create a new subagent via the editor — templates prefill, Save writes the real file

5. Click **New subagent** in the Subagents group. The editor opens in **create mode**, prefilled
   from the agent template: a **name** input (editable), known frontmatter fields (`name`,
   `description`, `tools`, `model` — `model` seeded `inherit`), and a **System prompt** textarea.
6. Enter name `docs-writer`, a `description` (e.g. *"Writes documentation"*), leave/adjust the
   others, and type a short system prompt. Click **Save**.
7. Confirm a success toast (*"Saved agent 'docs-writer'"*). The Subagents group now lists
   `docs-writer` with a green **valid** chip. In the second terminal, verify the real file:
   ```
   type D:\Temp\p26-scratch\.claude\agents\docs-writer.md
   ```
   It must be a fenced `---` frontmatter block with your fields + the body. Confirm it also shows as
   **untracked** in Bonsai's normal **status panel** (save does not stage or commit).
8. **Save with a missing required field still writes, but flags it.** Create another subagent
   `no-desc` with a `name` but a **blank `description`**, and Save. Confirm it saves (an info toast
   about *"Saved with N issue(s)"*), the file appears on disk, and its row shows the amber
   **N issue(s)** chip. (Repeat the create flow for a **skill** and a **command** if you like —
   the skill create must produce `.claude/skills/<name>/SKILL.md`, creating the directory.)

## 3. Edit preserves unknown / preserved frontmatter keys

9. Open the existing `test-runner` subagent. Note the known fields populate the inputs. Edit its
   `description` to something new and **Save**.
10. Verify on disk the edit landed and the file is intact:
    ```
    type D:\Temp\p26-scratch\.claude\agents\test-runner.md
    ```
11. (Preserved-keys check) In the second terminal add an **unknown key** to a flat agent and confirm
    an edit round-trip keeps it. For example append `color: blue` under `test-runner`'s frontmatter
    on disk, Refresh the panel, open `test-runner`, change the `description`, Save — then
    `type` the file again and confirm **`color: blue` is still present** (unknown keys survive the
    round-trip; the editor only surfaces the known fields but preserves the rest).

## 4. A complex-frontmatter asset opens READ-ONLY and cannot be overwritten

12. Click the `fancy` subagent (the **complex — read-only** chip). The editor opens with a banner:
    *"This asset has complex YAML frontmatter Bonsai can't safely edit yet — edit it in your
    editor…"*. Confirm the frontmatter inputs and body are **disabled** and the **Save** button is
    **disabled** (tooltip *"Complex frontmatter is read-only"*).
13. Confirm the backend refuses even a forced overwrite: the on-disk `fancy.md` is unchanged. (The
    backend re-guard returns an error and writes nothing if any path ever attempted a flat overwrite
    of complex YAML — verified by automated test; here just confirm the UI never lets you Save and
    the file stays byte-identical.)
    ```
    type D:\Temp\p26-scratch\.claude\agents\fancy.md
    ```

## 5. Deleting a skill confirms — and removes the WHOLE directory

14. Trigger **Delete** on the `code-review` **skill** (row action or the editor's delete). A confirm
    dialog opens whose text names the **whole directory**: it warns it removes the
    `.claude/skills/code-review/` directory **and every file inside it (SKILL.md plus any supporting
    scripts, templates, or references)**.
15. Click **Cancel**. Verify **nothing was removed**:
    ```
    dir D:\Temp\p26-scratch\.claude\skills\code-review
    ```
    both `SKILL.md` and `helper.py` are still present.
16. Trigger Delete again → **confirm**. A success toast appears and the `code-review` skill
    disappears from the list. Verify the **entire directory** is gone (including `helper.py`):
    ```
    dir D:\Temp\p26-scratch\.claude\skills\code-review
    ```
    must report the path does not exist. Confirm the other assets (`test-runner`, `fancy`,
    `changelog`, plus anything you created) are untouched.
17. (Agent/command delete is single-file.) Delete a **subagent** (e.g. `no-desc`) → confirm only its
    `.md` is removed and the `.claude/agents/` directory (with the other agents) remains.

## 6. No temp remnants

18. Confirm the atomic temp+rename left no stray temp files anywhere in the workdir:
    ```
    dir /s /b D:\Temp\p26-scratch\*.bonsai-tmp
    ```
    should print nothing.

---

**Sign-off:** every numbered item behaves as described. In particular: the three groups list every
kind with correct **valid / N-issue / complex-read-only** chips against the real files (§1); create
prefills a template and Save writes a real, valid file, while a missing required field still saves
but is flagged (§2); an edit preserves unknown frontmatter keys through the round-trip (§3); a
complex-YAML asset is read-only with Save disabled and its bytes never change (§4); the skill
delete-confirm names the whole `.claude/skills/<name>/` directory, Cancel removes nothing, and
confirm removes the entire directory including supporting files while agent/command delete removes
only the single `.md` (§5); and no `*.bonsai-tmp` remnants linger (§6). Test on a scratch repo only.
Report any deviation to the orchestrator with the failing step number and the exact `dir` / `type`
output.
