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

*Unverified step:* `wt -d` splitting on `;` was not executed (running an injected sub-command was out
of bounds for the audit). Confirm with a benign `wt -d "C:\Users; new-tab"` on a dev box.

### HIGH-2 — `.gitmodules` `path` can be rooted or UNC, and `Path::exists()` dials it

`Path::join` **replaces** the base for any rooted path. Measured on this host:

| `.gitmodules` `path` | joined result |
|---|---|
| `vendor/lib` | `D:\Data\Repos\myrepo\vendor/lib` (contained) |
| `/Windows/System32` | `D:/Windows/System32` (escapes to drive root) |
| `//attacker.example/share` | `//attacker.example/share` (**base discarded**) |
| `C:/Windows` | `C:/Windows` (base discarded; separately blocked by libgit2's trailing-colon rule) |

libgit2's `path_is_valid` clears `GIT_FS_PATH_REJECT_EMPTY_COMPONENT` for submodule paths, so a
leading `/` or `//` is accepted; it also normalizes backslashes *before* validating, which makes the
backslash-rejection flag inert here.

`src-tauri/src/commands/external.rs:72` then calls `p.exists()` on that value -> Win32 performs an
SMB/WebDAV connection to an attacker-controlled host -> **NetNTLMv2 disclosure**, relayable.

**Possible zero-click variant, UNVERIFIED and the first thing to check:** `list_submodules`
(`submodule.rs:132-141`) calls `repo.submodule_status(&name, SubmoduleIgnore::None)` for every row,
and the sidebar fetches it automatically (`useSidebarCollections.ts:56-67`). If libgit2 stats the
joined path the same way, the SMB callout happens **on repo open, before any click**, and the
external-tool surface is merely the second beneficiary. Verify with a network capture against a
scratch repo.

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

Audit run 2026-09-03. Findings reported, **no code changed by the audit itself**. Remediation of
HIGH-1 / HIGH-2 / MEDIUM-1 tracked on `TODO.md`. The `wt ;` step and the zero-click variant of HIGH-2
are explicitly marked unverified above — neither was executed, and both should be confirmed before
the severities are treated as final.
