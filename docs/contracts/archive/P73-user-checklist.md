# P73 — USER CHECKPOINT checklist

Everything below needs the native Tauri window or human perception, so the
orchestrator cannot pass it. The AI gate is green (see `TODO.md` P73).

Run `pnpm tauri dev`, then open **`D:\Repos\ham-digi-backend`** — the repo where you
reported both defects (submodule `src/Hamilton.Voyager.Protocol/protocol`).

> **If you already ran the workaround** (`git submodule update --init -- src/Hamilton.Voyager.Protocol/protocol`)
> the submodule is now checked out, so item 1 will show "up to date" instead of
> "not checked out" and there is no wedge left to repair. To re-create the exact
> wedged state: delete `src/Hamilton.Voyager.Protocol/protocol/.git` and everything
> else inside that folder, keeping the (now empty) folder and
> `.git/modules/src/Hamilton.Voyager.Protocol/protocol` intact.

## 1. The badge no longer lies
- [ ] Sidebar → **Submodules** → the row reads **`not checked out`** (it used to say
      "not initialized"). Hovering it shows: *No files on disk yet. Right-click the
      row → Initialize and check out.*

## 2. The reported Init defect is gone
- [ ] Right-click the row. The item is **`Initialize and check out`** (not "Init"),
      and **`Update` is greyed out** while the row is not checked out.
- [ ] Click it. While it runs the row shows a **`checking out…`** pill.
- [ ] It succeeds with a toast **`Checked out src/Hamilton.Voyager.Protocol/protocol`**
      and the badge becomes **`up to date`** — the toast and the badge now agree,
      which is the whole complaint.
- [ ] **No credential prompt and no network wait** — the cached data under
      `.git/modules` is reused. This is the part that is impossible to fake.
- [ ] The folder really has files: `src/Hamilton.Voyager.Protocol/protocol` now
      contains `README.md`, `envelope.proto`, `robotics/`, `storage/`, and the
      `document.proto` you had open.

## 3. The reported Update error is gone
- [ ] On an already-checked-out row, `Update` is enabled and `Initialize and check
      out` is greyed out (they are mutually exclusive).
- [ ] Nothing anywhere says **`attempt to reinitialize`**. That message should now
      be unreachable; if you ever see it again, that is a bug — report the exact text.

## 4. Refusals are readable (only if you want to force them)
- [ ] Re-create the wedge but leave one stray file in the folder, then Initialize:
      the toast should read *Couldn't check out … The folder already has files in it.
      Move or delete everything inside '…', then try again.* — and your stray file
      must still be there afterwards, byte-for-byte.

## 5. Both themes
- [ ] Toggle light/dark: the five badge states stay legible and all pills are the
      same height in the list (no stepping between rows).

## Notes for the reporter
- Deinitialize/Remove still ask for confirmation first; nothing about those changed.
- A submodule whose `.git/modules` folder is *corrupt* (not just detached) now gets an
  explicit "leftover data … delete the folder" message instead of the raw libgit2 text.
  That is the one message that names an internal path, because deleting it is the only
  remedy — Bonsai deliberately never deletes it for you.
