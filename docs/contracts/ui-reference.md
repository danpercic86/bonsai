### 12.6 Identity in the header

- The identity control is the far-right item of `.header-toolbar` (§1) and reads the **effective**
  Git identity — `local` if set, otherwise `global` — and **names its source** in the menu. A
  local-only read is wrong: Git resolves local-over-global, so a repo with no local identity still
  commits fine, and a control that showed nothing there would be lying.
- Trigger: 32×32 button containing a 22px circle of **initials** (first letters of the first two
  name words, max 2). No identity → glyph `?` with a 1px `--warning` ring (**7.3:1** / **4.5:1**);
  the glyph and the accessible name, not the hue, carry the state. Loading → `·` + `aria-busy`.
  `aria-haspopup="menu"` + `aria-expanded`.
- Menu: a `ContextMenu` anchored with the house idiom (`rect.right`, `rect.bottom + 2`), a
  non-interactive `header` block stating name / email / source, then one row per saved identity with
  `checked` (⇒ `role="menuitemradio"`) and a `detail` second line, then `Manage identities…`.
- The menu owns its open state and lifts it via `onMenuOpenChange` (the `TabStrip.tsx:35-37`
  precedent) because App early-returns global shortcuts while a menu is open.
- **Writing an identity into a repo confirms only when it would overwrite a differing *local*
  value** — writing into an empty slot destroys nothing. The confirm names both identities and the
  consequence, uses `confirmVariant='primary'` (recoverable), and says
  `Commits you have already made are not changed.`
- P69 added **four** additive fields: `ContextMenuItem` gained `checked`, `detail` and `busy`, and
  `ContextMenuProps` gained `header` and `busy` (`ContextMenu.tsx:50` / `:72`). All are additive:
  absent ⇒ byte-identical rendering to before. The **check column belongs to the list, not the
  row** — it is reserved whenever any item declares `checked`, so plain rows in the same menu stay
  aligned with the labelled ones.

### 12.7 Forge accounts (P79/P80)

- **Provider display without color as sole carrier:** `ForgeProviderBadge` (2-letter monogram, GH /
  GL / BB / AZ / ??) + `ForgeAvatar` (image or login-initial monogram, 22px cozy / 20px compact).
  Both reuse `.identity-avatar` / `.pr-draft-tag` geometry; no hue carries meaning.
- **PR-panel account switcher (P80)** reuses the §12.6 identity-menu idiom exactly: a `ContextMenu`
  anchored `rect.right` / `rect.bottom + 2`, a non-interactive `header` block (`Accounts on {host}`,
  plus the no-default nudge line when applicable), one `checked` (`role="menuitemradio"`) row per
  account with a `detail` second line, then `Use host default` + `Add another account…`. The
  left group (avatar + login + host) is the trigger, shown as a button **only when the host has ≥2
  accounts** (no switcher chrome for a single account). Writes are optimistic + `busy`.
- **`AccountSource` label vocabulary** (canonical microcopy — do not reword per surface;
  `src/components/forgeAccountSource.ts`):

  | `accountSource` | header caption | tooltip |
  |---|---|---|
  | `override`    | `Pinned to this repo` | `Pinned to this repository. Other repositories on this host use the default.` |
  | `ownerMatch`  | `Matched by owner`    | `Chosen because its username matches this repository's owner.` |
  | `hostDefault` | `Host default`        | `The default account for this host.` |
  | `single`      | *(none)*              | *(none)* |
  | `none`        | *(none)*              | *(n/a — connect view shows)* |

- **Per-repo vs global semantics (P80):** the PR panel is **per-repo** — it pins/unpins the repo's
  account override (`Reset to host default` is nondestructive, no confirm) and never signs out.
  **Full sign-out (keychain token deletion via `forgeRemoveAccount`) lives only in Settings →
  Accounts**, always behind a danger `ConfirmDialog` that names the account and states pinned repos
  fall back. Never label a per-repo unpin "Disconnect" (it reads as sign-out).
- Connected-state chip: `● Connected` (`--success` dot, `--text-2` word) / `○ Token missing`
  (`--text-3` dot) — the word carries meaning, never the dot color alone.
- No new tokens were introduced for any forge-account surface; all classes are built from existing
  `--bg/-text/-border/-accent/-success/-warning/-danger` tokens.
