# Security audit — P125 dependency tracker contract, pre-implementation (2026-09-28)

**Scope:** `docs/contracts/P125-dependency-tracker.md` (§0, §4.1–4.4, §5, §6.4–6.5, §7, §8, §9, §11)
against `docs/specs/009-dependency-tracker/spec.md` (D1–D9, AC1–AC13). There is no code yet, so
this audits the **design**. Findings are written as contract amendments.
**Existing code checked:** `crates/bonsai-forge/src/http.rs`, `crates/bonsai-forge/Cargo.toml`,
`crates/bonsai-core/src/external_url.rs::validate_web_url`, `src-tauri/src/commands/external.rs::open_url`,
`src-tauri/tauri.conf.json` CSP, `src-tauri/capabilities/*.json`, `src-tauri/src/obs/{trace.rs::emit_logged, mod.rs::arm_panic_flush}`,
`deny.toml`, the reqwest feature set in the resolved graph (`cargo tree -e features -i reqwest`), the local
RustSec advisory DBs (latest commit 2026-09-23), and the upstream `yaml-rust2` `YamlLoader` source.
**Method:** read-only. Nothing was executed against a repository.

## The one-sentence result

Nothing is CRITICAL. There are **two HIGHs**. First, the D7 "private deps are never sent" invariant
is **fail-open** and misses the most common place a private scope registry is configured: a
gitignored repo `.npmrc`, which `discover_full` skips by design. Second, the chosen YAML crate expands
aliases without a limit. A few-KB hostile `pnpm-lock.yaml` therefore OOM-aborts the app on open, and
session restore makes that a crash loop. The type-level "structural" guarantee has two holes as
specified (§7.3 `AdvisoryId::parse`, §7.4 bonsai-core's spawning APIs). Both are cheap to close
before P125a.

## What is genuinely well defended (evidence, so it is not re-litigated)

- **Fixed hosts, no steering (§7.1/§7.2/§7.6).** Hosts are `&'static str`, `https_only(true)`, and
  `redirect::Policy::none()`. Response `links`/`registries`/`url` fields are not deserialized. Registry
  links come from `catalog.rs` templates, and the name always follows the first `/` after a static host.
  `validate_web_url` (`crates/bonsai-core/src/external_url.rs:55-101`) allowlists host characters and
  rejects `@` userinfo. No repo-controlled or response-controlled string can pick the host that
  `open_url` opens. **§7.6 is clean**, apart from the path-normalisation nit in LOW-2.
- **No decompression bombs.** The resolved reqwest features are `blocking, json, stream,
  rustls-no-provider, __rustls, __tls`. There is no `gzip`/`brotli`/`zstd`/`deflate`, so response bodies are
  never auto-decompressed. The §7.2 body caps are therefore caps on wire bytes.
- **The claim that the forge transport should not be reused is confirmed.** `bonsai_forge::HttpTransport::send`
  (`crates/bonsai-forge/src/http.rs:156-187`) takes an arbitrary `url` and arbitrary `headers`. The client
  builder at `http.rs:147-150` sets only `redirect(none)`, with no `connect_timeout`/`timeout`/`https_only`.
  One nuance: reqwest's *blocking* client has a default 30 s total timeout, so "no timeouts" is slightly
  overstated. The conclusion still stands.
- **No CSP/capability widening.** The CSP `connect-src` is `'self' ipc: http://ipc.localhost`. The
  capabilities grant no `http:`, `shell:` or `fs:` permissions, so the webview cannot reach deps.dev/OSV
  itself. All HTTP is in Rust.
- **Obs logging.** `emit_logged` (`src-tauri/src/obs/trace.rs:150-168`) logs the event *name* only, never
  the payload. `DepsChangedPayload` contains no strings except `repoId`. The panic hook
  (`obs/mod.rs:282`) only flushes and then chains to the default hook, which prints to stderr. Panic
  payloads are not persisted to the session log.
- **In-process git2.** `Repository::open`, `index()` and `statuses()` run no hooks, no `core.fsmonitor`
  (libgit2 does not implement it), no external filter drivers and no credential helpers. The discovery
  path is process-free.
- **Disable/persist race (§6.4).** Persisting under the same `repos` lock that `apply_config` holds while
  it bumps `epoch` does ensure no cache write lands after the wipe. The startup wipe covers a failed
  `remove_dir_all`. Nothing else persists dependency data: `emit_logged` does not log payloads, and
  settings hold only `{enabled, intervalHours}`.
- **Parsers with built-in limits.** serde_json keeps its 128-depth recursion limit. quick-xml does not
  expand DTD or external entities.

---

## Findings

### HIGH-1 — D7 is fail-open, and the commonest private-scope signal is invisible to it (§4.1, §4.3, §4.4, §7.3, §9 AC6)

**Scenario (no attacker needed; this is a privacy failure of the headline invariant).** A company
monorepo uses pnpm and depends on `@corp/billing-core` (among others) from a private registry. The
scope mapping `@corp:registry=https://npm.corp.example/` is in the **repo-root `.npmrc`**. That file is
**gitignored**, which is common precisely because it also holds `//npm.corp.example/:_authToken=…`. The
user enables the tracker. Here is what happens:

1. `discover_full` (§4.1 line 336) builds the file set from the index plus `statuses(…, !include_ignored)`.
   The ignored `.npmrc` is never in `files`, so `RegistryContext::load(workdir, &files, …)` (§4.4) never
   sees it.
2. Classification cannot fall back on the lockfile host. `pnpm-lock.yaml` v6–v9 records only
   `resolution: {integrity}` for packages from the configured registry. pnpm leaves the registry out on
   purpose, to keep lockfiles portable. So the "lock `resolved`/`tarball` host" rule in §4.3 has nothing
   to match. *(High confidence from pnpm's lockfile design. Not verified against a live lockfile here.)*
3. `@corp/billing-core` passes `valid_public_name`. `classify` returns `Public` because its only negative
   branches are detected signals. The name and resolved version go to `api.deps.dev` and `api.osv.dev`.

The same fail-open shape leaks in other cases the contract does not cover:
- **npm/yarn config sources:** env `NPM_CONFIG_REGISTRY` / `npm_config_@scope:registry` /
  `YARN_NPM_REGISTRY_SERVER` / `YARN_REGISTRY`; yarn v1 `.yarnrc`; the global `$PREFIX/etc/npmrc`; the
  user-level `~/.yarnrc.yml`.
- **Other ecosystems:** Cargo `CARGO_REGISTRIES_<NAME>_INDEX`, and legacy `.cargo/config` without the
  `.toml` extension; pip `PIP_CONFIG_FILE`, the site-wide `/etc/pip.conf`, and the per-OS
  `%APPDATA%\pip\pip.ini` versus `~/.config/pip/pip.conf` versus `~/.pip/pip.conf`; uv/pyproject
  `[[tool.uv.index]]` with `default = true`; the NuGet machine-wide config and `<clear/>` /
  `<disabledPackageSources>`.
- **Internal workspace members:** a yarn v1 workspace member (`"@corp/b": "^1.0.0"`, with no lock entry
  for it) is classified `Registry(Default)`, i.e. Public.

**Why the proof misses it.** The type path in §7.3 proves that *whatever reaches a socket went through
`classify`*. It does not prove that `classify` is right. The AC6 sentinel test covers only signals the
adapters already detect, so it cannot fail for a missed signal.

**Amendment (contract, before P125a — `classify` semantics and the `RegistryContext` interface land in P125a):**
1. **Registry-config lookup must not use the git `FileSet`.** Read each ecosystem's config files by a
   bounded filesystem ancestor walk from each manifest's directory up to the workdir, whether or not the
   file is tracked or ignored, applying the MEDIUM-3 read rules. Keep `FileSet` for manifests and lockfiles only.
2. **Fail closed.** For an ecosystem, if any config file that could name a registry exists but cannot be
   read, is over 1 MiB, or cannot be parsed, every dependency of that ecosystem is `Private(CustomRegistry)`.
   The same applies to an npm-family repo where a repo or user `.npmrc` / `.yarnrc*` sets any `registry` key
   that could not be resolved to a host.
3. **Local-name set.** Collect the package names declared by every manifest in the repo: `package.json`
   `name`, Cargo `[package].name`, `project.name` / `tool.poetry.name`, csproj `PackageId`, and go.mod
   `module`. Any dependency whose normalised name is in that set is `Local(Workspace)`, whatever its source.
4. **List every config path and env var as a §0 row** marked VERIFIED or ASSUMED, per OS. Today none of
   them appear there.
5. **Extend §9 AC6.** Add sentinels for:
   - an **ignored** `.npmrc` scope, with a pnpm-v9 lock that has no tarball URLs;
   - an env-only registry, one per ecosystem;
   - a yarn v1 workspace member;
   - an unparseable `nuget.config`. This one must come out Private (fail-closed).

**Confidence:** high on the discovery gap (it is literally the rule on line 336). Medium-high on pnpm
lockfile host omission.

### HIGH-2 — `yaml-rust2` expands aliases without limit on repo-controlled lockfiles (§1, §4.2, §7.5)

**Scenario.** A hostile repo commits a small `pnpm-lock.yaml` (or a yarn-berry `yarn.lock`) that uses
nested anchors and aliases, the YAML form of "billion laughs". This is the minimum proof. No payload is
given here.

`yaml_rust2::YamlLoader`'s `on_event` handles `Event::Alias(id)` by
`self.anchor_map.get(&id) … v.clone()`, which deep-copies the anchored subtree. Upstream `src/yaml.rs`
has no limit on expansion count or depth. So:
- memory grows exponentially in the nesting level;
- the 32 MiB **input** cap in §4.2 bounds nothing;
- `catch_unwind` does not catch the allocator's OOM abort;
- `CHECK_DEADLINE` (§5.1) has no enforcer inside one synchronous parse.

With the tracker enabled, the parse runs automatically on open (§6.3 "Opened"). Bonsai reopens the last
repo on launch, and the aborted check never wrote a cache (`last_attempt_ms` stays `None`), so
**relaunching crashes again**. The user is stuck in a loop until they edit settings by hand.

§7.5's "no entity expansion" line covers XML only. The contract says nothing about YAML aliases.

**Amendment (contract, before P125b):**
- Parse YAML with the `yaml_rust2::parser::Parser` event API, not `YamlLoader`.
- **Reject any `Event::Alias` and any anchor** with `ParseError("unsupported YAML feature")`. Real
  pnpm-lock and yarn-berry files do not use them; P125b should confirm this on a corpus of real lockfiles
  and record it in §0 as ASSUMED until then.
- Cap nesting depth (e.g. 64) and total node count (e.g. 2 M) in the event receiver.
- Require every in-house parser (yarn v1, requirements, PEP 440) to be iterative or depth-capped.
- Add `deep_nesting` tests for TOML. `toml_parser` 1.x recursion limits were not verified here.
- Add fixtures `yaml_alias_bomb ⇒ ParseError in < 100 ms` and `yaml_deep_nesting ⇒ ParseError`.
- Specify that `CHECK_DEADLINE` is enforced by the async caller (timeout, then `CancelToken`, then epoch
  discard), not by the blocking worker.

**Confidence:** high. The upstream source was read.

### MEDIUM-1 — User-level registry configs are read whole, and their values have no containment spec or test (§4.4, §9 AC6, Q6)

`~/.npmrc` (`_authToken`, `_auth`), `pip.conf` (`index-url = https://user:token@…`), `NuGet.Config`
(`<packageSourceCredentials>` with `ClearTextPassword`) and `$CARGO_HOME/config.toml` (`[registry] token`)
routinely hold credentials. The contract says "never sent, logged, or placed in the report", but gives
no mechanism:
- `HostEnv::read_user_file -> Option<String>` hands the **entire file** to the pipeline.
- Nothing forbids `Debug`/`Serialize` on `RegistryContext`.
- `RegistryRef::Host(String)` does not require userinfo to be stripped.
- The failure path is unspecified. **`toml::de::Error`'s `Display` renders the offending source line with
  a caret.** An implementer who maps a config or manifest parse error with `e.to_string()` into
  `ParseError.message` (or into a `CheckError::Internal(String)` that is then logged) copies a token line
  into the report, which goes to the cache file on disk and to IPC.

The token never leaves the machine through this path, which is why this is MEDIUM rather than HIGH. It
does land in plaintext files the user was told do not contain it.

**Amendment (contract, before P125b; the derive policy before P125a):**
- **Minimum-read design.** Each config is parsed inside `registry_ctx::load` by a key-filtered extractor:
  - npm: only `registry` and `@scope:registry`;
  - pip: only `index-url` / `extra-index-url`;
  - NuGet: only `packageSources`, `disabledPackageSources` and `packageSourceMapping`;
  - Cargo: only `source.*.replace-with` and `registries.*.index`.

  The extractor keeps only the **host** (use a real URL parser and drop userinfo, path and query) plus
  scope names, then drops the raw `String` before `load` returns. Credential keys and sections are never
  deserialized. Use a raw-line skip for `.npmrc`, and do not model `packageSourceCredentials` in the XML DTO.
- `RegistryContext` has **no** `Debug` / `Serialize` / `Clone`. Its fields are hosts and bools only.
- Registry-config parse failures take the HIGH-1 fail-closed path. They are never `ParseError` groups
  and never logged with content.
- `ParseError.message` is limited to a fixed category plus `line:col`. It never contains a parser's
  `Display` output (this also applies to repo manifests, §7.5).
- `CheckError` variants carry enums, not `String`. §5.3 already limits the message to category plus
  status, so make that structural.
- **Token sentinel test.** Put `zz-sentinel-token` in every `FakeHostEnv` user config and in
  `PIP_INDEX_URL=https://u:zz-sentinel-token@…`. Assert the sentinel is absent from:
  - outbound URLs and bodies;
  - the cache file bytes;
  - the serialized `DepTrackerView`;
  - `format!("{:?}")` of every `ParseError` / `CheckError` produced.

### MEDIUM-2 — The "structural" D7 guarantee has a public string→sendable constructor (§7.3)

`AdvisoryId::parse(&str)` is `pub`, and its regex `^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$` accepts most
private package names (`zz-sentinel-reg`, `corp-billing`). `OutboundRequest::osv_vuln(&AdvisoryId::parse(&dep.name)?)`
type-checks, and the contract's compile-time claim does not cover it. `PageToken` has the same shape.
The `compile_fail` doctest only proves that a `PublicPackage` struct literal fails.

**Amendment (before P125a):**
- `AdvisoryId` and `PageToken` are constructible only inside `remote/osv.rs`, from a deserialized OSV
  response DTO. Make the constructor `pub(in crate::remote)`, or make the DTO deserialize directly into
  the newtype.
- Add a second `compile_fail` doctest: `AdvisoryId::parse("x")` outside `remote` fails.
- Reuse from `ctx.prev` (§5.1 step 10) must compare strings, never re-mint an `AdvisoryId` from cache data.
- State in §7.3 that private names *do* appear in the local view and cache as "private — not checked"
  rows, by design. D7's boundary is the socket only.

### MEDIUM-3 — Repo file reads follow symlinks and special files, with a metadata-only size cap (§4.1, §4.2)

**Scenario.** On macOS or Linux, where git checks out symlinks, a hostile repo tracks
`package.json -> /dev/zero`, or a FIFO, or `requirements.txt -> ../../<some user file>`.
- `stamps` and the 32 MiB cap use `fs::metadata`, which follows the link. `/dev/zero` reports `len 0`,
  so a `read_to_string` never finishes (OOM), and a FIFO blocks the worker forever. Either one repeats
  on every open.
- A symlink out of the workdir makes a user file get parsed as a manifest. Any line that passes
  `valid_public_name` is then sent to deps.dev.
- Index paths containing `..` or absolute components are not rejected by the contract.

The attacker cannot receive this data, because the hosts are fixed. The impact is DoS plus local data
being sent to Google.

**Amendment (before P125b):**
- Skip index entries whose mode is symlink (`0o120000`) or gitlink.
- Open each file with `symlink_metadata`, require `is_file()`, and require the canonicalized path to sit
  under the canonicalized workdir.
- Read with `take(cap + 1)`, never with metadata-then-`read_to_string`.
- Reject paths containing `..`, a root or a drive prefix.
- `stamps` / `probe_stale` also use `symlink_metadata`.
- State that "`.cargo/config.toml` repo ancestors" means **in-repo ancestors only**. Cargo itself walks to
  the filesystem root, and the contract is ambiguous. `$CARGO_HOME` is covered separately.

### MEDIUM-4 — AC11 "never spawns a process" is bypassable through `bonsai-core` (§1, §7.4)

`bonsai-deps` depends on `bonsai-core` for `AppError` and `repo_key`, and `bonsai-core` exposes
process-spawning APIs:
- `ai/process.rs`, `git/exec.rs`, `git/exec_stream.rs`, `git/hooks_stream.rs`, `git/maintenance.rs`;
- `external.rs`, and `external_url::open_url` with its runner.

The clippy ban (`std::process::Command` types and `::new`) applies only to code *in* `bonsai-deps`.
The source scan greps for `process::Command`, `Command::new`, `tokio::process` and `CommandRunner`.
Calling `bonsai_core::git::exec::…` or `bonsai_core::external_url::open_url(&SpawnRunner, …)` from
`bonsai-deps` would pass both. AC11's proof would be green while the invariant is false. The concern is
a future diff, not today's code.

Transitive crates (reqwest, rustls-platform-verifier, git2 for open/index/status, serde, toml, quick-xml,
semver, and yaml-rust2 with its deps `arraydeque` / `hashlink` / `encoding_rs`) do not spawn processes on
the paths used.

**Amendment (before P125a — `no_process.rs` lands there):**
- `tests/no_process.rs` enforces an **allowlist of `bonsai_core::` paths**: `bonsai_core::error::AppError`
  and `bonsai_core::git::history_index::store::repo_key`. Any other `bonsai_core::` token under the
  scanned roots fails the test.
- Add `std::os::*::process`, `libc::` and `SpawnRunner` to the scanned tokens.
- Add a P125a done-criterion that snapshots `cargo tree -p bonsai-deps -e normal --prefix none` into a
  test fixture, so any new transitive crate shows up in review.

### LOW-1 — "At most one in-flight request after disable" is wrong (§6.4)

With `DEPSDEV_PARALLEL = 6`, up to 6 deps.dev GETs (or 1 OSV POST plus 5 vuln GETs) can be in flight
when the user disables. Each can take up to the 30 s request timeout. `GatedTransport` correctly stops
new sends, and the lock/epoch rule correctly stops persistence. **Amendment:** change the text, and the
consent copy if it repeats the claim, to "up to `DEPSDEV_PARALLEL` requests already in flight complete;
none start". Suppress `Progress` emits whose epoch is stale. AC12 should block all 6 workers in the
FakeTransport.

### LOW-2 — Dot-segment names reach URL paths (§7.3 `valid_public_name`, §7.1, §7.6)

The NuGet regex `^[A-Za-z0-9._-]{1,100}$` accepts `..`. The Go regex is satisfied by a first element of
`..`, because it "contains a dot". The URL crate normalizes `/packages/..` (and `%2E%2E`) to a different
deps.dev endpoint, and the `pkg.go.dev` link normalizes the same way. The host stays the same, so the
impact is correctness only. **Amendment:** `valid_public_name` rejects any name, or any `/`-separated Go
element, that is `.` or `..`. Also require the Go first element to contain a dot *and* a letter or digit.

### LOW-3 — Display-string hygiene for rows that bypass validation (§2.1, §7.5)

`Local`/`Private` rows show the raw repo-derived `name`/`declared`/`current`, and `manifest_path` and
`ParseError.file` are unbounded. `summary` strips only control characters: Rust's `is_control` covers
Unicode Cc but not Cf, so it misses bidi overrides such as U+202E. **Amendment:**
- cap every repo- or response-derived display string (e.g. 256 characters);
- strip Cc **and** Cf in all of them;
- state that `summary` is rendered as plain text, never as markdown or HTML;
- cap unique advisory ids per check (e.g. 5 000), manifests per repo (e.g. 2 000) and total bytes parsed
  (e.g. 256 MiB). Exceeding a cap gives `Service` or a `ParseError` group.

### LOW-4 — ASSUMED API facts that affect the security signal (§0, §3.3)

- If the ASSUMED Go `v`-prefix rule is wrong, every Go row reports `vulns_checked: true` with zero
  advisories. That is a silent false "no vulnerabilities".
- The `database_specific.severity` assumption falls back safely to CVSS and then to `Unknown`.
- The yanked-as-latest gap (§3.3) could point users at a version pulled for cause. It is already flagged.

**Amendment:** P125d cannot close until its recorded fixtures include one **known-vulnerable** pinned
version per ecosystem, and a test asserts at least one advisory for each. For Go, that is a module
version with a published GO-/GHSA- id.

### INFO

- `yaml-rust2` has no RustSec entry in either local advisory DB (latest 2026-09-23). The crate it forks,
  `yaml-rust`, is flagged unmaintained there, and so are `serde_yaml` / `unsafe-libyaml`.
- `yaml-rust2` adds `arraydeque`, `hashlink` and `encoding_rs`, all within the `deny.toml` license
  allowlist. Recommend `default-features = false` to drop `encoding_rs`.
- Add `yaml-rust`, `serde_yaml`, `serde_yml` and `unsafe-libyaml` to `[bans].deny`.
- reqwest honours `HTTPS_PROXY`. A TLS-inspecting corporate proxy trusted by the OS store will see the
  public names and versions. That is the user's own network; consider mentioning it in the consent copy.
- The report (which includes private rows) must not become input to the AI or MCP features without its
  own consent. There is no such path in this contract.

---

## Remediation, in the order it should be done

1. **Before P125a:**
   - HIGH-1 items 2–5: fail-closed `classify`, the local-name set, the §0 rows, and the sentinel design.
   - MEDIUM-2: private `AdvisoryId`/`PageToken`.
   - MEDIUM-4: the `bonsai_core::` allowlist and the dependency snapshot.
   - LOW-2: dot segments.
   - The MEDIUM-1 derive policy on `RegistryContext`.
2. **Before P125b:**
   - HIGH-1 item 1: filesystem config lookup that includes ignored files.
   - HIGH-2: event parser, no aliases, depth and node caps.
   - MEDIUM-1: the extractor and the token sentinel.
   - MEDIUM-3: the no-follow bounded read.
3. **Before P125d closes:** LOW-4 known-vulnerable fixtures; LOW-3 caps; the text of `CheckError` and
   `ParseError.message`.
4. **With P125e:** the LOW-1 text and a 6-worker AC12 test.

## Status

**Verdict: amend first.** Once the listed amendments are in, the contract's remaining design (fixed
hosts, the type-gated transport, the epoch/lock disable, no CSP change) is sound. The §7-mandated
security-auditor passes on the P125d and P125e diffs still apply.

---

## Re-check 2026-09-28 (rev 3)

**Inputs:** `docs/contracts/P125-dependency-tracker.md` rev 3 (888 lines) and `docs/contracts/P125-ecosystems.md`
(E3 inventory, E4 test table). I read bounded ranges around every `[HIGH-*]` / `[MEDIUM-*]` / `[LOW-*]` /
`[Q7]` tag: §0–§1, §2.1–2.2, §4.1–4.5, §5.1, §6.4–6.5, §7.3–7.6, E3, E4. I also checked
`crates/bonsai-core/src/git/repo.rs:132` (`path_ignorecase`) for deviation (a).

### Per-finding status

| Finding | Status | Evidence / what is missing |
|---|---|---|
| HIGH-1 item 1 (config lookup includes ignored files) | **resolved** | §4.1 "Registry configs are NOT taken from `FileSet`"; §4.4 `load` runs a `safe_read` ancestor walk from each manifest dir to the workdir. |
| HIGH-1 item 2 (fail-closed) | **resolved** | `PrivateReason::ConfigUnreadable`. Unreadable, TooLarge, NotAFile, a failed `extract_host` or a config syntax error all set `eco.fail_closed`. `classify` checks this before any Public path (§7.3). |
| HIGH-1 item 3 (local-name set) | **resolved** | `declared_names` + `LocalNames`, checked first in `classify`. |
| HIGH-1 item 4 (inventory) | **resolved as a contract item** | E3 lists every path and env var per OS. Every row is ASSUMED, and P125b/c1/c2 must flip them to VERIFIED (a gating obligation, correctly stated). pnpm ≥ 10 settings in `pnpm-workspace.yaml` are not in E3; P125b should confirm whether a registry key can live there. |
| HIGH-1 item 5 (sentinels) | **partially resolved** | Cases (a)–(f) cover the gaps I raised. **Missing:** the E4 AC6 matcher is a "case-insensitive substring in any URL or body", and deps.dev URLs are percent-encoded (`@zzign/pkg` → `%40zzign%2Fpkg`). A full-string sentinel match on a scoped name can therefore **pass while the name leaks**. See NEW-1. |
| HIGH-2 (YAML alias bomb) | **resolved** | §1 and §4.2: event `Parser` only, `YamlLoader` banned, alias or anchor ⇒ `UnsupportedFeature`, depth 64 and node cap 2 M. The TOML depth pre-scan is added. The deadline is enforced by the async caller (§6.4). Bomb tests are in E4. The "no anchors in real lockfiles" assumption is recorded in §0 as ASSUMED. |
| MEDIUM-1 (credentials) | **resolved** | `RegistryContext` has no Debug/Serialize/Deserialize/Clone. The key-filtered `extract_keys` never stores token lines. `extract_host` uses a real URL parser and keeps the host only. `ParseError` has no free text and never calls `Display` on a parser error. `CheckError` has no String payloads, and `CheckFailure.message` is table-rendered. E4 6′ is the token sentinel, covering the Q7 configs too. Two small leftovers are in NEW-2. |
| MEDIUM-2 (`AdvisoryId`/`PageToken`) | **resolved** | They are minted only by private `mint_*` functions in `remote/osv.rs` from DTO fields. There is a second `compile_fail` doctest. Cache reuse compares strings and never mints an id from cache data. |
| MEDIUM-3 (symlinks / special files / traversal) | **resolved** | Index modes `0o120000`/`0o160000` are skipped. `path_is_clean` is applied. `safe_read` does `symlink_metadata`, then a regular-file check, then canonical containment, then a re-check after open, then `take(cap+1)`. `safe_stat` is used for stamps. fsread tests are in E4. |
| MEDIUM-4 (no-process bypass via bonsai-core) | **resolved** | The `bonsai_core::` allowlist expands `use` groups. The token list is extended. `dep-tree.txt` becomes a P125a done-criterion. See NEW-3 and NEW-4 for two nits. |
| LOW-1 | **resolved** | §6.4 now says up to 6 requests in flight, stale-epoch `Progress` is suppressed, and the test blocks all 6 workers. |
| LOW-2 | **resolved** | `.`/`..` elements are rejected for every ecosystem, and the Go first element needs a dot plus a letter or digit. Tests are in E4. |
| LOW-3 | **resolved** | `clean_display` strips Cc and Cf with length caps. `summary` is plain text. MAX_MANIFESTS, MAX_PARSE_BYTES and MAX_ADVISORIES are set. |
| LOW-4 | **resolved (gated at P125d)** | A known-vulnerable fixture per ecosystem, with ≥ 1 advisory asserted. The Go fixture closes the `v`-prefix row. |

### Architect deviations

- **(a) `path_ignorecase` in the allowlist — acceptable.** `repo.rs:132` is `git2::Repository::open_ext(NO_SEARCH)` followed by a
  `core.ignorecase` config read. It runs in-process and spawns nothing. **Nit:** §6.5 puts the key computation in
  `bonsai-deps/src/cache.rs`, but the allowlist admits `path_ignorecase` only under `src-tauri/src/deps/`. State that
  src-tauri computes `repo_key` and passes it to `cache::load/save`. Otherwise P125d fails its own scan.
- **(b) `registryHost` and `transitiveChecked` — acceptable.**
  - `registry_host` is the `extract_host` output: host only, userinfo/port/path/query dropped, then `clean_display`.
  - It is never made into a link (§7.6). It appears only in the local IPC view and cache.
  - It is not a dependency name, so D7's socket boundary is unaffected. E4 6′ asserts it contains no `@`.
  - `transitive_checked` is a count.
- **(c) Q7 above-workdir walk — does not reopen MEDIUM-3 or MEDIUM-1.**
  - It reads config file names only; manifests and lockfiles stay bounded to the workdir, with an E4 test.
  - Reads are no-follow on the final component, regular-file-only, re-checked after open, and capped at 1 MiB via `take`.
  - They go through the same `extract_keys` and `extract_host`, and the Q7 configs carry the token sentinel.
  - Every signal the extractors can produce only withholds dependencies. The one widening primitive is NuGet
    `<clear/>`, whose closest-wins precedence mirrors NuGet's own restore semantics, so a dependency is treated
    as public only when NuGet itself would restore it from nuget.org.
  - Parent directories are the user's own filesystem (outside the threat model). Symlinked *intermediate*
    directories are therefore acceptable.
  - Residual (INFO): a permission-denied ancestor makes the whole ecosystem fail closed, i.e. every Cargo/NuGet
    row shows `ConfigUnreadable`. This errs in the safe direction. P125c1/c2 should treat `NotFound` and
    `PermissionDenied` on a *missing* file as Absent only if the tool itself does (UNVERIFIED).

### New findings introduced or surfaced by rev 3

- **NEW-1 (LOW, MUST-amend: one line in E4 row 6) — the sentinel matcher is defeated by percent-encoding.**
  - Required matcher: percent-decode every recorded URL path and query, and JSON-decode every body, then run the
    case-insensitive search.
  - Or: build every sentinel around a bare marker (`zzsentinel`, `zzign`, `zzenv`, `zzws`) and match on that
    marker alone.
  - Without this, cases (a) `@zzign/pkg`, (b) `npm_config_@zzenv…` and (c) `@zzws/b` can leak undetected.
- **NEW-2 (LOW) — raw config text still passes through two types that could derive `Debug`.**
  - `UserRead::Content(String)` and `ReadOutcome::Content(String)` hold whole `.npmrc` / `pip.conf` text transiently.
  - `HostEnv::vars_with_prefix_ci("npm_config_")` returns *all* `npm_config_*` values, including
    `npm_config_//host/:_authToken`.
  - Amendment: give these types no `Debug`, or a redacting `Debug` that prints `Content(<n bytes>)`.
  - Have `vars_with_prefix_ci` take the key predicate and return only matching keys, so token-bearing env
    values are never materialised in a `Vec`.
  - This can land in P125b.
- **NEW-3 (LOW) — `dep-tree.txt` snapshot portability.**
  - `cargo tree` without `--target` resolves the host target only. git2/reqwest pull platform-specific crates
    (e.g. `openssl-sys` on Linux if any feature enables it).
  - So a byte-identical check will differ between Windows, macOS and Linux CI.
  - Amendment: `--target all` (or one pinned triple) plus `-e normal --prefix none`, sorted.
- **NEW-4 (INFO) — a blocking thread keeps running after a deadline hit.**
  - After a `CHECK_DEADLINE` timeout the blocking thread runs until its next `cancel` check.
  - With the rev 3 caps this is bounded, so it is acceptable as is.
  - Recommendation: make "a new `start_run` waits for, or refuses while, a zombie from the previous epoch or token
    is still alive" explicit. It is not required.
- **Q8 (INFO).** The neutral consent copy is fine from a security standpoint, provided it still names both hosts
  (`api.deps.dev`, `api.osv.dev`) and the fields sent (§7.1). That belongs to the ui-designer review.

### Re-check verdict

**OK to implement P125a once NEW-1 (the one-line E4 sentinel-matcher fix) is in the contract.** Every
item in the audit's P125a list is resolved.
- **Gating P125b:** NEW-2, which must land with `registry_ctx.rs` / `fsread.rs`, and the obligation to verify the
  E3 rows, which the contract already states. HIGH-2 and MEDIUM-3 are fully specified for P125b.
- **Fold into P125a's done-criterion:** NEW-3.
- **Fix before P125d:** the deviation (a) nit.

---

## Re-check 2026-09-28 (Q9 / P125c1)

**Scope:** the Q9 ruling (contract §11 end; §4.4/§4.5 `[Q9]` lines) and the uncommitted P125c1 working tree
against baseline `615593b`. I read in full:
- `crates/bonsai-deps/src/fsread/follow.rs` and `src/fsread.rs`;
- `src/registry_ctx/{walk.rs,host_env.rs,load_go.rs}`, plus `load_npm.rs` 60–100;
- the key-filter entry points in `npm_cfg.rs`;
- `fake_env.rs` 150–175 and `follow_tests.rs` 25–50.

I did not read `load_cargo.rs`/`cargo_cfg.rs` bodies beyond confirming they route through `walk.rs`, and I
did not re-read the three sentinel test files. Nothing was executed.

**Important context:** the production `HostEnv` (the only non-test caller of `read_config_follow`) lands in
P125e. Today the follow path is reached only through `FakeHostEnv`. None of the findings below has a live
path yet. They gate P125e, not this commit.

### Q1 — what a repo- or parent-controlled symlink above the workdir can reach

"Parent-controlled" is realistic. If the user opens a **submodule** (or any nested repo) of a hostile
superproject as its own repo, the superproject's tracked files, symlinks included, sit *above* the opened
workdir. A single repo cannot place anything above its own workdir.

| Target | Result | Evidence |
|---|---|---|
| `\\srv\sh`, `//srv/sh`, `\\?\UNC\…`, `\\?\GLOBALROOT\…`, `\\?\Volume{…}`, `\\.\pipe\…`, `\??\…`, `//?/UNC/…` | **rejected before being touched** (NotAFile ⇒ fail-closed) | `is_network_path` runs on the start path, on every `read_link` target text and on the final path (`follow.rs:78,109,124`). Only `\\?\X:` passes. Tests are at `follow_tests.rs:25-50`. |
| NT-rooted `\Device\Mup\…`, drive-relative `C:foo` | rejected (Unreadable) | `push_components` refuses a rooted path that is not absolute. |
| `..` games, verbatim `//?/C:/../UNC/…` | contained | `..` is applied **lexically** to the already-resolved prefix and never passed to the OS. `PathBuf::pop` cannot rise above the prefix. The final `net(&out)` re-check covers it. |
| 8.3 names | no effect | They only alias local paths and cannot form a UNC prefix. |
| Mapped / `subst` drive letter (`Z:\…`) | **followed** | This reaches the network, but only a server the user has already mapped. An attacker cannot choose the host. INFO. |
| DOS device names (`NUL`, `CON`, `COM1`, `CONIN$`, including `C:\x\NUL` on this Win10 host) | probably fail-closed, **UNVERIFIED** | Rust's `is_file()` on Windows means "not a dir and not a symlink", so a device may pass the `lstat` gate. The handle's `GetFileInformationByHandle` should then fail ⇒ Unreadable. See Q9-4. |
| `/dev/zero`, FIFOs, sockets, char/block devices | rejected without open | The final `symlink_metadata().is_file()` check is at `follow.rs:141`. An intermediate component that is not a dir ⇒ NotAFile. |
| `/proc/self/fd/N` → `pipe:[…]` | fail-closed | The relative text gives NotFound behind a link ⇒ Unreadable. |
| `/proc/self/environ` and other procfs regular files | read (≤ 1 MiB) | The npm/Go extractors keep only registry keys, and the resulting hosts are local display only. **No value leaves the process.** |
| **macOS `/net/<host>/…` (autofs `-hosts`), `/afs/<cell>/…`, Linux autofs `-hosts`** | **followed ⇒ network access to an attacker-chosen host, plus a possible long hang** | `is_network_path` only knows Windows syntax. `lstat("/net/evil.example")` makes automountd contact `evil.example` (DNS, then portmap/NFS; AUTH_SYS carries uid/gid/hostname, not a secret). A hard-mount timeout can leave the blocking thread stuck well past `CHECK_DEADLINE`, which only abandons it. **See Q9-1.** Whether macOS ships `/net -hosts` enabled in current releases is UNVERIFIED; it historically did. |
| slow or hung NFS the user mounted | may hang the blocking thread | The host is not attacker-chosen. INFO. |
| OOM | bounded | `take(cap+1)` with a 1 MiB cap, 40 hops, and a component stack bounded by target text length. |

### Q2 — TOCTOU between resolution and re-open

**Acceptable under the threat model.**
- `read_config_follow` resolves, `lstat`s the target, then `File::open`s it, which follows links again.
- `read_regular` re-checks that the handle is a file.
- Exploiting the window needs a concurrent writer in a directory on the path, i.e. control of the local
  machine, which is outside the threat model. Checked-out repo content is static.

Optional hardening (LOW, Q9-5): on Unix, compare `(dev, ino)` of the resolved `lstat` with `f.metadata()`,
as `read_nofollow_under::same_file` already does. That closes the final-component swap. Intermediate-directory
swaps remain and are accepted.

### Q3 — can pnpm `auth.ini` or GOENV collect a token?

**No.**
- **`auth.ini`** goes through the npmrc extractor (`npm_cfg::apply_pair`). It keeps only a key equal to
  `registry` or ending in `:registry`. `//host/:_authToken`, `_auth` and similar lines are skipped unstored.
  A `${…}` placeholder in a registry key fails closed. URL values go through `host_of`/`extract_host`, which
  keeps the host only.
- **GOENV:** `file_values` matches only the listed keys: `GOPRIVATE`, `GONOPROXY`, `GONOSUMDB`, `GOINSECURE`,
  `GOPROXY` and `GOSUMDB`. `GOAUTH` and `GOFLAGS` are never read. `GOPROXY=https://u:tok@proxy.corp` yields
  only `proxy.corp`.
- In both cases the whole file exists transiently as `UserRead::Content`, whose `Debug` is redacted
  (`host_env.rs`).

### Q4 — is any Q9 change fail-open?

**No fail-open path in the committed-to-be code.** Specifically:
- `resolve` returns `Absent` only for NotFound on an original-path component *before any link was followed*.
- A dangling link, or EACCES / hop overflow / an unresolvable target, gives Unreadable. A network or device
  target gives NotAFile.
- `fake_env::from_outcome` maps everything except Absent/Content to Unreadable ⇒ `Closed` (`walk.rs`).
- Go takes the **union** of environment and file, and any non-public `GOPROXY`/`GOSUMDB`, or a host-less
  entry, ⇒ private/Closed.

The one latent risk is Q9-3.

### Findings

- **Q9-1 (MEDIUM, MUST-fix before P125e wires the real `HostEnv`) — the "reject network targets" constraint
  is Windows-only.**
  - **Scenario:** a hostile superproject tracks a symlink `.npmrc` (or `.cargo/config.toml`,
    `pnpm-workspace.yaml`) → `/net/attacker.example/x`. The user opens one of its submodules in Bonsai with
    the tracker enabled.
  - The above-workdir walk `lstat`s through `/net`, and the automounter dials `attacker.example`. That gives
    attacker-chosen egress (IP, uid/gid, hostname) and a possibly long blocked thread.
  - The Q9 ruling's constraint "reject … network-share targets" is not met on macOS/Linux.
  - **Fix (preferred: remove the primitive).** Follow symlinks only where the Q9 rationale lives: the fixed
    user/system config files, plus an above-workdir ancestor that *is* `$HOME` or `$CARGO_HOME`. In any other
    ancestor above the workdir, keep no-follow, so a symlinked config there ⇒ fail-closed. A hostile parent
    can then only withhold, which it can already do with a plain file. This narrows the Q9 ruling's wording,
    so the orchestrator must confirm it with the user.
  - **Fix (alternative, if the user keeps "follow everywhere"):**
    - On Unix, before `lstat`ing a child of `out`, refuse to descend when `out`'s `st_dev` differs from the
      start directory's and `$HOME`'s `st_dev`. That catches autofs, NFS, FUSE and AFS mount points before
      they trigger.
    - Also reject absolute target text under `/net/`, `/Network/`, `/afs/` and `/.autofs` as a second layer.
    - Add a unix-gated test: a symlink to a path under a different-device directory ⇒ NotAFile.
- **Q9-2 (LOW, functional; defeats the Q9 motivation) — any link on the path makes a later plain miss "dangling".**
  - `via_link` is sticky. If `~/.cargo` (or `~/.config`, or `/home` itself) is a symlinked **directory**,
    every absent file under it gives NotFound with `via_link = true`, i.e. Unreadable.
  - So the always-absent legacy `$CARGO_HOME/config`, or a missing pnpm `rc`, turns the whole ecosystem
    private. That is the exact outcome Q9 was ruled to avoid.
  - This fails safe.
  - **Fix:** treat NotFound as dangling only while consuming components that came from a link's *target
    text*. Push a marker after the target's components and clear `via_link` when it pops. Test: a symlinked
    `.cargo` dir with no legacy `config` ⇒ Absent.
- **Q9-3 (LOW, must be structural before P125e) — the NotAFile→Unreadable mapping lives only in the test double.**
  - `from_outcome` is private to `fake_env.rs` (`cfg(test)` / `test-support`).
  - If P125e's real `HostEnv` maps `NotAFile` or `TooLarge` to `Absent`, a network-rejected or oversize
    config is silently skipped and its private signals are lost. That is fail-open.
  - **Fix:** move `from_outcome` to `host_env.rs` as `pub(crate)`, used by both implementations, with an
    exhaustive match and no `_` arm.
- **Q9-4 (LOW, UNVERIFIED) — Windows DOS device names are not rejected by name.**
  - Reject any resolved component whose stem (before the first `.`, trailing spaces and dots trimmed) is
    case-insensitively `CON`, `PRN`, `AUX`, `NUL`, `COM0`–`COM9`, `LPT0`–`LPT9`, `CONIN$` or `CONOUT$`,
    including the superscript-digit forms, before `lstat`.
  - This host is Win10, where `C:\x\NUL` still maps to the device.
- **Q9-5 (LOW, optional)** — the Unix `(dev, ino)` handle comparison described under Q2.
- **INFO:**
  - A workdir on a UNC share (canonical `\\?\UNC\…`) fails every above-workdir config closed, so npm, Cargo
    and NuGet all show private. That is safe; document it in the P125-ui copy for "all private".
  - Windows cloud-file reparse points (OneDrive placeholders) are not `is_symlink`, and opening one hydrates
    it. They cannot be planted via git, so they are not attacker-steerable.
  - pnpm `auth.ini` and GOENV key filters are clean (Q3).

### Verdict (Q9)

**OK to commit P125c1 as a wip.** No production caller of the follow path exists until P125e, and nothing
here is fail-open today. **MUST-fix before P125e lands:**
- **Q9-1**, which needs the user to confirm the narrowed follow scope, or else the device-boundary alternative;
- **Q9-3**.

Q9-2 should ride along with Q9-1 (same function). Q9-4 and Q9-5 are optional hardening.
