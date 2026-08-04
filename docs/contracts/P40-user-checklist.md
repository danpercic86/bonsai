# P40 — Git Config Editing — USER CHECKPOINT checklist

These steps require the **native Tauri window** and human perception, so the orchestrator
cannot self-verify them (per CLAUDE.md gate rules). Run them yourself with
`pnpm tauri dev` and confirm each box.

## WARNING — Global edits touch your real `~/.gitconfig`

Steps (c) and any Global-level write below modify your **real user config**
(`C:\Users\<you>\.gitconfig`). Use a **throwaway value** you are happy to remove, note the
original value first (`git config --global --get <key>`), and restore/unset it after the check.
All other steps operate on a **scratch repo only**.

## Setup — build a scratch repo

Do NOT use a real project. In a terminal:

```
mkdir D:\Temp\bonsai-scratch\p40-manual
cd D:\Temp\bonsai-scratch\p40-manual
git init -b main
git config --local user.name  "Scratch Local"
git config --local user.email "scratch@local.test"
echo hello > a.txt
git add a.txt
git commit -m "seed"
```

Launch Bonsai (`pnpm tauri dev`) and **Open** `D:\Temp\bonsai-scratch\p40-manual`.

---

## (a) Settings → Git config renders with real values + Local/Global toggle

- [ ] Open **Settings**; find the **Git config** section (below Appearance).
- [ ] It shows three groups: **Identity** (`user.name`, `user.email`), **Behaviour**
      (`core.autocrlf`, `init.defaultBranch`, `pull.ff`, `pull.rebase`), and **Advanced** (list).
- [ ] A segmented **Local | Global** toggle is present; **Local** is selected by default.
- [ ] On **Local**, Identity shows `Scratch Local` / `scratch@local.test` (the values you set).
- [ ] Switch to **Global**: the fields re-fetch and show your real user-wide values (or blanks
      with an `inherited from …` hint where a key is only set at another level). Switch back to **Local**.

## (b) Set identity at Local; inherited hint clears

- [ ] With **Local** selected, change `user.name` to `Local Edited` and blur/Enter; change
      `user.email` to `edited@local.test`.
- [ ] In a terminal: `git config --local user.name` → `Local Edited`;
      `git config --local user.email` → `edited@local.test`.
- [ ] The muted `inherited from …` hint under those fields is **absent** (the value is now set
      at Local, not inherited).

## (c) Set a value at Global; cross-check `git config --global`  (edits your real ~/.gitconfig)

- [ ] First note the original: `git config --global --get core.autocrlf` (may be empty).
- [ ] Switch the toggle to **Global**. Set `core.autocrlf` to a throwaway enum value (e.g. `input`).
- [ ] In a terminal: `git config --global --get core.autocrlf` → `input`.
- [ ] Restore it afterwards: set it back to the original in-app, or
      `git config --global --unset core.autocrlf` if it was previously empty.

## (d) Add / edit / remove an Advanced key; cross-check `git config`

- [ ] Back on **Local**, in **Advanced** use the "Add entry" row: key `alias.co`, value `checkout`; save.
- [ ] Terminal: `git config --local --get alias.co` → `checkout`.
- [ ] Edit that row's value to `commit`; save. Terminal: `git config --local --get alias.co` → `commit`.
- [ ] Remove the row. Terminal: `git config --local --get alias.co` exits non-zero (unset).

## (e) Identity gap: commit blocked → "Set identity…" unblocks it

Build a second scratch repo with **no identity** to reproduce the gap:

```
mkdir D:\Temp\bonsai-scratch\p40-noid
cd D:\Temp\bonsai-scratch\p40-noid
git init -b main
git config --local user.useConfigOnly true   & rem forces "no identity" even if global is set
echo hi > b.txt & git add b.txt
```

Open `p40-noid` in Bonsai. (If your global identity still satisfies the commit, temporarily
unset it or rely on `user.useConfigOnly`.)

- [ ] Stage `b.txt`, type a message, click **Commit** → it **fails** with a
      "Set your Git identity: …" banner.
- [ ] The banner shows a **"Set identity…"** button; click it → **Settings** opens with the
      **Git config** section scrolled/focused on **Identity**.
- [ ] Set `user.name` and `user.email` at **Local**, close Settings, and **Commit** again → it **succeeds**.
- [ ] Cleanup: remove `user.useConfigOnly` from the scratch repo if you set it.

## (f) Invalid key rejected inline

- [ ] On **Local → Advanced**, add an entry with a malformed key (e.g. `nosection`, or an empty key).
- [ ] The save is **rejected with an inline error** (invalid name) and nothing is written
      (`git config --local --list` shows no such entry).

---

## After you finish — restore your real config

- [ ] Undo/unset any **Global** value you changed in step (c) so your real `~/.gitconfig` is back
      to its original state (`git config --global --get <key>` to confirm).
- [ ] Delete the scratch repos under `D:\Temp\bonsai-scratch\` when done.
