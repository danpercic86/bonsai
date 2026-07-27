# M0 — Scaffold: Implementation Contract

Status: authoritative for M0. Implementer: senior-dev. UI styling references
`docs/contracts/ui-reference.md` (read it before building the shell).

Goal: a Tauri v2 + React/Vite/TS app named **Bonsai** that opens a window, lets the user pick a
folder, and shows whether it is a Git repo plus its HEAD. Plus: mock IPC browser harness, pinned
toolchain, Rust unit tests on fixture repos.

---

## 1. Project layout (exact files for M0)

```
.gitattributes                     # "* text=auto eol=lf" + binary exceptions
rust-toolchain.toml                # pinned Rust channel
package.json                       # npm package "bonsai", packageManager pin
pnpm-lock.yaml                     # generated
vite.config.ts
tsconfig.json
index.html
src/
  main.tsx                         # React entry, mounts <App/>
  App.tsx                          # 3-pane shell + open-repo flow
  styles.css                       # theme tokens from ui-reference.md (dark default)
  ipc/
    types.ts                       # TS mirror types (RepoInfo, HeadInfo, AppError) + IpcApi
    index.ts                       # selects real vs mock impl via VITE_MOCK_IPC
    tauri.ts                       # real impl: invoke() + plugin-dialog open()
    mock.ts                        # mock impl: canned fixtures, fake pickFolder
src-tauri/
  Cargo.toml                       # crate "bonsai"
  build.rs                         # tauri_build::build()
  tauri.conf.json
  capabilities/
    default.json
  icons/                           # tauri icon set (generate with `pnpm tauri icon` or copy defaults)
  src/
    main.rs                        # windows_subsystem attr; calls bonsai::run()
    lib.rs                         # builder, .manage(AppState), generate_handler!, dialog plugin
    commands.rs                    # #[tauri::command] open_repo
    error.rs                       # AppError (thiserror + Serialize)
    state.rs                       # AppState
    git/
      mod.rs                       # pub mod repo;
      repo.rs                      # read_repo_info(path) + unit tests
```

Do NOT create `src/graph/`, `src-tauri/src/graph.rs`, or `src-tauri/src/watcher.rs` yet — later
milestones.

## 2. Naming (exact)

- Window title: `Bonsai`. `tauri.conf.json`: `productName: "Bonsai"`, `identifier: "com.bonsai.app"`.
- `package.json` `"name": "bonsai"`, `"private": true`.
- `Cargo.toml` `name = "bonsai"`; lib section: `name = "bonsai"`, `crate-type = ["staticlib", "cdylib", "rlib"]`.

## 3. Pinned versions

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.88"        # or newer current stable; pin an explicit minor, not "stable"
components = ["clippy", "rustfmt"]
```

`package.json`: `"packageManager": "pnpm@11.17.0"`.

Rust deps (`src-tauri/Cargo.toml`):

```toml
[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
git2 = "0.20"            # vendored libgit2; MSVC toolchain builds it
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
```

Frontend deps: `react` ^19, `react-dom` ^19, `@tauri-apps/api` ^2, `@tauri-apps/plugin-dialog` ^2.
Dev deps: `vite` ^7, `typescript` ~5.9, `@vitejs/plugin-react` ^5, `@tauri-apps/cli` ^2,
`@types/react` ^19, `@types/react-dom` ^19. Use whatever exact versions `pnpm add` resolves; do not
downgrade.

Scripts: `"dev": "vite"`, `"dev:mock": "vite --mode mock"` (optional convenience; harness is run as
`VITE_MOCK_IPC=1 pnpm dev` / `$env:VITE_MOCK_IPC='1'; pnpm dev`), `"build": "tsc && vite build"`,
`"preview": "vite preview"`, `"tauri": "tauri"`.

`tauri.conf.json` essentials:

```json
{
  "productName": "Bonsai",
  "version": "0.1.0",
  "identifier": "com.bonsai.app",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [{ "title": "Bonsai", "width": 1280, "height": 800, "minWidth": 900, "minHeight": 600 }],
    "security": { "csp": null }
  },
  "bundle": { "active": true, "targets": "all", "icon": ["icons/icon.ico"] }
}
```

`vite.config.ts`: `@vitejs/plugin-react`, `clearScreen: false`,
`server: { port: 1420, strictPort: true }`, `envPrefix: ['VITE_', 'TAURI_ENV_']`.

## 4. IPC surface (M0: exactly one command)

### Rust types (`git/repo.rs` for domain types, `error.rs` for AppError)

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadInfo {
    pub branch_name: Option<String>, // None when detached or unborn
    pub oid: String,                 // full 40-char hex; "" when unborn
    pub detached: bool,
    pub unborn: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub path: String,                // canonical workdir path as passed in
    pub is_repo: bool,
    pub head: Option<HeadInfo>,      // None iff is_repo == false
}
```

**Unborn-HEAD decision (locked):** an `init`'d repo with no commits returns
`is_repo: true, head: Some(HeadInfo { branch_name: Some("<default-branch>"), oid: "", detached: false, unborn: true })`.
The branch name comes from the symbolic HEAD target (`refs/heads/main` → `main`). `head` is `None`
only when `is_repo` is false. This gives M1+ a stable "repo open but empty" signal.

```rust
// error.rs
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("git error: {0}")]
    Git(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("{0}")]
    Other(String),
}
// Serialize as { "kind": "git" | "io" | "other", "message": "..." }
impl serde::Serialize for AppError { /* manual impl producing the shape above */ }
impl From<git2::Error> for AppError { /* -> Git(e.message().to_string()) */ }
impl From<std::io::Error> for AppError { /* -> Io(e.to_string()) */ }
```

### Core function (pure, testable — no Tauri types)

```rust
// git/repo.rs
/// Blocking. Opens the repo at `path` (no search upward: use Repository::open_ext with
/// git2::RepositoryOpenFlags::NO_SEARCH so a subfolder of a repo is reported is_repo=false).
/// A non-repo directory returns Ok(RepoInfo { is_repo: false, head: None }), NOT Err.
/// Err is reserved for real failures (path does not exist / not a directory / IO).
pub fn read_repo_info(path: &std::path::Path) -> Result<RepoInfo, AppError>;
```

Unborn detection: `repo.head()` failing with `ErrorCode::UnbornBranch` (or
`repo.head_unborn()? == true`) → build the unborn `HeadInfo`, reading the branch name from
`repo.find_reference("HEAD")?.symbolic_target()` (strip `refs/heads/`). Detached:
`repo.head_detached()?` → `branch_name: None`, `oid` = head commit id, `detached: true`.

### Command (`commands.rs`)

```rust
#[tauri::command]
pub async fn open_repo(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<RepoInfo, AppError> {
    // 1. spawn_blocking(move || read_repo_info(&path)) via tauri::async_runtime::spawn_blocking
    // 2. on Ok(info) where info.is_repo: store path into state (see §5)
    // 3. return info
}
```

Join errors from `spawn_blocking` map to `AppError::Other`.

### TypeScript mirrors (`src/ipc/types.ts`)

```ts
export interface HeadInfo {
  branchName: string | null;
  oid: string;
  detached: boolean;
  unborn: boolean;
}

export interface RepoInfo {
  path: string;
  isRepo: boolean;
  head: HeadInfo | null;
}

export interface AppError {
  kind: 'git' | 'io' | 'other';
  message: string;
}

export interface IpcApi {
  openRepo(path: string): Promise<RepoInfo>;      // rejects with AppError
  pickFolder(): Promise<string | null>;           // null = user cancelled
}
```

## 5. App state (`state.rs`)

```rust
#[derive(Debug, Clone)]
pub struct OpenRepo {
    pub path: std::path::PathBuf,   // workdir root of the opened repo
}

#[derive(Debug, Default)]
pub struct AppState {
    pub repo: std::sync::Mutex<Option<OpenRepo>>,
}
```

- M0 stores only the path. `git2::Repository` is **not** kept in state; every command reopens it
  (cheap, avoids Send/Sync headaches). Later milestones extend `OpenRepo` (watcher handle, caches)
  without changing this shape.
- Registered in `lib.rs` via `.manage(AppState::default())`.

`lib.rs` skeleton:

```rust
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![commands::open_repo])
        .run(tauri::generate_context!())
        .expect("error while running Bonsai");
}
```

## 6. Folder picker + capability

Frontend (real impl only, `src/ipc/tauri.ts`): `import { open } from '@tauri-apps/plugin-dialog'`;
`pickFolder()` = `open({ directory: true, multiple: false, title: 'Open repository' })`, returning
`string | null`.

`src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:allow-open"
  ]
}
```

## 7. Mock IPC harness

- `src/ipc/index.ts`:

```ts
import type { IpcApi } from './types';

export const ipc: IpcApi = import.meta.env.VITE_MOCK_IPC === '1'
  ? (await import('./mock')).mockIpc
  : (await import('./tauri')).tauriIpc;
```

  Dynamic imports are required so a plain browser never loads `@tauri-apps/*` at runtime in mock
  mode. (Top-level await is fine with Vite; alternatively export an async `getIpc()` — implementer's
  choice, but components must consume a single `ipc` object.)
- `src/ipc/tauri.ts`: `openRepo` = `invoke<RepoInfo>('open_repo', { path })`; `pickFolder` per §6.
- `src/ipc/mock.ts` exports `mockIpc: IpcApi`:
  - `pickFolder()` → resolves (after ~150 ms) to `'C:\\mock\\bonsai-fixture'`.
  - `openRepo(path)` fixtures, keyed on path:
    - `'C:\\mock\\bonsai-fixture'` (and any unknown path): `{ path, isRepo: true, head: { branchName: 'main', oid: '9fceb02d0ae598e95dc970b74767f19372d61af8', detached: false, unborn: false } }`
    - path containing `'not-a-repo'`: `{ path, isRepo: false, head: null }`
    - path containing `'unborn'`: `{ path, isRepo: true, head: { branchName: 'main', oid: '', detached: false, unborn: true } }`
    - path containing `'error'`: reject with `{ kind: 'io', message: 'mock: path does not exist' }`
  - All mock responses go through a small `delay(150)` helper to exercise loading states.
- Rule going forward: every new IPC method added in later milestones gets a mock implementation in
  the same change.

## 8. M0 UI (`App.tsx` + `styles.css`)

Per `docs/contracts/ui-reference.md` (layout geometry, tokens, dark default):

- Header bar: app name "Bonsai" left; when a repo is open, repo folder name + full path (muted);
  refresh button placeholder (disabled in M0).
- 3-pane grid: left sidebar (placeholder text "Branches"), center (placeholder "Commit graph"),
  right panel (placeholder "Status").
- Empty state (no repo open): centered in the window — app name, short tagline, primary button
  "Open repository" → `ipc.pickFolder()` → if non-null, `ipc.openRepo(path)`.
- After open:
  - `isRepo: false` → inline error style message "Not a Git repository" + the path, keep the
    Open button available.
  - normal head → header shows `⎇ main @ 9fceb02` (branch + 7-char short OID).
  - detached → `HEAD detached @ 9fceb02` with a `detached` label pill.
  - unborn → `main (no commits yet)` with an `unborn` label pill.
  - command rejection → show `AppError.message` in the error style.
- State: plain `useState` in `App.tsx` (`repo: RepoInfo | null`, `error: string | null`,
  `loading: boolean`). No state library.

## 9. Testing requirements (Rust, in `git/repo.rs` `#[cfg(test)]`)

Fixtures built with **git2 only** (no `git` CLI), in `tempfile::TempDir` (add
`tempfile = "3"` to `[dev-dependencies]`). Helper: init repo, set local config
`user.name`/`user.email`, write a file, stage via index, `commit` with `Signature`.

1. `repo_with_one_commit`: `read_repo_info` → `is_repo == true`, `head.unwrap()` has
   `unborn == false`, `detached == false`, `branch_name == Some(<default>)` where `<default>` is
   read from the fixture repo's HEAD symbolic target (do not hardcode `main` vs `master`), and
   `oid` equals the commit id created by the fixture (assert string equality).
2. `non_repo_dir`: empty temp dir → `Ok`, `is_repo == false`, `head.is_none()`.
3. `unborn_head`: `Repository::init` only → `is_repo == true`, head `unborn == true`, `oid == ""`,
   `branch_name.is_some()`.
4. `detached_head`: fixture from (1), then `repo.set_head_detached(oid)` →
   `detached == true`, `branch_name == None`.
5. `missing_path`: nonexistent path → `Err(AppError::Io(_))` (or a non-panicking `Err`).

## 10. Acceptance criteria

AI gate (orchestrator verifies):
- `cargo check` (and ideally `cargo clippy -- -D warnings`) passes in `src-tauri/`.
- `pnpm build` (tsc + vite) passes.
- `cargo test` passes, including all §9 tests.
- Browser harness: `VITE_MOCK_IPC=1 pnpm dev` renders the 3-pane shell; clicking "Open repository"
  shows the mock repo path + `main @ 9fceb02` with no console errors and no Tauri imports executed.

USER CHECKPOINT (never self-declared):
- `pnpm tauri dev` opens a native window titled "Bonsai".
- The folder picker opens, the user selects a real repo, and its path + HEAD are shown.

## 11. Sub-increment split for senior-dev

- **M0a — Scaffold compiles.** Full file tree, configs, pinned toolchain, empty command handler
  wired, placeholder `App.tsx`. Gate: `cargo check` + `pnpm build` green; `pnpm tauri dev` builds.
- **M0b — Repo info core + command + tests.** `git/repo.rs`, `error.rs`, `state.rs`,
  `commands.rs::open_repo` with `spawn_blocking`; all §9 tests. Gate: `cargo test` green.
- **M0c — Frontend shell + IPC + mock harness.** `src/ipc/*`, `App.tsx` per §8, `styles.css`
  tokens from ui-reference.md. Gate: `pnpm build` green; harness renders and open-repo flow works
  with mock data.

## 12. Ambiguities resolved here (flag to orchestrator if disagreed)

- **Unborn HEAD**: represented as `head: Some(..unborn: true, oid: "")` — chosen over `head: None`
  so `None` unambiguously means "not a repo".
- **Repo discovery**: `open_repo` does NOT search parent directories (NO_SEARCH). Rationale:
  predictable picker semantics; a "discover from subfolder" affordance can be a Polish item.
- **Icons**: use the default Tauri icon set for M0 (`pnpm tauri icon` on any square PNG or copy the
  create-tauri-app defaults); custom branding is Polish.
