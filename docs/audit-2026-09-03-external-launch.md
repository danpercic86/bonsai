# Security audit — external-process launching (2026-09-03)

Scope: `crates/bonsai-core/src/external.rs`, `crates/bonsai-core/src/procutil.rs`,
`src-tauri/src/commands/external.rs`, `src-tauri/src/commands/obs.rs` (`log_reveal_dir`),
`src-tauri/capabilities/default.json`, `src-tauri/tauri.conf.json`, and the frontend callers
(`src/hooks/useExternalTools.ts`, `src/components/workspaceMenusRows.ts`).

**Threat model:** the user clones or opens a repository authored by someone else. Anything read out
of `.gitmodules`, `.git/config`, a ref name, or the working tree is attacker-controlled.

---

## The one-sentence result

**The launch mechanism is well built; its stated trust assumption about the path is false.**
`crates/bonsai-core/src/external.rs:21-23` records, as an accepted residual risk, that
*"`{path}` is a repo path the user already opened — so this is self-inflicted at worst, never
attacker-controlled."* That premise does not hold: `{path}` is also `sub.absPath`, which is built
from `.gitmodules` at `crates/bonsai-core/src/git/submodule.rs:113` with no validation. That single
false comment is how the gaps below survived — it is load-bearing, and it must be corrected in the
same change as the code.

## What is genuinely well defended (evidence, so it is not re-litigated)

- **The spawn is shell-free on every platform.** `SpawnRunner::run` (`external.rs:114-143`) is
  `Command::new(program).args(&spec.args).current_dir(&spec.cwd)`. No `cmd /c`, no `start`, no
  `sh -c`, no `ShellExecute`. **`tauri-plugin-shell` is not a dependency at all** — external
  launching is first-party Rust behind four named commands, which removes the entire "shell scope
  bypass" class.
- `parse_template` (`:212-231`) substitutes `{path}` *inside* an already-tokenized argv element, so
  a template can never split a path into two arguments.
- `url_ladder` is private and `open_url` validates unconditionally before any spawn.
  `validate_web_url` (`:340-386`) allow-lists the host charset, rejects `@`, screens control and
  whitespace characters over the whole string, caps length, and returns category-only errors that
  never echo the input.
- `resolve_program` (`procutil.rs:19-45`) walks `PATH` with `PATHEXT` and **does not consult the
  current directory** — no CWD-hijack in program resolution.
- Capabilities are least-privilege and deliberately so: `dialog` is narrowed to `allow-open` rather
  than `dialog:default` (which would also carry `allow-message`/`allow-save`); `process:default` is
  `allow-exit`/`allow-restart`, used only for `relaunch()` after an update; `withGlobalTauri` is off.
- CSP sets `script-src 'self'` with **no** `unsafe-inline` and no `unsafe-eval`, and `connect-src`
  names no remote origin.
- `log_reveal_dir` (`commands/obs.rs:320-327`) derives its path from `AppHandle`. No attacker input.

---

## Findings

### HIGH-1 — `.gitmodules` `path` reaches `wt -d`, where `;` is a sub-command delimiter (Windows)

`submodule.rs:113` -> `workspaceMenusRows.ts:117` (spread **unconditionally**; the comment at :115
says "Always enabled") -> `external.rs:243` `spec("wt", &["-d", &p], …)`.

A tracked directory name may legally contain `;` and spaces on NTFS and in git. libgit2's
submodule-path check rejects `<>:"|?*`, control characters, `..`, and trailing dot/space/colon —
**not `;`**. With the default (empty) terminal template, rung 1 is `wt -d "<path>"`, and `wt` splits
its own arguments on `;` into sub-commands. The project's own comment at `external.rs:31-35` asserts
this `wt` behaviour as fact.

**Gain:** arbitrary command execution as the user, from cloning a repo and one context-menu click.
**Reachable with no user template at all**, because `wt` is rung 1 of the default ladder.

**CONFIRMED 2026-09-03** (remediation pass): spawning `wt -d "<dir with ;>"` exactly as
`SpawnRunner` does, `wt` split the single `-d` argv token at `;` and did not start in the intended
directory. Settled without executing any injected payload — the discriminator is where `wt` starts,
not what it runs.

**Nuance found while measuring, and it matters:** the value must be **quoted** in `.gitmodules`.
Unquoted, `;` and `#` are git-config comment characters and the path never reaches us at all.

### HIGH-2 — `.gitmodules` `path` can be rooted or UNC, and `Path::exists()` dials it

`Path::join` **replaces** the base for any rooted path. Measured on this host:

| `.gitmodules` `path` | joined result |
|---|---|
| `vendor/lib` | `D:\Data\Repos\myrepo\vendor/lib` (contained) |
| `/Windows/System32` | `D:/Windows/System32` (escapes to drive root) |
| `//attacker.example/share` | `//attacker.example/share` (**base discarded**) |
| `C:/Windows` | **never reaches `join`** — libgit2 drops it from `submodules()` (measured) |
| `../escape`, `..\escape`, `vendor/../../escape` | **never reaches `join`** — libgit2 drops them (measured) |

libgit2's `path_is_valid` clears `GIT_FS_PATH_REJECT_EMPTY_COMPONENT` for submodule paths, so a
leading `/` or `//` is accepted; it also normalizes backslashes *before* validating, which makes the
backslash-rejection flag inert here.

`src-tauri/src/commands/external.rs:72` then calls `p.exists()` on that value -> Win32 performs an
SMB/WebDAV connection to an attacker-controlled host -> **NetNTLMv2 disclosure**, relayable.

**The zero-click variant is DISPROVEN** (remediation pass, 2026-09-03) — this **lowers HIGH-2 to
click-triggered**. The hypothesis was that `repo.submodule_status(&name, SubmoduleIgnore::None)`,
called for every row at `submodule.rs:110` and fetched automatically by the sidebar, would stat the
escaped path on repo open. Tested by building a superproject with a real index+HEAD gitlink and
planting a real submodule repo at (a) the Rust-`join` location and (b) the workdir-**concatenated**
location: only (b) flipped the status flags (`WD_DELETED` -> `IN_WD | WD_MODIFIED`).
**libgit2 concatenates `sm->path` under the superproject workdir and does not honour the rooted/UNC
escape that Rust's `Path::join` does.** `submodule_info` itself only string-joins and touches no
filesystem.

The SMB callout is real but happens at `p.exists()` in the launch command — measured on this host:
UNC-loopback `exists()` **6.6 ms** vs **19 us** local. It dials. It just needs the click.

### MEDIUM-1 — There is no containment check anywhere

The only check on the launch path is `p.exists()` (`commands/external.rs:66-93`). No
canonicalization, no symlink resolution, no repo containment, no "is a directory" test. The core
entry points (`external.rs:452-478`) take a bare `&Path` and document only that the caller
guarantees existence.

**Two things bound the damage, and both are worth stating because they are load-bearing:**

1. **`cwd` is set to the target path itself** (`external.rs:167`, `:223`), and a non-directory as
   `current_dir` fails the spawn outright — measured: `Err(Os { code: 267, kind: NotADirectory })`.
   This kills the sharpest variant: a hostile repo shipping `payload.exe` with `.gitmodules`
   `path = payload.exe`, which would otherwise be `explorer <file>` -> ShellExecute -> execution.
   **This defence is accidental, undocumented, and one refactor away from vanishing** — adding
   `explorer /select,<file>` to reveal a *file* is the natural next feature and would open it.
2. The program is fixed by the ladder and the path is one argv token, so option injection is
   unreachable for submodule paths (`join` yields either a base-prefixed or a rooted path, never a
   leading `-`).

### MEDIUM-2 — `terminalCommand` / `editorCommand` are unvalidated program strings the webview can set

`commands/ui_settings.rs:243-247` assigns them verbatim from the patch; consumed at
`commands/external.rs:81,87`. A repo cannot write them, so this is **not** reachable under the stated
threat model — but `set_ui_settings` is an unprivileged webview command, so anything achieving script
execution in the renderer (or a future AI/MCP tool that can drive settings) parks a program name with
one call and runs it on the next "Open in terminal". The property *renderer compromise != arbitrary
local execution* does not currently hold. The tight `script-src 'self'` CSP is what keeps this at
MEDIUM.

### LOW-1 — Children inherit the hostile repo directory as cwd (Windows DLL search order)

`external.rs:117`. Under SafeDllSearchMode the current directory is searched after System32/Windows
but **before** `PATH`. A hostile repo can ship a `.dll` at its root. No concrete gadget was
enumerated — this is the primitive, not a confirmed exploit. Cheap mitigation: `explorer` and `code`
take the path as an argument, so only the terminal rung genuinely needs the cwd.

### LOW-2 — The not-found error echoes the attacker-controlled path into a toast

`commands/external.rs:73` — `format!("path no longer exists: {path}")`. Contrast `validate_web_url`,
which is deliberately category-only and documents why (`external.rs:335-339`). A repo-authored path
can be long, RTL-overridden, or crafted to imitate a system message. Same rule should apply.

### LOW-3 — `xdg-open` on a non-directory (Linux/macOS)

`external.rs:270` MIME-dispatches. Unreachable today only because of the accidental `chdir` defence
in MEDIUM-1 — which is the only thing between `.gitmodules` `path = evil.desktop` and a dispatched
launch.

### INFO — CSP is missing `form-action` / `base-uri`

Neither falls back to `default-src`. A script-execution compromise could exfiltrate by auto-submitting
a form to a remote origin despite the tight `connect-src`. Add
`form-action 'none'; base-uri 'none'; object-src 'none'` — one line, no behaviour change.

Not resolved from the on-disk manifest: whether `core:webview:default` excludes
`allow-create-webview-window` in this Tauri version. Worth one line of confirmation.

---

## Test coverage: thorough on templates, empty on paths

`crates/bonsai-core/src/external_tests.rs` is genuinely good on **template** and **URL** hostility —
`parse_template_shell_metachars_stay_literal_args`,
`parse_template_substitutes_path_with_spaces_into_one_arg`,
`url_ladder_never_uses_a_shell_and_keeps_the_url_in_one_token`,
`open_url_validates_before_spawning_anything`, and a rejection table that asserts both variant and
category with an explicit note about avoiding vacuous assertions.

It has **no** path-hostility case at all. Missing, each mapping to a finding above: `;` in a path
reaching `wt -d`; a rooted or UNC path surviving `join`; a path that is a file rather than a
directory; `..` traversal; a leading `-`; control characters; non-UTF-8. There is no test of
`submodule_info`'s `abs_path` construction at all — the fixture at `git/submodule_tests.rs:30`
hardcodes a benign `"/repo/vendor/libcore"`.

Note also `submodule.rs:113` uses `to_string_lossy()`, so a non-UTF-8 submodule path is silently
mangled into a *different* path that is then launched. Not obviously exploitable; untested and lossy.

**The gap is consistent:** every test treats the *template* as untrusted and the *path* as trusted —
exactly the assumption stated in the module header, and exactly the assumption that is wrong.

---

## Remediation, in the order it should be done

1. **Validate at the producer, not the launcher.** In `submodule_info`
   (`crates/bonsai-core/src/git/submodule.rs:113`), require `sm.path()` to be relative with only
   plain components, then canonicalize the join and require it to start with the canonicalized
   superproject workdir. Reject the row rather than launch. **One check closes HIGH-1, HIGH-2,
   MEDIUM-1 and LOW-3 together** — a `;` blocklist would be the wrong layer.
2. **Make the directory requirement explicit** at `src-tauri/src/commands/external.rs:72`, so the
   accidental `chdir` defence stops being accidental. Note it as an invariant on `LaunchSpec`.
3. **Correct `external.rs:21-23`.** The accepted-residual-risk note is false and load-bearing.
4. LOW-2's category-only error; the CSP one-liner; then MEDIUM-2 and LOW-1 as their own increments.
5. Add the path-hostility tests. Each must be proven to fail against the unfixed code.

---

## Status

Audit run 2026-09-03; **no code changed by the audit itself**. Remediation landed the same day.

**Both unverified steps were then measured, and one of them changed a severity:**

| claim | outcome |
|---|---|
| `wt` splits `;` inside one argv token | **CONFIRMED** — HIGH-1 stands |
| HIGH-2 is zero-click via `submodule_status` | **DISPROVEN** — libgit2 concatenates; click-triggered only |
| `C:/Windows` and `..` reach `Path::join` | **WRONG in this report** — libgit2 drops them first |
| MEDIUM-1's accidental `NotADirectory` defence | **reproduced as described** |

The residual admitted-and-escaping set is therefore exactly **rooted** (`/x`) and **UNC**
(`//h/s`, `\\h\s`). `;` and spaces are admitted but stay contained — they are dangerous only once a
path has *also* escaped, or via the `wt` delimiter on a contained path.

**Closed by `contained_abs_path`** (`crates/bonsai-core/src/git/submodule_abs_path.rs`), a
producer-side gate: HIGH-1, HIGH-2, MEDIUM-1, LOW-3, plus LOW-2's path echo and MEDIUM-1's implicit
directory assumption at the command layer. `..` and drive-letter paths are rejected there too as
defence in depth, even though libgit2 clears them today.

**Still open, each its own increment:** MEDIUM-2 (`terminalCommand`/`editorCommand` unvalidated),
LOW-1 (cwd DLL search order), INFO (CSP `form-action`/`base-uri`/`object-src`).

**One residual documented rather than closed:** a symlink introduced *inside* an already-checked-out
superproject at a not-yet-created leaf path bypasses the canonicalize recheck, since `canonicalize`
fails on a missing leaf. The primary vectors are closed lexically regardless of filesystem state.
