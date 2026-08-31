# P11 — Feature follow-up batch

> Contract for senior-dev. Implement strictly to the signatures and sequences below.
> Rust owns ALL git logic AND graph layout math; React only renders. IPC carries compact,
> already-computed data. `src/ipc/mock.ts` MUST keep compiling and mirror every wire change.
> Prior art: `docs/contracts/M5-branches.md` (command/`_inner` pattern, ConfirmDialog),
> `docs/contracts/M6-remotes.md` (mutation command shape, notices), `docs/contracts/P9-stash-management.md`,
> `docs/contracts/P10-stash-as-node.md` (menu icons, `ContextMenuItem.icon`), P2/P3b (settings surface).
>
> Plan of record: `this-is-a-folloup-structured-milner.md` (approved). This contract covers
> **P11b, P11c, P11d, P11f, P11g** in full and gives **P11e** a short spec. **P11a** (Sidebar tags
> collapsed-by-default, one-liner) is already done and out of scope.

## §0 Scope, locked decisions, invariants

- Scrollable all-files diff (P11g) applies to **BOTH** Compare-with-HEAD **AND** single-commit
  (vs first-parent) diffs.
- Auto-fetch: **OFF by default**, **active tab only**, default interval **5 min**, configurable in
  Settings.
- Settings page exposes all four knob groups: auto-fetch (interval + toggle); commit node & avatar
  sizes; row height & lane spacing; theme & list view.
- No new Rust git logic for P11g (lazy per-file hunks reuse existing commands — §5 decides & justifies).
- Suggested build order: **P11f** (self-contained) → **P11b → P11c → P11d → P11e** (settings chain)
  → **P11g** (largest). Each sub-increment compiles and is committable on its own.

---

## §1 P11f — "Create branch here"

Create a local branch at an arbitrary commit, carrying uncommitted work across via auto-stash.

### 1.1 Rust core — `src-tauri/src/git/branches.rs`

```rust
/// Result of `create_branch_here`. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchHereResult {
    /// true when uncommitted work was auto-stashed and carried across.
    pub stashed: bool,
    /// Present only when `stashed`; the outcome of re-applying the stash on the
    /// new branch (`Applied` = clean carry-over, `Conflicts{paths}` = carried
    /// with markers, stash retained). `None` when the worktree was clean.
    pub apply: Option<crate::git::stash::ApplyStashOutcome>,
}
```

```rust
/// Blocking. Create local branch `name` at commit `oid`, then check it out,
/// carrying any uncommitted work across via auto-stash. Composes existing
/// primitives; NEVER lossy (working changes are recovered on every failure path).
pub fn create_branch_here(
    workdir: &Path,
    name: &str,
    oid: &str,
) -> Result<CreateBranchHereResult, AppError>;
```

**Exact ordered algorithm (safety/rollback is the point):**

1. **Validate & resolve FIRST — zero side effects on failure.**
   - `validate_branch_name(name)?` (reuses the private @178 helper → `InvalidName`).
   - `let repo = open_repo_at(workdir)?`.
   - `let target_oid = git2::Oid::from_str(oid)` → on `Err` return
     `AppError::Git(format!("cannot create branch: '{oid}' is not a valid commit id"))`.
   - `let target = repo.find_commit(target_oid)` → on `Err` (incl. `NotFound`) return
     `AppError::Git(format!("cannot create branch: commit '{oid}' not found"))`.
     (**Decision:** reuse `AppError::Git` for a bad/unknown oid — no new error variant. Flagged §8.)
2. **Pre-check branch existence BEFORE any side effect** (so `BranchExists` never strands a stash):
   `if repo.find_branch(name, git2::BranchType::Local).is_ok() { return Err(AppError::BranchExists(format!("branch '{name}' already exists"))); }`
3. **Auto-stash (dirty-vs-clean decision).** Call
   `let stashed = stash::create_stash(workdir, None, /* include_untracked */ true)?.created;`
   - **Decision — reuse `create_stash`'s own dirtiness detection** rather than a separate status
     scan: `create_stash` already returns `created:false` on a clean worktree (libgit2 `GIT_ENOTFOUND`
     → "nothing to stash"), and its `require_clean` gate errors `OperationInProgress` mid-merge/rebase
     — exactly the correct guard for this operation. So `stashed == false` means the worktree was
     clean (nothing carried); `stashed == true` means work was stashed and must be re-applied.
     `configMissing` can surface here (stash authors a commit → needs signature); let it propagate.
4. **Create the branch ref** at the resolved commit: `repo.branch(name, &target, /* force */ false)`.
   On `Err`: **if `stashed`, best-effort `let _ = stash::pop_stash(workdir, 0);` to restore the
   working changes onto the original branch**, then return the mapped error (`Exists` →
   `BranchExists`, else `e.into()`). (The pre-check in step 2 makes `Exists` a race-only backstop.)
5. **SAFE checkout** the new branch: `checkout_branch(workdir, name)?` (@226; runs `checkout_tree`
   before `set_head`, so a conflict leaves worktree + HEAD untouched). On `Err`:
   **roll back so nothing is stranded** — best-effort delete the just-created ref
   (`let _ = delete_branch(workdir, name);`) and, **if `stashed`, best-effort
   `let _ = stash::pop_stash(workdir, 0);`** to restore work onto the original branch, then return
   the checkout error. (Post-stash the worktree is clean, so this path is defensive.)
6. **Re-apply the carried work** iff stashed: `if stashed { let outcome = stash::pop_stash(workdir, 0)?; return Ok(CreateBranchHereResult { stashed: true, apply: Some(outcome) }); }`.
   `pop_stash` drops on clean apply and RETAINS on conflict (never lossy — P9 guarantee). A
   `Conflicts` outcome is a **success return**, not an error (the branch was created & checked out;
   the changes are present with markers).
7. **Clean case:** `Ok(CreateBranchHereResult { stashed: false, apply: None })`.

**Error kinds (enumerated):** `invalidName` (bad name), `branchExists` (name taken),
`operationInProgress` (mid-merge/rebase, via `create_stash`), `configMissing` (unset identity, via
`create_stash`), `checkoutConflict` (defensive, via `checkout_branch`), `git` (bad/unknown oid, or
any other libgit2 error), `noRepo` (command layer). Rollback (pop/delete) is best-effort and never
masks or replaces the originating error; if a best-effort pop itself conflicts, the stash simply
stays on the stack for manual recovery — the working changes are never lost.

### 1.2 Command — `src-tauri/src/commands.rs` + `src-tauri/src/lib.rs`

Standard thin → `_inner` → `spawn_blocking` shape (mirror `create_branch`, §651). Does NOT emit
`repo-changed` (frontend refetches imperatively).

```rust
/// Creates local branch `name` at commit `oid`, auto-stashing/​re-applying
/// uncommitted work across the checkout (P11 §1). Errors: invalidName |
/// branchExists | operationInProgress | configMissing | checkoutConflict | git
/// | noRepo. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn create_branch_here(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    oid: String,
) -> Result<CreateBranchHereResult, AppError>;

async fn create_branch_here_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    oid: String,
) -> Result<CreateBranchHereResult, AppError>; // repo_path(state, repo_id)? → spawn_blocking(|| branches::create_branch_here(&path, &name, &oid))
```

Register `create_branch_here` in the `generate_handler!` list in `lib.rs` (add to the branches block).

### 1.3 IPC — types.ts / tauri.ts / mock.ts

`src/ipc/types.ts`:

```ts
export interface CreateBranchHereResult {
  stashed: boolean;
  /** Present only when `stashed`; null otherwise. */
  apply: ApplyStashOutcome | null;
}
```

Add to `IpcApi`:

```ts
  /** Create local branch `name` at commit `oid`, auto-stashing/​re-applying
   *  uncommitted work across the checkout. Rejects invalidName | branchExists
   *  | operationInProgress | configMissing | checkoutConflict | git | noRepo. */
  createBranchHere(repoId: string, name: string, oid: string): Promise<CreateBranchHereResult>;
```

(Note: `Option<ApplyStashOutcome>` serializes to `apply: {kind:...} | null` — declare the TS field
as `ApplyStashOutcome | null`, matching serde's `None → null`.)

`src/ipc/tauri.ts`:

```ts
createBranchHere: (repoId, name, oid) =>
  invoke<CreateBranchHereResult>('create_branch_here', { repoId, name, oid }),
```

`src/ipc/mock.ts` — stateful, must reflect visually via the P10 stash-node rebuild path:

```
async createBranchHere(repoId, name, oid): Promise<CreateBranchHereResult>
  await delay(250)
  if (name is invalid — trim empty / starts with '-' / already in mockBranches.local) →
      throw { kind: name-taken ? 'branchExists' : 'invalidName', message: ... }
  const dirty = mockStatus has any staged/unstaged/untracked/conflicted entry
  // add the new branch to mockBranches.local at oid (isHead:true; unset previous head's isHead),
  // set mockHeadBranch = name, mockHeadOid = oid  (so the graph HEAD pill moves)
  if (!dirty) return { stashed: false, apply: null }
  // simulate carrying work across:
  const conflict = new URLSearchParams(location.search).get('branch') === 'cbhconflict'
  if (conflict) {
      // leave mockStatus dirty + push a synthetic conflicted entry; DO NOT clear the stash
      return { stashed: true, apply: { kind: 'conflicts', paths: ['src/app.ts'] } }
  }
  // clean carry-over: mockStatus is preserved as-is on the new branch (changes moved with us)
  return { stashed: true, apply: { kind: 'applied' } }
```

- Mock keeps compiling and does not need to create a real stash entry (the changes are shown as
  moving with the checkout); the `?branch=cbhconflict` trigger exercises the Conflicts toast.
- `getGraph` already rebuilds from live state (P10 §3.3) — the moved HEAD pill and new branch pill
  appear on the next `refreshAll`.

### 1.4 Frontend — icon, menu items, PromptDialog, handler

**`src/components/menuIcons.tsx` — new `BranchIcon`** (16×16, same `svgProps` as the P10 set):
a git-branch glyph — a straight trunk line with a fork branching off to a dot.

```tsx
/** Create branch here — a trunk with a fork branching to a new dot. */
export function BranchIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="4.5" cy="3" r="1.5" />
      <circle cx="4.5" cy="13" r="1.5" />
      <circle cx="11.5" cy="6.5" r="1.5" />
      <path d="M4.5 4.5 V11.5" />
      <path d="M4.5 8 C4.5 6 7 6.5 10 6.5" />
    </svg>
  );
}
```

**`src/components/RepoWorkspace.tsx`:**

1. Import `BranchIcon`. Import a new `PromptDialog` (below).
2. New state: `const [pendingCreateBranch, setPendingCreateBranch] = useState<{ oid: string } | null>(null);`
3. `branchMenuItems(name, kind)` (@1147): insert immediately AFTER the `Checkout` item (before
   `Copy branch name`), gated on `gate` (`mutating || opActive`), using the resolved `tip`:
   ```ts
   {
     label: 'Create branch here',
     icon: <BranchIcon />,
     disabled: gate,
     onSelect: () => setPendingCreateBranch({ oid: tip }),
   },
   ```
   (Applies to BOTH `localBranch` and `remoteBranch` menus — `tip` is already resolved for both.)
4. `buildContextItems(target)` commit branch (@1269, the "Compare with HEAD" list): PREPEND a
   `Create branch here` item (icon `<BranchIcon />`, `disabled: gate`, `onSelect: () =>
   setPendingCreateBranch({ oid: target.oid })`). Keep it available even when HEAD is unborn IS
   fine for creating a first branch? No — creating at a commit requires a commit; the commit-row
   menu only exists when there are commits, so no extra guard is needed. Gate on `gate`.
5. New handler:
   ```ts
   async function handleCreateBranchHere(oid: string, name: string): Promise<void>;
   // setMutating(true); try { const res = await ipc.createBranchHere(repoId, name, oid);
   //   await refreshAll();  // the existing full-refresh batch (~569)
   //   if (!res.stashed)                         pushToast('success', `Created and checked out ${name}`);
   //   else if (res.apply?.kind === 'applied')   pushToast('success', `Created ${name} and carried your changes over`);
   //   else /* conflicts */                      pushToast('warning',
   //       `Created ${name}; your changes were carried over with conflicts — resolve them in the status panel`);
   // } catch (e) { pushToast('error', errorMessage(e)); }
   // finally { setMutating(false); setPendingCreateBranch(null); }
   ```
6. Render `PromptDialog` near the other dialogs (ConfirmDialog siblings), driven by
   `pendingCreateBranch`:
   ```tsx
   <PromptDialog
     open={pendingCreateBranch !== null}
     title="Create branch here"
     label="Branch name"
     placeholder="feature/my-branch"
     confirmLabel="Create branch"
     busy={mutating}
     validate={(v) => {
       const t = v.trim();
       if (t === '' || t.startsWith('-')) return 'Enter a valid branch name';
       if (branches?.local.some((b) => b.name === t) === true) return 'A branch with that name already exists';
       return null;
     }}
     onSubmit={(v) => void handleCreateBranchHere(pendingCreateBranch!.oid, v.trim())}
     onCancel={() => setPendingCreateBranch(null)}
   />
   ```

**`src/components/PromptDialog.tsx` — new reusable component** (modeled on `ConfirmDialog.tsx`):

```ts
export interface PromptDialogProps {
  open: boolean;
  title: string;
  /** Label above the text input. */
  label: string;
  placeholder?: string;
  initialValue?: string;
  confirmLabel: string;
  busy: boolean;
  /** Return an error string to block submit, or null when valid. Re-run on every
   *  keystroke; the error renders under the input and disables the confirm button. */
  validate?(value: string): string | null;
  onSubmit(value: string): void;
  onCancel(): void;
}
export function PromptDialog(props: PromptDialogProps): JSX.Element | null;
```

Behavior (mirror ConfirmDialog idioms):
- Reuses `.dialog-overlay` / `.dialog-card` / `.dialog-title` / `.dialog-body` / `.dialog-buttons`.
- Local `value` state, seeded from `initialValue ?? ''` when `open` flips true.
- **Initial focus lands on the INPUT** (select its text); `useEffect([open])`.
- Esc cancels via a **capture-phase** window listener with `stopPropagation` (same as ConfirmDialog,
  so App's global Esc-deselect does not also fire). Overlay click cancels.
- **Enter submits** (form `onSubmit`, `preventDefault`) — only when `validate` returns null and not
  `busy`. Unlike ConfirmDialog (Cancel-focused so stray Enter is safe), a prompt's whole purpose is
  to submit text, so Enter-to-submit is correct here.
- Confirm button uses `.btn-primary` (this is a create action, not destructive), `disabled` when
  `busy` or the current value is invalid. Cancel button `.btn-secondary`.
- Renders the validation error in a `<p className="dialog-error">` under the input when non-null.
- Returns `null` when `!open`.

### 1.5 P11f acceptance criteria (scratch repo + harness)

- Rust: on a scratch repo with commits `C0..C2` on `main`, dirty worktree, `create_branch_here(dir,
  "feat", <C0 oid>)` → `{ stashed:true, apply:{kind:"applied"} }`; HEAD is `feat` at `C0`;
  `git status --porcelain` shows the carried changes; the stash stack is empty (clean pop dropped).
- Rust: clean worktree → `{ stashed:false, apply:null }`; HEAD is the new branch at `oid`.
- Rust: name already exists → `Err(BranchExists)` and **NOTHING stashed** (stash stack unchanged),
  HEAD unchanged.
- Rust: bad/unknown oid → `Err(Git)` before any side effect.
- Rust: conflict carry-over case (dirty change to a file that differs at the target commit) →
  `{ stashed:true, apply:{kind:"conflicts", paths:[...]} }`; index has conflicts; stash RETAINED.
- Rust: mid-merge/rebase → `Err(OperationInProgress)`, no branch created.
- Harness: right-click a branch pill/sidebar row AND a commit row → "Create branch here" item with
  `<BranchIcon />` present, gated while busy; selecting opens `PromptDialog`; submitting a valid
  name toasts the correct Applied/Conflicts/clean wording; `?branch=cbhconflict` shows the warning
  toast. `tsc`/`pnpm build` clean; mock compiles.

---

## §2 P11b — Settings model + IPC plumbing (no UI)

Extend the settings surface end-to-end. No new command; reuse `get_ui_settings` / `set_ui_settings`.

### 2.1 Rust — `src-tauri/src/settings.rs`

Add two `#[serde(default)]` nested structs with `Default` impls matching current `METRICS`, plus
clamp helpers + range consts (mirror `clamp_pane_widths`):

```rust
/// Auto-fetch preference (P11). OFF by default; interval in minutes.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoFetch {
    pub enabled: bool,
    pub interval_minutes: u32,
}
impl Default for AutoFetch {
    fn default() -> Self { AutoFetch { enabled: false, interval_minutes: 5 } }
}

/// Graph geometry knobs (P11). Defaults EQUAL the frontend METRICS defaults
/// (dot 4 / avatar 10 / row 32 / lane 16) — the "no override" baseline.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphPrefs {
    pub dot_radius: u32,
    pub avatar_radius: u32,
    pub row_height: u32,
    pub lane_width: u32,
}
impl Default for GraphPrefs {
    fn default() -> Self { GraphPrefs { dot_radius: 4, avatar_radius: 10, row_height: 32, lane_width: 16 } }
}

pub const AUTO_FETCH_INTERVAL_MIN: u32 = 1;
pub const AUTO_FETCH_INTERVAL_MAX: u32 = 120;
pub const DOT_RADIUS_MIN: u32 = 2;   pub const DOT_RADIUS_MAX: u32 = 10;
pub const AVATAR_RADIUS_MIN: u32 = 6;  pub const AVATAR_RADIUS_MAX: u32 = 16;
pub const ROW_HEIGHT_MIN: u32 = 24;  pub const ROW_HEIGHT_MAX: u32 = 48;
pub const LANE_WIDTH_MIN: u32 = 10;  pub const LANE_WIDTH_MAX: u32 = 28;

pub fn clamp_auto_fetch(a: AutoFetch) -> AutoFetch {
    AutoFetch { enabled: a.enabled,
        interval_minutes: a.interval_minutes.clamp(AUTO_FETCH_INTERVAL_MIN, AUTO_FETCH_INTERVAL_MAX) }
}
pub fn clamp_graph_prefs(g: GraphPrefs) -> GraphPrefs {
    GraphPrefs {
        dot_radius: g.dot_radius.clamp(DOT_RADIUS_MIN, DOT_RADIUS_MAX),
        avatar_radius: g.avatar_radius.clamp(AVATAR_RADIUS_MIN, AVATAR_RADIUS_MAX),
        row_height: g.row_height.clamp(ROW_HEIGHT_MIN, ROW_HEIGHT_MAX),
        lane_width: g.lane_width.clamp(LANE_WIDTH_MIN, LANE_WIDTH_MAX),
    }
}
```

Add both fields to `Settings` (additive, `#[serde(default)]` via container-level `default`):

```rust
pub auto_fetch: AutoFetch,
pub graph: GraphPrefs,
```

...and to `Default for Settings` (`auto_fetch: AutoFetch::default(), graph: GraphPrefs::default()`).
`SETTINGS_VERSION` stays `1` (forward-compatible additive fields — a legacy file loads with
defaults). In `load_from`, ALSO clamp the two new structs on read (defend a hand-edited file):
`s.auto_fetch = clamp_auto_fetch(s.auto_fetch); s.graph = clamp_graph_prefs(s.graph);`.

### 2.2 Rust — `src-tauri/src/commands.rs`

Extend `UiSettings`, `UiSettingsPatch`, `apply_patch`, and both getter/setter mappings:

```rust
pub struct UiSettings {
    pub theme: ThemeChoice,
    pub pane_widths: PaneWidths,
    pub list_view: ListView,
    pub auto_fetch: AutoFetch,   // NEW
    pub graph: GraphPrefs,       // NEW
}

pub struct UiSettingsPatch {
    pub theme: Option<ThemeChoice>,
    pub pane_widths: Option<PaneWidths>,
    pub list_view: Option<ListView>,
    pub auto_fetch: Option<AutoFetch>,   // NEW (whole-struct patch, like pane_widths)
    pub graph: Option<GraphPrefs>,       // NEW
}

fn apply_patch(s: &mut settings::Settings, patch: UiSettingsPatch) {
    // ...existing three arms unchanged...
    if let Some(auto_fetch) = patch.auto_fetch { s.auto_fetch = clamp_auto_fetch(auto_fetch); }
    if let Some(graph) = patch.graph { s.graph = clamp_graph_prefs(graph); }
}
```

`get_ui_settings` / `set_ui_settings` add `auto_fetch: s.auto_fetch, graph: s.graph` to the returned
`UiSettings` literals. Whole-struct patch semantics match `pane_widths` (frontend sends the entire
nested object when any sub-field changes). Import `AutoFetch`, `GraphPrefs`, `clamp_auto_fetch`,
`clamp_graph_prefs` from `settings`.

### 2.3 TS mirrors — types.ts / mock.ts

`src/ipc/types.ts`:

```ts
export interface AutoFetchSettings { enabled: boolean; intervalMinutes: number; }
export interface GraphPrefs { dotRadius: number; avatarRadius: number; rowHeight: number; laneWidth: number; }

export interface UiSettings {
  theme: Theme;
  paneWidths: PaneWidths;
  listView: ListView;
  autoFetch: AutoFetchSettings;   // NEW
  graph: GraphPrefs;              // NEW
}
export interface UiSettingsPatch {
  theme?: Theme;
  paneWidths?: PaneWidths;
  listView?: ListView;
  autoFetch?: AutoFetchSettings;  // NEW
  graph?: GraphPrefs;             // NEW
}
```

`src/ipc/mock.ts`: extend `DEFAULT_UI_SETTINGS` (`autoFetch: { enabled:false, intervalMinutes:5 }`,
`graph: { dotRadius:4, avatarRadius:10, rowHeight:32, laneWidth:16 }`); add clamps
(`clampAutoFetch`, `clampGraphPrefs` mirroring the Rust ranges) in `readUiSettings`; and merge the two
new fields in `setUiSettings` (`autoFetch: patch.autoFetch !== undefined ? clampAutoFetch(patch.autoFetch) : current.autoFetch`, same for `graph`). `tauri.ts` needs NO change (same `set_ui_settings`
invoke). Export the range constants for the UI (§3) to reuse — put mins/maxes in a shared TS const,
e.g. `src/graph/metrics.ts` or a new `src/settings/ranges.ts` (recommend a tiny new module so both
mock and SettingsPanel import the same numbers; state your choice in the PR).

### 2.4 P11b unit tests (Rust)

Extend the `apply_patch` / settings tests:
- Each new field applied independently: a patch with only `auto_fetch: Some(..)` changes only
  auto-fetch; only `graph: Some(..)` changes only graph; `None` leaves both unchanged.
- Clamping on write: out-of-range `interval_minutes` (0, 999) → clamped to 1/120; each graph knob
  below-min/above-max → clamped to its bound; in-range passes through.
- Round-trip: `Settings` with non-default `auto_fetch` + `graph` `save_to`→`load_from` equals the
  input; a legacy JSON without the keys loads with defaults (extend
  `old_settings_file_without_*` style test).

---

## §3 P11c — Settings page UI

### 3.1 `src/components/SettingsPanel.tsx` — new full-screen overlay "page"

Mirror the `ShortcutOverlay` idiom (`.dialog-overlay` backdrop, `.dialog-card` variant e.g.
`.settings-card`, `role="dialog"`, backdrop-click + ✕ close, Esc handled by App's global handler).

```ts
export interface SettingsPanelProps {
  open: boolean;
  onClose(): void;
  theme: Theme;
  listView: ListView;
  autoFetch: AutoFetchSettings;
  graph: GraphPrefs;
  /** Fires on ANY change with a partial patch; App debounces the persist +
   *  updates its own state so consumers re-render live. */
  onChange(patch: UiSettingsPatch): void;
  /** Reuse App's existing toggles for the Appearance section. */
  onToggleTheme(): void;
  onToggleListView(): void;
}
export function SettingsPanel(props: SettingsPanelProps): JSX.Element | null; // null when !open
```

Three sections:

1. **Auto-fetch.** A checkbox bound to `autoFetch.enabled` → `onChange({ autoFetch: { ...autoFetch,
   enabled } })`. An interval control (number input + range slider) `min=1 max=120 step=1` bound to
   `autoFetch.intervalMinutes`, disabled when `!enabled`, → `onChange({ autoFetch: { ...autoFetch,
   intervalMinutes } })`. Label the unit ("minutes"). Copy: "Fetch the active repository
   automatically."
2. **Graph.** Four labeled controls (number input + range slider each), ranges/steps from §2.3:
   dot radius (2–10), avatar radius (6–16), row height (24–48), lane width (10–28). Each →
   `onChange({ graph: { ...graph, <field>: value } })`. Show the current value; **live preview** is
   automatic because App threads `graph` into the canvas (§4) and re-renders.
3. **Appearance.** Theme control (reuse `onToggleTheme`, reflect `theme`) and list-view control
   (reuse `onToggleListView`, reflect `listView`). No new persistence path — those toggles already
   persist (App §181/§191).

Clamp values in the handlers before calling `onChange` (defense; backend also clamps).

### 3.2 App wiring — `src/App.tsx`

- New state: `const [settingsOpen, setSettingsOpen] = useState(false);` plus
  `const [autoFetch, setAutoFetch] = useState<AutoFetchSettings>({ enabled:false, intervalMinutes:5 });`
  and `const [graph, setGraph] = useState<GraphPrefs>({ dotRadius:4, avatarRadius:10, rowHeight:32, laneWidth:16 });`
  and `const [metricsVersion, setMetricsVersion] = useState(0);` (see §4).
- Launch effect (~234): after reading `s = await ipc.getUiSettings()`, also
  `setAutoFetch(s.autoFetch); setGraph(s.graph);`.
- **Gear button** in the header toolbar (`.header-toolbar`, alongside the theme/list-view buttons):
  `className="btn-icon settings-toggle"`, gear glyph (e.g. `⚙`), `onClick={() =>
  setSettingsOpen(true)}`, `title/aria-label="Settings"`.
- Render `<SettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} theme={theme}
  listView={listView} autoFetch={autoFetch} graph={graph} onChange={handleSettingsChange}
  onToggleTheme={toggleTheme} onToggleListView={toggleListView} />` as a sibling near
  `ShortcutOverlay` (~469).
- Include `settingsOpen` in `globalModalOpen` (`overlayOpen || menuOpen || settingsOpen`) so the
  workspace Esc-layering and shortcuts defer to the modal; and add an Esc-close for it in the
  existing overlay-Esc effect (~311) alongside `overlayOpen`.
- **Debounced persist** (mirror `commitPaneWidths` @172): `handleSettingsChange(patch)` updates local
  state immediately (`if (patch.autoFetch) setAutoFetch(patch.autoFetch); if (patch.graph) {
  setGraph(patch.graph); setMetricsVersion((v) => v + 1); }`) and debounces a single
  `ipc.setUiSettings(patch)` write on a ~300 ms timer, toasting on failure. Graph changes bump
  `metricsVersion` so the canvas re-measures (§4).

### 3.3 Prop flow to the workspace

Pass `autoFetch`, `graph`, and `metricsVersion` down to each `RepoWorkspace` (mounted per tab,
~421):

```tsx
<RepoWorkspace ... autoFetch={autoFetch} graph={graph} metricsVersion={metricsVersion} />
```

`RepoWorkspace` uses `graph` + `metricsVersion` for the canvas (§4) and `autoFetch` for the timer
(§5-P11e).

---

## §4 P11d — Apply graph knobs to the renderer

Rust owns layout math; the four knobs here are **pure render geometry** (dot/avatar size, row
height, lane spacing) — no layout recompute, no IPC change. Thread a settings-derived
effective-metrics object into the canvas.

### 4.1 Effective metrics

Define in `src/graph/metrics.ts`:

```ts
export type EffectiveMetrics = typeof METRICS;

/** Overlay the four user knobs onto the METRICS baseline. All other fields
 *  (gutter, refColWidth, ring widths, fonts, maxRenderLanes, …) are unchanged. */
export function effectiveMetrics(g: {
  dotRadius: number; avatarRadius: number; rowHeight: number; laneWidth: number;
}): EffectiveMetrics {
  return { ...METRICS, dotRadius: g.dotRadius, avatarRadius: g.avatarRadius,
           rowHeight: g.rowHeight, laneWidth: g.laneWidth };
}
```

`RepoWorkspace` computes `const metrics = useMemo(() => effectiveMetrics(graph), [graph]);` and
passes it to `GraphCanvas`.

### 4.2 `draw.ts` — a `metrics` param replaces direct METRICS reads for the four knobs

The module-level pure geometry helpers and draw passes currently read `METRICS` directly. Change the
ones that touch the **four knobs (or `rowHeight`-derived `HALF_ROW`)** to take an
`m: EffectiveMetrics` parameter; leave fields that never change (gutter, refColWidth, fonts, ring
radii/widths, maxRenderLanes) reading `m.*` too for consistency (they equal `METRICS.*`). Signatures
that gain `m` (thread it from the single `drawGraph` call — no module-level mutable state):

- `export function laneX(lane: number, m: EffectiveMetrics): number` — uses `m.laneWidth`
  (`refColWidth`, `gutter`, `maxRenderLanes` from `m`).
- `graphAreaRight(laneCount, m)`, `summaryStartX(laneCount, m)` — use `m.laneWidth`.
- `export function rowY(row: number, scrollTop: number, m: EffectiveMetrics): number` —
  `row * m.rowHeight + m.rowHeight / 2 - scrollTop` (drop the module `HALF_ROW` const; compute from
  `m.rowHeight` inline).
- `export function rowAtPoint(yCss, scrollTop, m)` — `Math.floor((yCss + scrollTop) / m.rowHeight)`.
- The avatar/dot/HEAD-ring/selection-ring draws (pass-4, `drawStashNode`, and the dot pass) — use
  `m.avatarRadius`, `m.dotRadius`, `m.avatarBgRingExtra`, ring radii from `m`.
- Pass-2 row backgrounds and pass-4 row-center `cy` math — use `m.rowHeight`.
- `drawGraph(...)` gains a final `m: EffectiveMetrics` param and forwards it to every helper it calls.
- Any test/self-test call sites (`p7SelfTest`, `refColArea`) pass `METRICS` (or a fixture
  `EffectiveMetrics`) explicitly.

Enumerate-and-verify: grep `METRICS.dotRadius|METRICS.avatarRadius|METRICS.rowHeight|METRICS.laneWidth|HALF_ROW`
in `draw.ts` after the change — none should remain outside the `EffectiveMetrics` type alias.

### 4.3 `GraphCanvas.tsx` — new props + runtime rowHeight

New props on `GraphCanvasProps`:

```ts
  /** Effective render geometry (METRICS overlaid with the user's graph knobs). */
  metrics: EffectiveMetrics;
  /** Bumped when any graph knob changes → forces full re-measure + repaint
   *  (analogous to `themeVersion`). */
  metricsVersion: number;
```

- Hold `const metricsRef = useRef(metrics); metricsRef.current = metrics;` (like `themeRef`), and
  **replace every `METRICS.rowHeight` read that drives virtualization/scroll math with
  `metricsRef.current.rowHeight`**: `rowIndexAt` (@113), `getVisibleRowCount` (@176), the visible-row
  window (@221-225 `wipOffset`/`firstRow`/`lastRow`), the WIP threshold (@241), `scrollToSelection`
  (@377-384), `spacerHeight` (@784), and the pass-through into `drawGraph` (`rowY`/`laneX`/etc. now
  take `m`). The hit-test avatar radius (@650) and pill anchors use `metricsRef.current.*`.
- **`metricsVersion` effect** (model on the `themeVersion` effect @356-366): on bump, re-run the
  measure/resize path and force a repaint. Because `rowHeight` changes the total scrollable height
  (`spacerHeight`) and the row↔pixel mapping, the effect must: recompute `spacerHeight`, re-measure
  the canvas (HiDPI backing store unchanged), and schedule a redraw. Depend on `[metricsVersion]`.
  The existing scroll position is kept in px; a differing `rowHeight` re-maps which rows are visible
  on the next draw — acceptable (no attempt to preserve the top row across a rowHeight change).
- `maxRenderLanes`, `refColWidth`, overscan, and all fonts stay FIXED (they are not knobs).
- Pass `metrics` into the single `drawGraph(ctx, layout, visibleEdges, vp, theme, ix, metricsRef.current)` call.

### 4.4 P11d acceptance

- Changing dot/avatar/row/lane in Settings visibly re-renders the graph (dots resize, rows grow/shrink,
  lanes spread) with correct hit-testing (clicking a row still selects the right commit at any
  rowHeight) and correct scroll extent (no clipped last row, no dead space) — verify in the harness.
- `pnpm build`/`tsc` clean; `p7SelfTest` still green (call sites pass `METRICS`).

---

## §5 P11e — Auto-fetch timer (short spec, no heavy contract)

Frontend-only; reuses `ipc.fetch(repoId)` (no backend change). In `RepoWorkspace.tsx`:

- Prop `autoFetch: AutoFetchSettings` (from App, §3.3).
- Read `mutating` via a ref (`mutatingRef.current = mutating`) so the interval callback sees the
  latest value WITHOUT the timer resetting on every mutation.
- `useEffect(() => { ... }, [active, autoFetch.enabled, autoFetch.intervalMinutes, repoId])`:
  - If `!active || !autoFetch.enabled` → do nothing (return no-op cleanup) — **active tab only,
    off by default**.
  - `const id = window.setInterval(tick, autoFetch.intervalMinutes * 60000);`
  - `tick`: `if (mutatingRef.current) return;` then `void ipc.fetch(repoId).then((res) => { const
    updated = res.remotes.reduce((n, r) => n + r.updatedRefs, 0); if (updated > 0) { void
    refreshAll(); pushToast('info', \`Fetched ${updated} ref${updated===1?'':'s'}\`); } })
    .catch((e) => pushToast('warning', \`Auto-fetch failed: ${errorMessage(e)}\`));`
  - **Silent on no-op** (no toast when `updated === 0`), quiet toast on error (no error banner).
  - Cleanup: `clearInterval(id)` (fires on unmount, on tab deactivation, and on any setting change —
    which reschedules with the new interval).
- The timer never fires for background tabs and never overlaps a mutation.

---

## §6 P11g — Scrollable all-files diff view (`DiffBrowser`)

Azure-DevOps-style: a file tree on the left filters a right-hand vertical scroll of stacked per-file
diffs. Replaces the single-file `DiffOverlay` interaction **for compare mode and commit-selected
mode only** (the working-dir StatusPanel keeps its existing single-`diffSlot` overlay unchanged).

### 6.1 Data strategy — LAZY per-file (no new backend command). Decision + justification.

**Decision: lazy per-file hunk loading, reusing the existing `compareWithHeadFileDiff` /
`getCommitFileDiff` commands. No new Rust.** Rationale:
- The header/hunk split exists precisely so the wire never carries every file's hunks at once; a
  bulk "all hunks" command would reintroduce the unbounded-payload problem the split solved (a large
  comparison could be tens of MB), violating the "compact, already-computed" IPC invariant.
- The per-file `MAX_FILE_DIFF_LINES = 5000` cap and `binary`/`tooLarge` flags already give bounded,
  cheap per-file responses; loading only visible cards keeps memory and IPC proportional to what the
  user actually looks at.
- The mock already implements both per-file commands — zero mock/backend churn.
If a future perf need arises, a batched command is an additive optimization; it is explicitly NOT in
P11g.

### 6.2 `src/components/DiffBrowser.tsx` — new component

```ts
export type DiffScope =
  | { kind: 'root' }
  | { kind: 'dir'; prefix: string }   // TreeDir.fullPrefix (no trailing '/')
  | { kind: 'file'; path: string };

export interface DiffBrowserProps {
  repoId: string;
  /** Which commands to call + how to label the root. */
  source:
    | { mode: 'commit'; oid: string; title: string }
    | { mode: 'compare'; oid: string; fromLabel: string; toLabel: string };
  /** Header list (already fetched by RepoWorkspace: CommitDiff.files / CompareDiff.files). */
  files: FileDiffHeader[];
  listView: ListView;
  onClose(): void;
}
export function DiffBrowser(props: DiffBrowserProps): JSX.Element;
```

Layout: a full-area overlay across the **graph (main) column** (same mount slot the current
`DiffOverlay` uses, replacing it for these two modes), a flex row:
- **Left column (file tree):** the `files` headers → `buildPathTree(files, (f) => f.path)`; a
  synthetic **ROOT** row at the top labeled `source.mode === 'commit' ? 'All files' : 'All files'`
  (or the repo basename) with the total file count; then the tree. Single-click selects `scope`
  (root / dir(fullPrefix) / file(path)); the selected node is highlighted. Folders keep a
  collapse/expand chevron independent of selection.
- **Right column (stacked diffs):** a vertical scroll container rendering one **card per header**
  matching `scope` (see §6.3), each card = a sticky file-path header + a `DiffView` (or placeholder).

**Tree renderer decision (flag §8):** `Tree.tsx` binds dir-row click to collapse and exposes only a
double-click `onActivateDir`, which does not match the requested single-click select-folder
behavior. Recommendation: reuse `buildPathTree` for STRUCTURE (keep the proven collapse/sort/chain
logic and the flat backend) but render the left column with a small purpose-built recursive
`DiffFileTree` sub-component (inside `DiffBrowser.tsx`) that supports single-click selection on root,
dirs, and files plus an independent expand/collapse chevron. Do NOT bend the shared `Tree`
(sidebar/status/compare/commit all depend on it). If the orchestrator prefers strictly reusing
`Tree`, the alternative is extending `Tree` with optional `selectedKey?`/`onSelectNode?` props — more
blast radius; not recommended.

### 6.3 Scope filtering

Given `scope` and the header list, the visible cards are:
- `root` → all `files` (in the existing path-ascending order).
- `dir` → `files.filter(f => f.path === scope.prefix || f.path.startsWith(scope.prefix + '/'))`.
- `file` → the single header with `path === scope.path`.

Default `scope` on mount = `{ kind: 'root' }`.

### 6.4 Lazy per-file loading

- Per-card state cache keyed by `${source.oid}:${header.path}`:
  `Map<string, { state: 'idle'|'loading'|'ready'|'error'; diff?: FileDiff; error?: string }>`
  (component-local `useRef` + a version counter to trigger re-render, or `useState` map).
- **IntersectionObserver** on each card's root element; when a card enters the viewport (root =
  the scroll container, small rootMargin e.g. `200px`) and its entry is `idle`, enqueue a fetch.
- **Concurrency cap:** at most **4** in-flight fetches; a small queue drains as fetches resolve.
- Fetch call by mode:
  - `commit` → `ipc.getCommitFileDiff(repoId, source.oid, header.path, header.origPath)`
  - `compare` → `ipc.compareWithHeadFileDiff(repoId, source.oid, header.path, header.origPath)`
- **Binary short-circuit:** if `header.binary`, render the "Binary file" placeholder WITHOUT
  fetching. Otherwise fetch; a returned `FileDiff.tooLarge` renders the existing "Diff too large"
  placeholder, `binary` the binary placeholder, empty hunks the "No changes" placeholder — reuse
  `DiffView` (it already handles all three).
- Cards render: `idle`/`loading` → a skeleton/loading row (reuse `SkeletonRows` if convenient);
  `ready` → `<DiffView diff={...} />`; `error` → an inline error line with a retry affordance.
- Cache persists while `DiffBrowser` is mounted; changing `scope` re-filters WITHOUT refetching
  already-loaded files.

### 6.5 Mount / entry / close — `RepoWorkspace.tsx`

- New state: `const [diffBrowser, setDiffBrowser] = useState<{ mode: 'commit'|'compare'; oid: string } | null>(null);`
- **Entry points** (both modes; the header list is already fetched into `commitDiff`/`compareData`):
  - Compare mode: an "Open diff" / "View all changes" affordance in `ComparePanel` (and/or clicking
    a file row) opens `DiffBrowser` with `{ mode:'compare', oid: compare.oid }`.
  - Commit-selected mode: same affordance in `CommitPanel` opens `{ mode:'commit', oid:
    graph.nodes[selectedIndex].id }`.
  - Recommendation: keep the right-panel `ComparePanel`/`CommitPanel` as the summary; make a
    file-row click (and an explicit "View all changes" button) open `DiffBrowser` (scrolled to that
    file when a specific row was clicked — set initial `scope = { kind:'file', path }`). This
    replaces the previous single-file `DiffOverlay` toggle for these two modes; `handleToggleCommitDiff`
    / `handleToggleCompareDiff` and the `diffSlot`/`DiffOverlay` path are retired for compare+commit
    (the working-dir StatusPanel keeps them).
- **Render:** in the graph-`<main>` area (~1517), when `diffBrowser !== null` render `<DiffBrowser
  repoId={repoId} source={...} files={mode==='commit' ? commitDiff.files : compareData.files}
  listView={listView} onClose={() => setDiffBrowser(null)} />` INSTEAD of the old `DiffOverlay` for
  these modes (guard that the header list is loaded).
- **Close:** ✕ button + Esc (extend the workspace Esc-layering effect @1304: if `diffBrowser !==
  null`, close it first — it sits ABOVE compare/commit selection in the layering). Left-clicking a
  new commit row or exiting compare also closes it (clear `diffBrowser` in those handlers).
- Build the `source` label fields from existing data (`fromLabel = HEAD(+branch)`, `toLabel =
  shortOid + summary` for compare; `title = shortOid + summary` for commit) — reuse
  `ComparePanel`/`CommitPanel` label logic.

### 6.6 P11g acceptance

- Compare-with-HEAD AND selecting a commit both can open the stacked scrollable `DiffBrowser`.
- Root scope shows every file's diff stacked; selecting a folder filters to that subtree; selecting
  a file shows just that one.
- Scrolling lazily loads each file's hunks as it enters view (network/mock calls fire per file, not
  all upfront; concurrency capped); already-loaded files are not refetched when scope changes.
- Binary and `tooLarge` files show placeholders (binary without a fetch); errors are per-card with
  retry.
- Esc / ✕ close the browser; the right-panel summary and graph remain intact behind it.
- `pnpm build`/`tsc` clean; mock compiles and serves per-file diffs.

---

## §7 Consolidated acceptance checklist

Rust (`cargo test`, scratch under `D:\Temp\bonsai-scratch`, `TMP/TEMP=D:\Temp`; run clippy/tests
sequentially):
- [ ] `create_branch_here`: clean → `{stashed:false, apply:null}`, HEAD moved to new branch at oid.
- [ ] `create_branch_here`: dirty at an OLDER commit → `{stashed:true, apply:Applied}`, changes present,
      stash stack empty.
- [ ] `create_branch_here`: conflict carry-over → `{stashed:true, apply:Conflicts{paths}}`, stash RETAINED.
- [ ] `create_branch_here`: existing name / bad oid → error BEFORE any stash (stack unchanged, HEAD unchanged).
- [ ] `create_branch_here`: mid-merge/rebase → `OperationInProgress`.
- [ ] `apply_patch`: auto_fetch/graph applied independently; clamped on write; `None` = unchanged.
- [ ] settings round-trip incl. new fields; legacy JSON loads with defaults.
- [ ] `cargo check` / clippy clean; wire serde shapes match TS (camelCase; `apply: null` when None).

Frontend (`pnpm build` + browser harness `VITE_MOCK_IPC=1`):
- [ ] "Create branch here" item (with `BranchIcon`) on branch pills, sidebar branch rows, and commit
      rows; gated while busy; `PromptDialog` opens, validates, submits; Applied/Conflicts/clean toasts.
- [ ] Gear opens `SettingsPanel`; three sections present; interval+toggle and the four graph knobs
      persist across reload (mock localStorage).
- [ ] Changing dot/avatar/row/lane visibly re-renders the graph; hit-testing + scroll extent correct
      at min/max rowHeight.
- [ ] Auto-fetch OFF by default; enabling it triggers `ipc.fetch` on the interval for the ACTIVE tab
      only; silent on no-op, quiet toast on refs-updated/error.
- [ ] `DiffBrowser` opens from compare AND commit modes; root/folder/file scope filters; scroll
      lazy-loads; binary/tooLarge/error handled; Esc/✕ close.
- [ ] `mock.ts` compiles; `tsc`/build clean; `p7SelfTest` green.

USER CHECKPOINT (native `pnpm tauri dev`, not self-declarable): create-branch-here stash→switch→
re-apply on a real repo; auto-fetch hits a real remote on the interval (active tab only); settings
persist across restart and graph metric changes feel right; multi-file diff scroll feel over a large
comparison.

---

## §8 Flags / decisions for the orchestrator

1. **Bad-oid error kind (P11f):** reused `AppError::Git` with a clear message rather than adding a new
   `invalidOid` variant — keeps the error surface small; the UI shows the message verbatim. Change if
   a dedicated kind is wanted.
2. **`create_branch_here` dirtiness detection:** reuses `create_stash`'s own `created` result +
   `require_clean` gate instead of a separate status scan — fewer moving parts and the exact right
   mid-operation guard. Confirmed correct against `stash.rs`.
3. **DiffBrowser tree (P11g):** recommends a purpose-built single-click `DiffFileTree` over
   `buildPathTree` data rather than bending the shared `Tree` (whose dir-click = collapse,
   select-folder = double-click). Flagged because the plan said "reuse Tree/buildPathTree" — structure
   is reused; the renderer is not, with rationale in §6.2.
4. **DiffBrowser owns its per-file fetching** (IntersectionObserver + concurrency queue + cache),
   a deliberate, localized exception to the "App owns all diff fetching" (`diffSlot`) pattern — the
   lazy-visible loader is naturally component-scoped; App still owns the compare/commit mode state and
   the header list. Flag if the orchestrator wants fetching hoisted to `RepoWorkspace`.
5. **Retiring the single-file `DiffOverlay` for compare+commit modes:** P11g replaces it there; the
   working-dir StatusPanel keeps `diffSlot`/`DiffOverlay` unchanged. Confirm this is the intended UX
   (vs keeping both).
6. **Clamp ranges** (dot 2–10, avatar 6–16, row 24–48, lane 10–28, interval 1–120) are architect
   picks; adjust if the graph looks wrong at the extremes at USER CHECKPOINT.
