# P125 — Dependency update tracker

**Status:** architect contract **rev 3**. Amended per `docs/audit-2026-09-28-P125-deps-contract.md`
(tags `[HIGH-1]` … `[LOW-4]`) and `docs/contracts/P125-ui.md` §11 (tags `[UI-a]` … `[UI-e]`). Rev 3
applies the Q7 ruling (tag `[Q7]`) and, for size, moves the config inventory, the test table, the
mock spec and the TS mirror into the companion file. The security-auditor passes on the P125d and
P125e diffs still apply.
**Spec:** `docs/specs/009-dependency-tracker/spec.md` (AC1–AC13, D1–D9). It is authoritative and not
repeated here.
**Companion:** `docs/contracts/P125-ecosystems.md` holds:
- E1 — version schemes;
- E2 — adapter rules + the not-tracked list;
- E3 — config inventory (§0.2);
- E4 — AC → test table (§9);
- E5 — mock fixtures (§8);
- E6 — TS mirror (§2.3).

**Line numbers drift. Resolve every citation by symbol first.**

**THE TWO INVARIANTS THIS MILESTONE BUYS.**
1. *No byte of a dependency that `privacy::classify` did not return as `Visibility::Public` can reach a
   socket.* The transport accepts only `OutboundRequest`. Its only constructors take `&PublicPackage`
   (constructible only in `privacy.rs`) or `&AdvisoryId` / `&PageToken` (constructible only in
   `remote/osv.rs`, from response data). See §7.3. `classify` **fails closed**: any registry signal
   it cannot read or resolve makes the dependency Private `[HIGH-1]`.
2. *The tracker never spawns a process.* `bonsai-deps` bans process APIs at lint time and by a source
   scan that allowlists its `bonsai_core` imports (§7.4). Git access is in-process git2.

**Amendment gating** (audit "Remediation"): **P125a** must contain HIGH-1 items 2–5 (classify
semantics, local names, §0.2 inventory, sentinel design), MEDIUM-2, MEDIUM-4, LOW-2, and the MEDIUM-1
derive policy. **P125b** must contain HIGH-1 item 1, HIGH-2, the MEDIUM-1 extractor and token
sentinel, and MEDIUM-3. **P125c1/c2** must contain Q7 (the Cargo / NuGet above-workdir config walk).
**P125d** must contain LOW-3 and LOW-4. **P125e** must contain LOW-1.

---

## 0. Verified vs assumed

### 0.1 External APIs (checked 2026-09-28)

| Fact | Status | Source |
|---|---|---|
| deps.dev base `https://api.deps.dev`, `GET /v3/systems/{system}/packages/{name}` → `versions[]{versionKey, publishedAt, isDefault, isDeprecated, deprecatedReason}` | VERIFIED | docs.deps.dev/api/v3, `api/v3/api.proto` |
| Systems `GO, RUBYGEMS, NPM, CARGO, MAVEN, PYPI, NUGET`; path params percent-encoded (`/` → `%2F`) | VERIFIED | docs.deps.dev/api/v3 |
| "Clients are expressly permitted to cache data served by the API"; Google API ToS applies; keyless | VERIFIED | docs.deps.dev/api/v3 |
| **v3 has NO batch endpoint.** Batching (`/v3alpha/versionbatch`, ≤5000) exists only in `v3alpha` (unstable) | VERIFIED | docs.deps.dev/api/v3alpha |
| `isDefault` = "the version installed when no version is specified … commonly the greatest version number, ignoring pre-release versions". **v3 has no yanked/unlisted/retracted field.** | VERIFIED (and a gap, §3.3) | `api.proto` |
| **Yanked ⇒ `isDeprecated: true`, `isDefault: false`** (§3.3 probe). crates.io: `deprecatedReason: "yanked"` (serde 0.7.6 / 1.0.31 / 1.0.95, tokio 0.1.12 / 1.0.0 / 1.16.0 / 1.39.0, libc, time, chrono, syn, log, rand all show yanked versions this way). PyPI: requests 2.32.0 / 2.32.1 carry `"Yanked due to conflicts with CVE-2024-35195 mitigation"`. Both `select_latest` fallback branches already filter `!is_deprecated`, so a yanked version is never picked: the §3.3 residual is closed for Cargo and PyPI. Q1's revisit trigger (deps.dev flags a yanked version `isDefault`) did not fire — **Q1 stays as ruled**. npm / Go / NuGet yank semantics were not probed. | VERIFIED (P125d live probe 2026-09-28; fixtures `tests/fixtures/remote/depsdev/{cargo/tokio,pypi/requests}.json`) | api.deps.dev |
| deps.dev canonicalises names in `versionKey.name`: PyPI PEP 503 (`Django` ⇒ `django`), NuGet lowercase (`Newtonsoft.Json` ⇒ `newtonsoft.json`); npm (`@types/node`), Cargo (`serde_json`) and Go names come back as sent. The §7.5 name-mismatch check therefore compares PyPI normalised, NuGet ASCII-case-insensitively, the rest exactly. Scoped / slashed names work percent-encoded (`%40types%2Fnode`, `github.com%2Fgorilla%2Fwebsocket`). Responses came back with no `Content-Encoding` (consistent with no decompression feature). | VERIFIED (P125d live probe) | api.deps.dev |
| OSV `POST https://api.osv.dev/v1/querybatch`, body `{queries:[{package:{name,ecosystem},version,page_token?}]}` → `results[]{vulns[]{id,modified}, next_page_token?}`. Returns **ids + modified only**. | VERIFIED | osv.dev post-v1-querybatch |
| OSV pagination: a per-query `next_page_token` when a query has >1000 vulns or the batch has >3000; re-send only the paged queries | VERIFIED (docs). **Not observed live:** a 1000-query batch returning 7565 vulns carried 0 `next_page_token`s (P125d probe), so the pagination tests use hand-crafted tokens | same |
| OSV querybatch `modified` has µs precision (`…04.067984Z`) while `/v1/vulns/{id}` has ns (`…04.067984836Z`) for the same record. `Advisory.modified` stores the **querybatch** value, the §5.1 step 10 reuse key (else reuse never fires) | VERIFIED (P125d live probe) | api.osv.dev |
| OSV `package.name` is **case-sensitive for NuGet**: `Newtonsoft.Json@12.0.3` ⇒ GHSA-5crp-9r3c-p9vr, `newtonsoft.json@12.0.3` ⇒ `{}`. `NEWTONSOFT.JSON` ⇒ `{}` (re-probed). **`packages.lock.json` keys carry the canonical nuspec casing** even when the project writes `Include="newtonsoft.json"` (hand-built scratch project, .NET SDK 10.0.303 `dotnet restore` against nuget.org only ⇒ key `"Newtonsoft.Json"`). `[P125d review, MEDIUM-D1]` a locked NuGet dependency is therefore sent in the lockfile's casing (when every TFM agrees); with no such casing, a row with zero OSV hits gets `vulnsChecked: false` ("not verified", never "clean"). `obj/project.assets.json` is not read (not in E2) | VERIFIED (P125d live probes) | api.osv.dev, api.nuget.org (restore only) |
| OSV: "no limits on the API"; 32 MiB response cap on HTTP/1.1 | VERIFIED | osv.dev/api |
| OSV ecosystem strings `npm`, `crates.io`, `PyPI`, `Go`, `NuGet`; PyPI `name` is normalized | VERIFIED | ossf osv-schema |
| OSV `severity[]{type: CVSS_V2|CVSS_V3|CVSS_V4|Ubuntu, score: vector}`; `database_specific` is free-form | VERIFIED | osv-schema |
| **OSV.dev operator `[UI-a]`:** the code is at `github.com/google/osv.dev`, whose README says it "contains all the code for running https://osv.dev on GCP". The docs site is `google.github.io/osv.dev`, and the launch was announced on security.googleblog.com. **No page found that explicitly says who operates the service.** | VERIFIED-INDIRECT (Google-developed, runs on GCP; no explicit operator statement) | github.com/google/osv.dev, google.github.io/osv.dev/faq |
| Max queries per querybatch = **1000** | VERIFIED (P125d live probe 2026-09-28: 1000 queries ⇒ 200; 1001 and 5000 ⇒ 400 `{"code":3,"message":"too many queries"}`) → `OSV_BATCH_MAX` const | api.osv.dev |
| `GET https://api.osv.dev/v1/vulns/{id}` returns the full OSV record | VERIFIED (exists); shape per osv-schema | osv.dev/api |
| GHSA-sourced records carry `database_specific.severity ∈ {LOW,MODERATE,HIGH,CRITICAL}` | VERIFIED as observed (P125d: GHSA-35jh-r3h4-6jhm and GHSA-5crp-9r3c-p9vr carry `"HIGH"`). GO- / PYSEC- records can carry neither `severity[]` nor `database_specific.severity` (GO-2020-0019, PYSEC-2018-28; the latter has no `summary` either) ⇒ `Unknown`; RUSTSEC-2021-0003 has only a `CVSS_V3` vector (9.8 ⇒ Critical) | api.osv.dev |
| OSV Go versions are queried **without** the `v` prefix; deps.dev GO versions carry `v` | **Superseded, VERIFIED** (P125d live probe): deps.dev GO versions carry `v` (`v0.42.0`), and OSV accepts **both** forms with identical results (`golang.org/x/text` `v0.3.7` ≡ `0.3.7` ⇒ GHSA-69ch-w2m2-3vjp, GO-2022-1059, GO-2026-5970; `v0.3.8` ≡ `0.3.8` ⇒ GO-2022-1059 correctly dropped, so the comparison is semantic). The pipeline sends the go.mod version as-is (`v1.4.0`). Closed by the `[LOW-4]` fixture `github.com/gorilla/websocket v1.4.0` ⇒ GO-2020-0019 (+ GHSA ids). | api.osv.dev, api.deps.dev |
| deps.dev accepts a lowercase system in the path (`/v3/systems/npm/…`) | VERIFIED (P125d live probe; `NPM` works too) | api.deps.dev |
| Real `pnpm-lock.yaml` / yarn-berry `yarn.lock` files use no YAML anchors or aliases | VERIFIED on the local corpus (2026-09-28, P125b) `[HIGH-2]`: 33 `pnpm-lock.yaml` (32 × v9.0, 1 × v5.4) + 1 berry `yarn.lock` (`__metadata.version` 8) through the `yaml_events` reader, **0 aliases, 0 anchors**. The 24 yarn-v1 locks found also parse. **Not covered:** pnpm v6/v7 and berry < 8 (no samples). Re-run with `yaml_events::corpus_tests::yaml_corpus` (ignored; `BONSAI_YAML_CORPUS`). | local repos (angular, components, bonsai, …), incl. `node_modules` |

**Deviation from the research leaning (ruled, Q1):** deps.dev + OSV only, with no native-registry
fallback. deps.dev lookups are parallel single GETs on stable v3 (6 in flight), not v3alpha batch.
D8 is met by OSV `querybatch` plus bounded parallelism.

### 0.2 Registry-config inventory `[HIGH-1 item 4]` `[Q7]`
Every config path and env var read, per OS, including the Q7 above-workdir walk for Cargo and NuGet,
is listed in `P125-ecosystems.md` **E3**. All rows are ASSUMED until P125b/c1/c2 verify them.

---

## 1. Module layout

**New crate `crates/bonsai-deps`** (ruled deviation from "likely bonsai-core"). Add it to the root
`Cargo.toml` `members`. Dependencies:
- `bonsai-core` — only `error::AppError` and `git::history_index::store::repo_key` `[MEDIUM-4]`;
- `git2` (ws), `serde`, `serde_json`, `toml`, `semver`, `quick-xml`, `url` (all already in the tree;
  `url` is used for host extraction `[MEDIUM-1]`);
- **`yaml-rust2` with `default-features = false` (NEW)**, used **only through the `parser::Parser`
  event API — `YamlLoader` is banned** `[HIGH-2]`;
- `reqwest` + `rustls` with bonsai-forge's exact feature set.

`deny.toml` `[bans].deny` += `yaml-rust`, `serde_yaml`, `serde_yml`, `unsafe-libyaml` (audit INFO).
New crate for the audit gate: `yaml-rust2`.

| File | Responsibility | Must not contain |
|---|---|---|
| `lib.rs` | `#![forbid(unsafe_code)] #![deny(clippy::disallowed_types, clippy::disallowed_methods)]`; re-exports; `run_check` | parsing, HTTP |
| `model.rs` | IPC + cache DTOs (§2), `summarize`, `clean_display` (§7.5) | other logic |
| `privacy.rs` | `PublicPackage`, `Visibility`, `LocalNames`, `classify` (§7.3) | HTTP |
| `registry_ctx.rs` | `RegistryContext::load` plus the key-filtered extractors (§4.4) | HTTP, process, `Debug`/`Serialize`/`Clone` on the context |
| `fsread.rs` | `safe_read`, `safe_stat` (§4.5) | parsing |
| `discover.rs` | `discover_full`, `probe_stale`, `stamps` (§4.1) | parsing |
| `yaml_events.rs` | bounded YAML event reader shared by pnpm/berry (§4.2) | `YamlLoader` |
| `catalog.rs` | **static data only**: not-tracked table, public-host allowlists, registry-page URL templates | IO |
| `adapter.rs` | `EcosystemAdapter`, `ParsedGroup`, `Dependency`, `LockEntry`, `DepSource`, `ADAPTERS` | ecosystem specifics |
| `eco/npm.rs`, `eco/npm_lock.rs`, `eco/pnpm_lock.rs`, `eco/yarn_lock.rs`, `eco/cargo.rs`, `eco/go.rs`, `eco/pypi.rs`, `eco/pypi_lock.rs`, `eco/nuget.rs` | one format family each (`P125-ecosystems.md` E2) | — |
| `version/{mod,semver_s,pep440,go,nuget}.rs` | §3 + E1 | — |
| `remote/transport.rs` | `Transport`, `ReqwestTransport`, `OutboundRequest`, bounded read (§7.2) | package logic |
| `remote/depsdev.rs` | deps.dev request builder + DTOs | `Dependency`, `LockEntry` |
| `remote/osv.rs` | OSV builders + DTOs; **the only home of `AdvisoryId` / `PageToken`** `[MEDIUM-2]` | `Dependency`, `LockEntry` |
| `cvss.rs` | CVSS 3.x base score | — |
| `check.rs` | pipeline (§5.1), `CancelToken`, `CheckError` | tauri |
| `cache.rs` | `CachedReport` load/save (tmp + rename) | — |
| `clippy.toml` | `disallowed-types = ["std::process::Command", "tokio::process::Command"]`, `disallowed-methods = ["std::process::Command::new"]` | — |
| `dep-tree.txt` | committed `cargo tree` snapshot `[MEDIUM-4]` (§7.4) | — |
| `tests/no_process.rs`, `tests/privacy_sentinel.rs`, `tests/token_sentinel.rs`, `src/**/*_tests.rs`, `src/fixtures/*.rs` (`cfg(test)`) | §9 | — |

**src-tauri** — new `src-tauri/src/deps/` (`mod.rs` state + hooks, `plan.rs`, `exec.rs`,
`plan_tests.rs`, `exec_tests.rs`) and `src-tauri/src/commands/deps.rs`. Changed:
- `settings/prefs.rs` (`DepTracker`), `settings.rs`, `settings/clamp.rs` (`clamp_dep_tracker`);
- `commands/ui_settings.rs` (`UiSettings`, `UiSettingsPatch`, `apply_patch`, the `set_ui_settings` hook);
- `commands/repo.rs` (`set_active_repo`, `close_repo`, the watcher `on_change` closure in `open_repo`);
- `scheduler/exec.rs::run_scheduler` (one call per tick);
- `lib.rs` (`.manage(DepsState)`, `generate_handler!`, startup wipe);
- `commands/registration_tests.rs`, `settings_defaults_parity_tests.rs`, `src-tauri/Cargo.toml`.

**Frontend** — new files:
- `src/ipc/types/depTracker.ts`;
- `src/ipc/types/ipc-api-deps.ts` (`IpcApiDeps`; `IpcApi extends … IpcApiDeps`, because `ipc-api.ts`
  may only shrink);
- `src/ipc/tauri/depTracker.ts`;
- `src/ipc/mock/handlers/depTracker.ts`, `src/ipc/fixtures/depTracker.ts`;
- `src/hooks/useDepTracker.ts`.

Changed: `src/ipc/types/{settings,index}.ts`, `src/ipc/mock.ts`, the `src/ipc/tauri/*` assembly,
`src/settings/{uiSettingsDefaults.json,defaults.ts,ranges.ts}`,
`src/ipc/mock/{persistence.ts,handlers/session.ts}`, `src/components/settings/catalog/general.ts`,
`src/components/settings/coverageFixtures.ts`, `src/test/uiSettingsKit.ts`, `src/obs/rawArgPolicy.json`.
UI components follow `P125-ui.md`.

**No CSP / capability change.** All HTTP originates in Rust, and `connect-src` stays
`'self' ipc: http://ipc.localhost`. Links use the existing `open_url` (`validate_web_url`) with URLs
Rust built (§7.6).

---

## 2. Data model

### 2.1 Rust (`bonsai-deps/src/model.rs`)

All types derive `Debug, Clone, PartialEq, Serialize, Deserialize` with
`#[serde(rename_all = "camelCase")]`. Every `String` field that comes from the repo or a response is
passed through `clean_display` (§7.5) `[LOW-3]`.

```rust
#[serde(rename_all = "lowercase")] pub enum Ecosystem { Npm, Cargo, Pypi, Go, Nuget }
#[serde(rename_all = "lowercase")] pub enum UpdateKind { Patch, Minor, Major }
#[serde(rename_all = "lowercase")] pub enum Severity { Critical, High, Medium, Low, Unknown }
#[serde(rename_all = "lowercase")] pub enum Relation { Direct, Transitive }
#[serde(rename_all = "lowercase")] pub enum DepScope { Normal, Dev, Build, Optional, Peer }

#[serde(rename_all = "camelCase")] pub enum LocalReason { Path, Git, Workspace, Url }
#[serde(rename_all = "camelCase")]
pub enum PrivateReason { CustomRegistry, ScopedRegistry, GoPrivate, SourceMapping,
                         ConfigUnreadable /* [HIGH-1] fail-closed */ }

#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Availability {
    Checked,                          // deps.dev knew it
    Unknown,                          // deps.dev 404. OSV is STILL queried when a resolved version exists,
                                      // so an Unknown row CAN carry vulns [UI-e]
    Local { reason: LocalReason },    // "local"
    Private { reason: PrivateReason,  // "private — not checked"
              registry_host: Option<String> }, // [UI-d] host only, from `extract_host` (§4.4; userinfo,
                                               // port, path and query stripped). None for GoPrivate /
                                               // ConfigUnreadable / a named registry with no index URL.
                                               // Display only; never sent.
}

pub struct Advisory {
    pub id: String,                 // text of an AdvisoryId minted from an OSV response (§7.3)
    pub aliases: Vec<String>,       // ≤ 16, each passing the AdvisoryId shape check
    pub summary: Option<String>,    // ≤ 300 chars; PLAIN TEXT — never rendered as markdown/HTML [LOW-3]
    pub severity: Severity,
    pub cvss_score: Option<f32>,    // 0.0..=10.0
    pub modified: String,           // OSV `modified`; the reuse key (§5.1 step 10)
    pub url: String,                // https://osv.dev/vulnerability/{id}, built by Rust
}

pub struct DepRow {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub relation: Relation,
    pub scope: DepScope,
    pub declared: Option<String>,   // manifest constraint (None for transitive)
    pub current: Option<String>,    // resolved version; None ⇒ UI shows `declared`
    pub latest: Option<String>,     // None for transitive / unknown / local / private
    pub update: Option<UpdateKind>, // None = up to date or n/a
    pub availability: Availability,
    pub vulns_checked: bool,        // false ⇒ UI must not claim "no vulnerabilities"
    pub vulns: Vec<Advisory>,       // severity desc, then id asc
    pub registry_url: Option<String>, // Some only for Checked | Unknown rows (§7.6)
}

#[serde(rename_all = "camelCase")]
pub enum ParseErrorKind { TooLarge, NotAFile, Unreadable, InvalidSyntax, UnsupportedFeature,
                          UnsupportedVersion, LimitExceeded, Internal }

#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GroupState {
    Ok,
    NoLockfile,
    /// [MEDIUM-1] `message` is RENDERED BY RUST from `category` + `line:col` using a fixed table
    /// (e.g. "invalid syntax at 12:4"). It never contains a parser's Display output or file content.
    ParseError { file: String, category: ParseErrorKind, line: Option<u32>, col: Option<u32>, message: String },
    NotTracked { label: String },
}

pub struct ManifestGroup {
    pub manifest_path: String,          // repo-relative, '/'-separated, ≤ 256 chars (LOW-3)
    pub lockfile_path: Option<String>,
    pub ecosystem: Option<Ecosystem>,   // None ⇔ NotTracked
    pub state: GroupState,
    pub rows: Vec<DepRow>,              // sorted per §5.2
    pub transitive_checked: u32,        // [UI-d] Public transitive lock entries queried against OSV
}

pub struct SeverityCounts { pub critical: u32, pub high: u32, pub medium: u32, pub low: u32, pub unknown: u32 }

pub struct DepSummary {                 // computed ONLY by `summarize(&[ManifestGroup])`; counts ROWS
    pub major: u32, pub minor: u32, pub patch: u32,
    pub vulnerable: u32,                // rows with vulns non-empty (direct + transitive, any availability)
    pub vulnerable_by_severity: SeverityCounts, // by each row's max severity
    pub up_to_date: u32,                // Checked direct rows with update None
    pub unknown: u32, pub local: u32, pub private: u32,
    pub not_tracked: u32, pub parse_errors: u32, // groups
}

pub struct DepReport { pub repo_id: String, pub checked_at_ms: i64,
                       pub groups: Vec<ManifestGroup> /* empty ⇒ "No dependency manifests found" */,
                       pub summary: DepSummary }

// persisted only (cache.rs); never on the wire
pub struct FileStamp { pub path: String, pub len: u64, pub mtime_ms: i64 }
pub struct CachedReport { pub schema: u32 /* CACHE_SCHEMA = 1 */, pub repo_id: String,
                          pub stamps: Vec<FileStamp>, pub report: DepReport }
```
§7.3 states it outright: **private and local names appear in the local view and cache by design.**
D7's boundary is the socket `[MEDIUM-2]`.

### 2.2 Rust view DTO (`src-tauri/src/deps/mod.rs`)

```rust
#[serde(rename_all = "camelCase")] pub enum CheckFailureKind { Offline, RateLimited, Service, Timeout, Internal }
/// [MEDIUM-1] `message` is rendered by Rust from `kind` + `http_status` with a fixed table. It carries no free text.
pub struct CheckFailure { pub kind: CheckFailureKind, pub http_status: Option<u16>, pub message: String, pub at_ms: i64 }
/// [UI-b] `done`/`total` count WORK UNITS = deps.dev package lookups + OSV querybatch requests
/// (§5.1 step 7). They are not packages; advisory-detail GETs are not counted.
pub struct RunningCheck { pub check_id: u64, pub started_at_ms: i64, pub done: u32, pub total: u32 }

pub struct DepTrackerView {
    pub repo_id: String,
    pub enabled: bool,
    pub report: Option<DepReport>,        // last completed report (from the cache), shown while running
    pub manifests_changed: bool,          // D5 stale
    pub running: Option<RunningCheck>,
    pub last_error: Option<CheckFailure>, // latest FAILED attempt; cleared on success
    pub next_check_ms: Option<i64>,
}
pub struct DepsChangedPayload { pub repo_id: String, pub check_id: Option<u64>, pub kind: DepsChangedKind,
                                pub done: Option<u32>, pub total: Option<u32> /* same unit as RunningCheck */ }
#[serde(rename_all = "camelCase")]
pub enum DepsChangedKind { Started, Progress, Finished, Failed, Cancelled, Stale, Cleared }
```

Settings follow the `AutoFetch` pattern in `settings/prefs.rs`:
`#[serde(rename_all = "camelCase", default)] pub struct DepTracker { pub enabled: bool /* false */, pub interval_hours: u32 /* 12 */ }`.
`clamp_dep_tracker` keeps `interval_hours ∈ 1..=168`. Wiring: `Settings.dep_tracker`,
`UiSettings.dep_tracker`, and `UiSettingsPatch.dep_tracker: Option<DepTracker>` (a whole-struct
patch). Enabling the tracker *is* the D1 consent.

### 2.3 TypeScript (`src/ipc/types/depTracker.ts`)
The exact camelCase mirror of §2.1/§2.2 (plus `DepTrackerSettings`) is in `P125-ecosystems.md`
**E6**. Keep the two in lockstep.

**Derived UI states** (computed in the hook, not in Rust):

| State | Condition |
|---|---|
| off | `!enabled` |
| loading | `!report && !lastError` |
| populated / up-to-date | `report`, with / without `major+minor+patch+vulnerable > 0` |
| stale | `manifestsChanged \|\| lastError` |
| offline | `lastError.kind ∈ {offline, rateLimited, timeout}` |
| error | `lastError.kind ∈ {service, internal}` |
| empty | `report.groups.length === 0` |

---

## 3. Version rules (`version/`; the per-scheme table is `P125-ecosystems.md` E1)

### 3.1 Trait
```rust
pub trait VersionScheme: Sync {
    type V: Ord + Clone;
    fn parse(&self, s: &str) -> Option<Self::V>;           // concrete versions only; iterative, no recursion
    fn is_prerelease(&self, v: &Self::V) -> bool;
    fn segments(&self, v: &Self::V) -> (u64, u64, u64);
    fn lower_bound(&self, constraint: &str) -> Option<Self::V>; // floor of a constraint (no-lockfile case)
}
```
Free functions `select_latest(eco, …)` / `classify_update(eco, …)` dispatch on `Ecosystem`.

### 3.2 `classify_update` — literal positions (ruled, Q3)
```
if latest <= cur                  → None
(a,b,_) = segments(cur); (x,y,_) = segments(latest)
if epoch differs or x != a        → Major
if y != b                         → Minor
else                              → Patch     // incl. prerelease→release, 4th segment, post
```

### 3.3 `select_latest` (AC3)
```
parsed = versions.filter_map(parse); allow_pre = current.map_or(false, is_prerelease)
if !allow_pre:
    if let Some(d) = parsed.find(is_default && !is_deprecated && !is_prerelease) { return d }
    return max(parsed.filter(!is_deprecated && !is_prerelease))
else:
    return max(parsed.filter(!is_deprecated))
```
**Residual gap (ruled, Q1):** deps.dev v3 has no yanked flag, so excluding yanked versions relies on
`isDefault`. The two fallback branches can surface a yanked version. P125d records a live probe
against one known-yanked crate and one yanked PyPI release.
**No-lockfile case:** `current = None`, `update = classify_update(lower_bound(declared), latest)`,
`vulns_checked = false`, and no OSV query. The exception is the "declared ⇒ resolved" rule in
ecosystems E2.

---

## 4. Discovery, adapters, registry context, file reads

### 4.1 Discovery (`discover.rs`) — manifests and lockfiles ONLY `[HIGH-1 item 1]`
```
discover_full(repo) -> FileSet:                           // check-time only (§5.1 step 1)
  entries = repo.index().entries()
           .filter(mode ∉ {0o120000 symlink, 0o160000 gitlink})                    // [MEDIUM-3]
         ∪ statuses(include_untracked, recurse_untracked_dirs, !include_ignored,
                    pathspec = ALL_MANIFEST_AND_LOCK_GLOBS)
  keep p where basename matches an adapter manifest/lockfile or catalog::not_tracked
  drop p if any segment ∈ {node_modules, bower_components, .venv, venv, vendor, target, .git}
  drop p unless path_is_clean(p)                                                     // [MEDIUM-3]
  if |manifests| > MAX_MANIFESTS (2000): keep first 2000 (sorted); report one extra group
     ParseError{file: "<N more manifests>", category: LimitExceeded}                 // [LOW-3]

probe_stale(repo, cached: &[FileStamp]) -> bool:          // cheap; watcher + focus path
  any safe_stat(path) ≠ cached stamp → true
  any repo.index() entry (non-symlink) with a matching basename whose path ∉ cached → true
  else false                                              // no untracked walk
```
`path_is_clean(p)`: every `Path::components()` element is `Normal`, with no `..`, `.`, `RootDir` or
`Prefix`. **Registry configs are NOT taken from `FileSet`.** `registry_ctx` walks for them itself
(§4.4), so gitignored `.npmrc` files are seen. `stamps` uses `safe_stat` (symlink-aware). A brand-new
untracked manifest surfaces at the next check.

### 4.2 Adapter trait (`adapter.rs`) and parser limits
```rust
pub trait EcosystemAdapter: Sync {
    fn ecosystem(&self) -> Ecosystem;
    fn is_manifest(&self, file_name: &str) -> bool;
    fn is_lockfile(&self, file_name: &str) -> bool;
    fn lockfile_for(&self, manifest: &str, files: &FileSet) -> Option<String>; // same dir, then ancestors ≤ workdir
    /// Names this manifest DECLARES for itself (HIGH-1 item 3), e.g. package.json `name`.
    fn declared_names(&self, manifest: &str) -> Vec<String>;
    fn parse(&self, input: &ParseInput<'_>) -> Result<ParsedGroup, ParseError>;
}
pub struct ParseInput<'a> { pub manifest_path: &'a str, pub manifest: &'a str,
                            pub lockfile: Option<(&'a str, &'a str)>, pub files: &'a FileSet }
pub struct ParsedGroup { pub direct: Vec<Dependency>, pub lock_entries: Vec<LockEntry>, pub has_lockfile: bool }
pub struct Dependency { pub name: String, pub scope: DepScope, pub declared: Option<String>,
                        pub resolved: Option<String>, pub source: DepSource }
pub struct LockEntry { pub name: String, pub version: String, pub source: DepSource }
pub enum DepSource { Registry(RegistryRef), Path, Git, Workspace, Url }
pub enum RegistryRef { Default, Named(String), Host(String) /* extract_host output only */, Scope(String) }
/// [MEDIUM-1] No free-text field. Adapters map parser errors to (kind, line, col) and must NEVER
/// call `to_string()` / `Display` on a toml / quick-xml / yaml / serde_json error.
pub struct ParseError { pub file: String, pub kind: ParseErrorKind, pub line: Option<u32>, pub col: Option<u32> }
pub static ADAPTERS: &[&dyn EcosystemAdapter];
```
Limits, all `[HIGH-2]` unless tagged otherwise:
- **YAML** (`yaml_events.rs`) uses the `yaml_rust2::parser::Parser` event API with a custom
  `EventReceiver`.
  - Any `Event::Alias` and any non-zero anchor id ⇒ `ParseError{UnsupportedFeature}`.
  - Depth > `YAML_MAX_DEPTH = 64` or nodes > `YAML_MAX_NODES = 2_000_000` ⇒ `LimitExceeded`.
  - The receiver builds only the fields the pnpm/berry adapters read.
- **In-house parsers** (yarn v1, requirements, PEP 440, go.mod, `.npmrc`) are iterative line/char
  scanners with no recursion.
- **TOML / JSON / XML**:
  - TOML nesting > 64 ⇒ `LimitExceeded`. This is checked by a pre-scan that counts `[`/`{` depth
    outside strings, because the `toml` crate's recursion limit is unverified.
  - serde_json keeps its 128-depth limit.
  - quick-xml: DTD ignored, no entity expansion.
- **Size** `[MEDIUM-3]`: a file > 32 MiB ⇒ `TooLarge`, enforced by `safe_read`.
- **Total parsed bytes** per check > `MAX_PARSE_BYTES = 256 MiB` ⇒ later files become `LimitExceeded`
  groups `[LOW-3]`.
- `catch_unwind` around each parse ⇒ `Internal`. This guards panics only. Memory is bounded by the caps
  above, **not** by `catch_unwind`.

### 4.3 Ecosystem rules → `P125-ecosystems.md` E2 (one row per ecosystem + the not-tracked catalog)

### 4.4 `RegistryContext` and the key-filtered extractors (`registry_ctx.rs`)
```rust
/// [MEDIUM-1] NO Debug, Serialize, Deserialize or Clone. Fields are hosts, scope names and bools only.
pub struct RegistryContext { /* per-eco: default_public: bool, custom_hosts, scoped: map<scope, Option<host>>,
                                 fail_closed: bool, replace_all: bool, go_private_globs, nuget_mapping … */ }
pub struct LocalNames { /* per-eco set of normalized names (HIGH-1 item 3) */ }
/// [NEW-2] NO Debug (or a redacting one that prints only the variant) — `Content` may hold tokens.
pub enum UserRead { Absent, Content(String), Unreadable }
pub trait HostEnv: Sync {
    fn var(&self, name: &str) -> Option<String>;
    /// [NEW-2] `keep` filters on the key BEFORE the value is collected: only registry/index keys are
    /// returned, so `npm_config_//host/:_authToken`-style values are never read into memory.
    fn vars_with_prefix_ci(&self, prefix: &str, keep: fn(&str) -> bool) -> Vec<(String, String)>; // npm_config_*, CARGO_REGISTRIES_*
    fn read_user_file(&self, which: UserConfig) -> UserRead;               // closed enum of E3 user/system paths
    /// [Q7][Q9-narrowed] Bounded read of `file_name` in `dir` (an ancestor ABOVE the workdir);
    /// symlinks followed ONLY when `dir` is `$HOME`/`$CARGO_HOME`, else a symlink ⇒ Unreadable; a
    /// UNC/network target, a dangling link, a non-regular target after open or > 1 MiB ⇒ Unreadable.
    fn read_above_workdir(&self, dir: &Path, file_name: &str) -> UserRead;
}
impl RegistryContext {
    pub fn load(workdir: &Path, manifests: &[String], env: &dyn HostEnv) -> RegistryContext;
}
/// Real URL parse; returns lowercase host only (userinfo, port, path, query and fragment dropped).
/// A value that does not parse ⇒ None, and the caller fails closed.
pub(crate) fn extract_host(raw: &str) -> Option<String>;
```
The `load` algorithm:
```
for each manifest dir d, walking d → workdir (inclusive) [HIGH-1 item 1]:
   for each E3 repo-walk file name f in that ecosystem: r = safe_read(workdir, d/f, 1 MiB)
      Absent → skip;  Unreadable | TooLarge | NotAFile(symlink/FIFO) → eco.fail_closed = true   [HIGH-1 item 2]
      Content(s) → extract_keys(eco, s)   // key-filtered: see below; the raw `s` is dropped here
[Q7] Cargo and NuGet only: for each ancestor a of the workdir, parent(workdir) → filesystem root:
   for each E3 above-workdir file name f: r = env.read_above_workdir(a, f)
      Absent → skip; Unreadable → eco.fail_closed = true; Content(s) → extract_keys(eco, s)
   config files only — manifests and lockfiles are NEVER read above the workdir (MEDIUM-3)
for each E3 user/system file: same (read_user_file; symlinks allowed for USER files, target must be a
   regular file, 1 MiB bounded read); Unreadable ⇒ fail_closed
for each E3 env var: extract_keys on the value only
extract_keys(eco, text):  // the ONLY keys ever materialised [MEDIUM-1]
   npm/pnpm .npmrc & pnpm rc: raw-line scan; keep lines whose key is exactly `registry` or matches
       `@<scope>:registry`; every other line (incl. `//host/:_authToken`, `_auth`, `always-auth`)
       is skipped WITHOUT being stored
   yarn: `registry` (v1), `npmRegistryServer`, `npmScopes.<s>.npmRegistryServer` (berry, via the
       YAML event reader; other keys are skipped)
   pip/uv: `index-url`, `extra-index-url` (ini [global]/[install]); uv `index-url`,
       `extra-index-url`, `[[index]] url` + `default`
   Cargo: `source.*.replace-with`, `source.*.registry`, `registries.*.index` (other tables not deserialized)
   NuGet: `packageSources/add@value`, `<clear/>`, `disabledPackageSources`, `packageSourceMapping`;
       the DTO has NO field for `packageSourceCredentials`/`apikeys`
   Go env file: `GOPRIVATE`, `GONOPROXY`, `GONOSUMDB` lines only
   each URL value → extract_host(v); None ⇒ eco.fail_closed = true                        [HIGH-1 item 2]
   a syntax error in a config ⇒ eco.fail_closed = true (NEVER a ParseError group, never logged)
local_names = ∪ adapter.declared_names(m) for every discovered manifest (normalized per eco)
```
NuGet precedence across levels (closest wins; `<clear/>` drops every farther level) follows the order
repo walk → above-workdir walk (nearest first) → user → machine-wide `[Q7]`.

### 4.5 Safe file reads (`fsread.rs`) `[MEDIUM-3]`
```rust
/// [NEW-2] NO Debug (or a redacting one that prints only the variant).
pub enum ReadOutcome { Absent, Content(String), NotAFile, TooLarge, Unreadable }
pub fn safe_read(workdir_canon: &Path, rel: &str, cap: u64) -> ReadOutcome;
pub fn safe_stat(workdir_canon: &Path, rel: &str) -> Option<(u64, i64)>;
```
```
safe_read:
  !path_is_clean(rel)                                   → NotAFile
  md = symlink_metadata(workdir/rel); NotFound          → Absent
  md.file_type().is_symlink() || !md.is_file()          → NotAFile      // symlink, FIFO, device, dir
  canon = canonicalize(workdir/rel); !canon.starts_with(workdir_canon) → NotAFile
  f = File::open(canon); f.metadata().is_file() else    → NotAFile      // re-check after open (TOCTOU)
  bytes = f.take(cap + 1).read_to_end(); len > cap      → TooLarge
  String::from_utf8_lossy(bytes)                        → Content
```
- `safe_stat` applies the same checks and returns `(len, mtime_ms)`.
- The E3 "repo walk" means **in-repo ancestors only**.
- The Q7 above-workdir walk (Cargo and NuGet config files only) goes through
  `HostEnv::read_above_workdir`: symlinks followed only in `$HOME`/`$CARGO_HOME` (else ⇒
  Unreadable), each link target checked before use (UNC / network ⇒ Unreadable), regular file
  only, re-check after open, and a `take(1 MiB + 1)` read `[Q7]` `[Q9-narrowed]`.
- `$CARGO_HOME` and the other user/system files are read with `HostEnv::read_user_file` (a bounded
  read that requires a regular file).

---

## 5. Check pipeline

### 5.1 `bonsai_deps::run_check` (blocking; runs inside one `spawn_blocking`)
```rust
pub struct CheckCtx<'a> {
    pub transport: &'a dyn Transport, pub env: &'a dyn HostEnv, pub cancel: &'a CancelToken,
    pub progress: &'a (dyn Fn(u32, u32) + Sync), pub prev: Option<&'a DepReport>, pub now_ms: i64,
}
/// [MEDIUM-1] No String payloads anywhere.
pub enum CheckError { Cancelled, Offline, RateLimited, Timeout,
                      Service(ServiceFault /* Status(u16) | Redirect | BadJson | TooLarge | TooManyAdvisories | PageLimit */),
                      Internal(InternalFault /* GitOpen | GitIndex | Io */) }
pub fn run_check(workdir: &Path, repo_id: &str, ctx: &CheckCtx<'_>) -> Result<CachedReport, CheckError>;
pub struct CancelToken(Arc<AtomicBool>);
```
```
 1 repo = git2::Repository::open(workdir); files = discover_full(repo)
 2 local_names, reg = RegistryContext::load(workdir, manifests(files), env)
 3 per manifest (sorted): NotTracked → group; else safe_read + adapter.parse → ParsedGroup | ParseError group
 4 vis = privacy::classify(eco, name, version, &source, &reg, &local_names) for every Dependency / LockEntry
 5 latest_q = dedupe Public direct by (eco,name); vuln_q = dedupe Public with a resolved version by (eco,name,ver)
 6 if both empty → assemble, zero requests
 7 total = |latest_q| + ceil(|vuln_q| / OSV_BATCH_MAX)     // the [UI-b] progress unit
 8 deps.dev, DEPSDEV_PARALLEL = 6 threads: cancel? → Cancelled; send(depsdev_package(&p));
      200 → select_latest; 404 → Unknown; 429 → RateLimited; other → Service       (any non-404 failure aborts)
 9 OSV chunks of OSV_BATCH_MAX → send(osv_querybatch(chunk, tokens)); pages ≤ 10 per query else Service(PageLimit);
      the response is parsed in remote/osv.rs, which mints AdvisoryId / PageToken from it (§7.3)
10 unique ids > MAX_ADVISORIES (5000) → Service(TooManyAdvisories)                              [LOW-3]
   for each minted id: if ctx.prev has an Advisory with id == id.as_str() && modified equal → reuse its
      DATA (string compare; never mint from cache) [MEDIUM-2]; else send(osv_vuln(&id)); 404 → severity Unknown
11 assemble rows (direct always; transitive only with vulns — AC4); transitive_checked per group; sort §5.2; summarize
12 stamps(files) → CachedReport
```
Constants: `OSV_BATCH_MAX = 1000` (VERIFIED, §0.1 `[P125d review]`), `DEPSDEV_PARALLEL = 6`. **`CHECK_DEADLINE = 10 min` is
enforced by the async caller** (§6.4), not inside the worker `[HIGH-2]`. The worker checks `cancel`
before every send and between parse steps. The check is atomic: it returns a full report or an
error, and the previous report is kept.

Severity: `database_specific.severity` (CRITICAL / HIGH / MODERATE→Medium / LOW) → else the highest
`CVSS_V3` `severity[]` scored by `cvss::base_score_v3` → band (≥9 Critical, ≥7 High, ≥4 Medium,
>0 Low) → else `Unknown`.

### 5.2 Ordering (Rust decides; the UI only filters)
- Groups: `manifest_path` ascending, root-level first.
- Rows: max severity descending (rows without vulns last), then Major > Minor > Patch > None, then
  Direct before Transitive, then name ascending.

### 5.3 Error mapping
| Cause | Result |
|---|---|
| connect / DNS / TLS | `Offline` |
| 429 | `RateLimited` |
| 5xx | `Service(Status)` |
| 3xx | `Service(Redirect)` |
| malformed JSON | `Service(BadJson)` |
| oversize body | `Service(TooLarge)` |
| per-request timeout or deadline | `Timeout` |
| transport `Gated` (src-tauri gate: disabled / stale epoch) `[P125d review]` | `Cancelled` |
| transport `Other` (neither connect nor timeout) `[P125d review]` | `Offline` |

`CheckFailure.message` comes from a fixed `kind`/status table.

---

## 6. Scheduler, triggers, cancellation, stale

### 6.1 `src-tauri/src/deps/mod.rs`
```rust
#[derive(Clone)] pub struct DepsState(Arc<DepsInner>);
pub struct DepsInner {
    pub cfg: Mutex<DepTracker>,
    pub epoch: AtomicU64,                              // bumped on disable
    pub repos: Mutex<HashMap<String, RepoDeps>>,
    pub transport: Arc<dyn bonsai_deps::Transport>,     // GatedTransport(real) | FakeTransport
    pub next_check_id: AtomicU64,
}
pub struct RepoDeps {
    pub running: Option<(RunningCheck, CancelToken, u64 /*epoch*/)>,
    pub last_attempt_ms: Option<i64>, pub last_error: Option<CheckFailure>,
    pub opened_unchecked: bool, pub stale: bool, pub last_probe_ms: i64,
}
pub fn apply_config(state: &DepsState, cfg: DepTracker, app: &AppHandle);
pub fn on_active_changed(state: &DepsState, old: Option<&str>, new: Option<(&str, PathBuf)>);
pub fn on_repo_closed(state: &DepsState, repo_id: &str);
pub fn on_fs_change(state: &DepsState, repo_id: &str, workdir: PathBuf);
pub fn tick(state: &DepsState, active: Option<(String, PathBuf)>, now_ms: i64, emit: &DepsEmitFn);
pub type DepsEmitFn = Arc<dyn Fn(DepsChangedPayload) + Send + Sync>;   // obs::emit_logged("deps-changed")
```
`tick` is called from `run_scheduler`'s loop next to `tick_once` (one global 15 s loop). There is no
`JobKind` change.

### 6.2 Pure planner (`deps/plan.rs`)
```rust
pub enum Trigger { Open, Interval, Manual, Enabled }
pub enum DepPlan { Run(Trigger), Wait { next_ms: Option<i64> } }
pub fn plan_dep_check(enabled: bool, is_active: bool, running: bool, opened_unchecked: bool,
                      interval_ms: i64, now_ms: i64, last_attempt_ms: Option<i64>) -> DepPlan
```
```
if !enabled || !is_active || running → Wait{None}
if opened_unchecked || last_attempt_ms.is_none() → Run(Open)
t = last_attempt_ms; if now >= t + interval || t > now + interval → Run(Interval) else Wait{Some(t+interval)}
```
Failed attempts also stamp `last_attempt_ms`. There is no backoff and no fast retry.

### 6.3 Triggers & scoping (ruled, Q4)
- **Active tab only.** Background tabs keep their cache and are never scheduled.
- **"Opened"** means the first activation of a repo in this process. `on_active_changed` sets
  `opened_unchecked` and calls `tick` directly.
- **Manual:** `deps_check_now`.
- **Enable:** a `false → true` change in `apply_config`.
- **Interval change:** applies on the next tick.
- **Cancellation:** `on_active_changed(old…)` and `on_repo_closed` set the old run's `CancelToken`.
  A cancelled run writes nothing and emits `Cancelled`. The frontend also drops any payload whose
  `repoId` ≠ current.

### 6.4 Run / persist / deadline / disable (`deps/exec.rs`)
```
start_run(state, repo_id, workdir, trigger) -> Result<u64, AppError>:
  lock repos; running → return existing checkId
  !cfg.enabled → Err(AppError::Other("dependency tracker is disabled"))
  id, epoch, token; record running; emit Started
  task = spawn_blocking( prev = cache::load; run_check(ctx.prev = prev.report,
             progress → if epoch == state.epoch && !token.is_cancelled() { emit Progress (≥250 ms) } ) ) [LOW-1]
  res = tokio::time::timeout(CHECK_DEADLINE, task).await                                        [HIGH-2]
     Err(elapsed) → token.cancel(); treat as Err(Timeout); the blocking thread's eventual result is
                    discarded by the epoch/token check below (it stops at its next cancel check)
  lock repos:
     epoch ≠ state.epoch || (token.is_cancelled() && res ≠ Timeout) → clear running; emit Cancelled; return
     Ok(c) → cache::save (under the lock); last_error = None; stale = false
     Err(e) → last_error = Some(render(e))
     last_attempt_ms = now; clear running; emit Finished | Failed
apply_config(new):
  old.enabled && !new.enabled → lock repos; epoch += 1; cancel all; clear map;
      remove_dir_all(<app_data_dir>/deps-tracker) (NotFound ok; else log + startup retry); unlock; emit Cleared
  !old.enabled && new.enabled → start_run(active, Enabled)
```
- Persisting under the lock that bumps `epoch` means no cache write can land after the wipe.
- **After a disable, the requests already in flight — up to `DEPSDEV_PARALLEL` (6), each up to the
  30 s request timeout — may complete. None start** (`GatedTransport`, §7.2) `[LOW-1]`.
- At startup with `enabled == false`, `lib.rs` removes any leftover `deps-tracker/` directory.

### 6.5 Cache (`bonsai-deps/src/cache.rs`)
- **Location:** `<app_data_dir>/deps-tracker/<repo_key>.json`, via `app_data_root`.
  `repo_key = history_index::store::repo_key(workdir, path_ignorecase(workdir))`, **computed in
  `src-tauri/src/deps/` and passed into `cache.rs` as a value** — `bonsai-deps` never calls
  `path_ignorecase`, matching the §7.4 allowlist (audit re-check nit).
- **Load** returns `None` on a schema or `repo_id` mismatch, a file > 32 MiB, or bad JSON. There is
  no display TTL.
- **Write** is tmp + rename. The report is re-read from disk per `deps_get_view`. Only
  `last_attempt_ms` is memoized.

### 6.6 Stale (D5, AC9)
- `stale = probe_stale(repo, &cached.stamps)`. It is computed by `deps_get_view` (called on window
  focus) and by `on_fs_change`.
- `on_fs_change` runs from the `open_repo` watcher closure, for Worktree and Refs bursts alike. It
  proceeds only if the tracker is enabled, the repo is active, the report is cached and not already
  stale, and 2 s have passed since the last probe. It then runs `probe_stale` in spawn_blocking and
  emits `Stale` on a false→true change.
- `watcher/classify.rs` stays untouched.
- A stale report never triggers a check.

---

## 7. Security & privacy

### 7.1 Outbound hosts (exhaustive) and fields
| Request | Method + URL | Sent |
|---|---|---|
| `depsdev_package` | `GET https://api.deps.dev/v3/systems/{npm|cargo|pypi|go|nuget}/packages/{pct(name)}` | ecosystem, package name |
| `osv_querybatch` | `POST https://api.osv.dev/v1/querybatch` `{"queries":[{"package":{"name","ecosystem"},"version","page_token"?}]}` | ecosystem, name, resolved version, OSV-issued page token |
| `osv_vuln` | `GET https://api.osv.dev/v1/vulns/{id}` | an advisory id OSV returned |

- **Headers:** `Accept: application/json`; `User-Agent: Bonsai-dependency-tracker`; `Content-Type` on
  POST.
- **Never sent:** cookies, auth, repo path or name, remote URL, manifest path, file content, registry
  hosts, or machine identifiers.
- HTTPS only. The hosts are `&'static str`.
- reqwest honours `HTTPS_PROXY`. A TLS-inspecting corporate proxy can see the public names (audit
  INFO; ui-designer may mention it in the consent copy).

### 7.2 Transport (`remote/transport.rs`)
```rust
pub struct OutboundRequest { method: Method, url: String, body: Option<String> }   // fields private to `remote`
impl OutboundRequest {
    pub(in crate::remote) fn depsdev_package(p: &PublicPackage) -> Self;
    pub(in crate::remote) fn osv_querybatch(q: &[PublicPackage], tokens: &[Option<&PageToken>]) -> Self;
    pub(in crate::remote) fn osv_vuln(id: &AdvisoryId) -> Self;
}
pub struct RawResponse { pub status: u16, pub body: Vec<u8> }
pub trait Transport: Send + Sync { fn send(&self, req: &OutboundRequest, cap: usize) -> Result<RawResponse, TransportError>; }
pub enum TransportError { Connect, Timeout, TooLarge, Gated, Other }
```
`remote/` exposes to `check.rs` only functions of the shape
`fn fetch_latest(&dyn Transport, &PublicPackage) -> …` and the OSV equivalents.

`ReqwestTransport` settings: ring provider, `redirect::Policy::none()`, `connect_timeout 10 s`,
`timeout 30 s`, `https_only(true)`, no cookie store, bounded read. Body caps: deps.dev 8 MiB, OSV
batch 16 MiB, OSV vuln 1 MiB.

`GatedTransport` (src-tauri) returns `Gated` with no I/O unless `cfg.enabled` holds and the epoch
matches (AC1/AC12). Forge's `HttpTransport` is not reused: it accepts arbitrary URLs and headers.

### 7.3 D7 structural guarantee (`privacy.rs`, `remote/osv.rs`)
```rust
// privacy.rs
pub struct PublicPackage { eco: Ecosystem, name: String, version: Option<String> } // private fields; no Deserialize/Default/From
pub enum Visibility { Public(PublicPackage), Local(LocalReason), Private(PrivateReason, Option<String> /*host*/) }
pub fn classify(eco: Ecosystem, name: &str, version: Option<&str>, src: &DepSource,
                reg: &RegistryContext, local: &LocalNames) -> Visibility;
// remote/osv.rs  [MEDIUM-2]
pub struct AdvisoryId(String);  // NO pub constructor, NO Deserialize/TryFrom/From impls
pub struct PageToken(String);   // same
fn mint_id(raw: &str) -> Option<AdvisoryId>;      // private to osv.rs; called ONLY on fields of a deserialized OSV DTO
fn mint_page_token(raw: &str) -> Option<PageToken>;
impl AdvisoryId { pub fn as_str(&self) -> &str; }
```
```
classify:
  n = normalize(eco, name)
  if local.contains(eco, n)                    → Local(Workspace)                    [HIGH-1 item 3]
  match src: Path|Git|Workspace|Url            → Local(…)
  if reg.fail_closed(eco)                      → Private(ConfigUnreadable, None)     [HIGH-1 item 2]
  if let Some((reason, host)) = reg.private_reason(eco, n, src) → Private(reason, host) // E2 Private column
  if !valid_public_name(eco, n)                → Private(CustomRegistry, None)
  → Public(PublicPackage{ eco, n, version.filter(valid_version_chars) })
```
`valid_public_name`:

| Eco | Rule |
|---|---|
| npm | `^(@[a-z0-9-~][a-z0-9-._~]*/)?[a-z0-9-~][a-z0-9-._~]*$`, ≤ 214 |
| Cargo | `^[A-Za-z0-9_-]{1,64}$` |
| PyPI | PEP 508 name, ≤ 128 |
| Go | `^[A-Za-z0-9._~/-]{1,256}$`; the first element contains a `.` **and** a letter or digit `[LOW-2]` |
| NuGet | `^[A-Za-z0-9._-]{1,100}$` |

For every ecosystem, **the name and each `/`-separated element must be neither `.` nor `..`**
`[LOW-2]`. Versions must match `^[A-Za-z0-9.+_!~-]{1,128}$`. `mint_id` accepts
`^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$`; `mint_page_token` accepts ≤ 1 KiB of printable ASCII.

Enforcement:
1. A `compile_fail` doctest: a `PublicPackage { .. }` literal outside `privacy.rs` does not compile.
2. A **second `compile_fail` doctest**: `AdvisoryId::parse("x")` / `AdvisoryId("x".into())` outside
   `remote::osv` does not compile `[MEDIUM-2]`.
3. `OutboundRequest` constructors are `pub(in crate::remote)`.
4. `no_process.rs` asserts that no `adapter::`, `Dependency` or `LockEntry` token appears under
   `remote/`.
5. The sentinel tests (§9).

**Private and local names DO appear in the local view and the cache as "private — not checked" /
"local" rows, by design. D7's boundary is the socket** `[MEDIUM-2]`.

### 7.4 No-process guarantee (AC11) `[MEDIUM-4]`
- **Lint:** `clippy.toml` bans `std::process::Command` / `tokio::process::Command` and `::new`, and
  the ban is denied crate-wide.
- **Source scan:** `tests/no_process.rs` scans `crates/bonsai-deps/src/**`, `src-tauri/src/deps/**`
  and `src-tauri/src/commands/deps.rs` for these tokens: `process::Command`, `std::process`,
  `Command::new`, `tokio::process`, `CommandRunner`, `SpawnRunner`, `libc::`, and `std::os::` followed
  by `process`.
- **`bonsai_core` allowlist:** any `bonsai_core::` path other than `bonsai_core::error::AppError` and
  `bonsai_core::git::history_index::store::repo_key` (plus `…::repo::path_ignorecase` under
  `src-tauri/src/deps/` only) fails the scan. `use bonsai_core::{…}` groups are expanded before
  matching.
- **Dependency snapshot:** `crates/bonsai-deps/dep-tree.txt` holds the committed output of
  `cargo tree -p bonsai-deps -e normal --prefix none --target all` (`--target all` so the
  snapshot is identical on every OS `[NEW-3]`), piped through
  `sed -E 's/ \([^)]*[\\/][^)]*\)//g'` to strip checkout-absolute path-dependency sources (found in
  P125a; generate from Git Bash — a PowerShell `>` writes UTF-16). The **P125a done criterion** is that a
  regenerated snapshot is byte-identical (`git diff --exit-code`). Any later change to the dependency
  set shows up in review. Adding it to the `pnpm gate` audit group is an orchestrator follow-up.

### 7.5 Untrusted-input hardening
- **Responses:**
  - typed serde, unknown fields ignored;
  - body caps as in §7.2;
  - `versions[]` > 20 000 ⇒ `Service`;
  - a deps.dev `versionKey.name` mismatch ⇒ Unknown;
  - deps.dev `links` / `registries` / `url` fields are not deserialized;
  - response data builds no URL except through a minted `AdvisoryId` / `PageToken`.
- **`clean_display(s, max)`** `[LOW-3]` strips Unicode **Cc and Cf** (this covers the bidi overrides
  U+202A–202E and U+2066–2069, and zero-width characters), then truncates to `max` chars. Limits:
  - 256 for every repo- or response-derived display string (`name`, `declared`, `current`, `latest`,
    `manifest_path`, `lockfile_path`, `ParseError.file`, `registry_host`);
  - 300 for `summary`;
  - 64 for ids and aliases.
- **Caps** `[LOW-3]`: `MAX_MANIFESTS = 2000`, `MAX_PARSE_BYTES = 256 MiB`, `MAX_ADVISORIES = 5000`.
- **Repo files:** §4.2 limits and §4.5 reads.

### 7.6 Links opened in the browser
`registry_url` comes from `catalog.rs` templates plus a percent-encoded name that passed validation
(dot segments are already rejected, `[LOW-2]`):
- `https://www.npmjs.com/package/{name}`
- `https://crates.io/crates/{name}`
- `https://pypi.org/project/{name}/`
- `https://pkg.go.dev/{module}`
- `https://www.nuget.org/packages/{id}`
- advisories: `https://osv.dev/vulnerability/{id}`

Local and Private rows get `None`, and `registry_host` is never made into a link. The frontend uses
only `openUrl`.

---

## 8. IPC surface

### Commands (`src-tauri/src/commands/deps.rs`)
```rust
#[tauri::command] pub async fn deps_get_view(state: State<'_, AppState>, deps: State<'_, DepsState>,
                                             app: AppHandle, repo_id: String) -> Result<DepTrackerView, AppError>;
#[tauri::command] pub async fn deps_check_now(state: State<'_, AppState>, deps: State<'_, DepsState>,
                                              app: AppHandle, repo_id: String) -> Result<u64, AppError>;
```
- **`deps_get_view`:** when disabled it returns `{enabled:false, …}` without reading the repo or the
  cache. Otherwise it runs spawn_blocking to load the cache and `probe_stale`. It rejects `noRepo`.
- **`deps_check_now`:** it rejects `noRepo` and `other("dependency tracker is disabled")`, and
  resolves the checkId of the running or newly started check.
- **Settings:** through `get_ui_settings` / `set_ui_settings` (`depTracker`). `set_ui_settings`
  calls `deps::apply_config` after persisting.

### Event
`deps-changed` → `DepsChangedPayload`, sent via `obs::emit_logged`. Progress fires at most 4/s. On any
kind other than `progress` for the current repo, the frontend refetches the view.

### Channels
None. A report is one command response.

### TS (`src/ipc/types/ipc-api-deps.ts`)
```ts
export interface IpcApiDeps {
  /** Never touches the repo when disabled. Rejects noRepo. */
  getDepTracker(repoId: string): Promise<DepTrackerView>;
  /** Manual refresh; resolves the running-or-new checkId. Rejects noRepo | other(disabled). */
  depsCheckNow(repoId: string): Promise<number>;
  onDepsChanged(cb: (p: DepsChangedPayload) => void): Promise<Unsubscribe>;
}
```
- `src/ipc/tauri/depTracker.ts`: `invoke('deps_get_view' | 'deps_check_now', { repoId })` and
  `listen('deps-changed')`.
- `rawArgPolicy.json`: both commands, using the existing repoId policy.
- `registration_tests.rs`: both names.

### Hook (`src/hooks/useDepTracker.ts`)
`useDepTracker(repoId)` returns `{ view, progress: {done,total} | null, refresh() }`.
- It fetches on mount, on repo change, on window focus and on `deps-changed`. It does **not** fetch
  on `repo-changed`.
- It discards results for a stale `repoId`.
- It never polls on a timer.
- Enable/disable goes through `useUiSettings`.

### Mock
The mock handler, the `?deps=` modes (including `mirror` and `large` `[UI-c]`) and the fixture
contents are specified in `P125-ecosystems.md` **E5**.

---

## 9. Test plan (AC → proof)

The AC → test table is in `P125-ecosystems.md` **E4**. That includes the HIGH-1 / MEDIUM-1 / Q7
sentinel tests, the HIGH-2 YAML/TOML bomb tests, the MEDIUM-3 fs tests and the LOW-4 live fixtures.

---

## 10. Sub-increments

| # | Scope | Done when | Commands |
|---|---|---|---|
| **P125a** | crate, `clippy.toml`, `model.rs` (+`summarize`, `clean_display`), `privacy.rs` (fail-closed `classify`, `LocalNames`, `valid_public_name` with dot-segment rules), `RegistryContext` **type + no-derive policy** (loader stubbed), `remote/osv.rs` newtype skeleton (`AdvisoryId`/`PageToken`, private mint), `version/*`, `tests/no_process.rs` (tokens + allowlist), `dep-tree.txt` | AC3 green; both `compile_fail` doctests; the no-process scan + allowlist; **`dep-tree.txt` regenerates byte-identical**; `privacy_tests` (LOW-2, fail-closed, local names) — gates: HIGH-1 items 2–5, MEDIUM-2, MEDIUM-4, LOW-2, MEDIUM-1 derive | `cargo nextest run -p bonsai-deps`; `cargo test -p bonsai-deps --doc`; `cargo clippy -p bonsai-deps -- -D warnings`; `cargo tree -p bonsai-deps -e normal --prefix none --target all \| sed -E 's/ \([^)]*[\\/][^)]*\)//g' > crates/bonsai-deps/dep-tree.txt && git diff --exit-code crates/bonsai-deps/dep-tree.txt` |
| **P125b** | `fsread.rs`, `discover.rs`, `yaml_events.rs`, `registry_ctx.rs` (ancestor walk, extractors, `extract_host`, npm/yarn/pnpm rows of E3, `FakeHostEnv` incl. `read_above_workdir`), `adapter.rs`, `catalog.rs`, the npm family, `deny.toml` bans | `fsread_tests`, `yaml_events_tests`, the npm part of `privacy_sentinel` (ignored `.npmrc`, env, yarn ws, symlinked config) and `token_sentinel` for npm; the YAML-corpus result recorded in §0.1; E3 npm/yarn rows flipped to VERIFIED — gates: HIGH-1 item 1, HIGH-2, MEDIUM-1 extractor, MEDIUM-3 | same + `cargo deny --all-features check` |
| **P125c1** | Cargo + Go adapters + their E3 rows, **incl. the Q7 above-workdir `.cargo/config{,.toml}` walk**; **[P125b review] (i)** extend the above-workdir config-only walk to npm (`workspaces` root) and pnpm (`pnpm-workspace.yaml` root) `.npmrc`, same user-file read rules; **(ii)** pnpm 11 global `config.yaml` as a new `UserConfig` variant reusing the `pnpm_workspace` extractor; TOML depth pre-scan | E2 rows covered; the Cargo/Go sentinels incl. E4 AC6 (f) Cargo half | `cargo nextest run -p bonsai-deps eco::` |
| **P125c2** | PyPI + NuGet adapters + their E3 rows (incl. NuGet `<clear/>`, disabled sources, machine-wide config, **the Q7 above-workdir `NuGet.Config` walk**) | E2 rows covered; the PyPI/NuGet sentinels incl. unparseable `nuget.config` and E4 AC6 (f) NuGet half | same |
| **P125d** | `remote/*`, `cvss.rs`, `check.rs`, `cache.rs`, the full `privacy_sentinel` + `token_sentinel`; live fixtures incl. the LOW-4 known-vulnerable set and the §3.3 yanked probe | AC2/4/5/6/7 green offline; LOW-3 caps; `CheckError`/`ParseError` carry no strings; **[P125b review] `catch_unwind` per parse, `MAX_PARSE_BYTES`, the cache-bytes + serialized-view halves of `token_sentinel`, sentinel harness switched from `outbound_from_public` to recorded requests; npm builtin `npmrc` read fail-closed; `pnpm_config__auth` presence-only check (never read the value)**. **security-auditor pass.** | `cargo nextest run -p bonsai-deps`; clippy |
| **P125e** | src-tauri: setting, `deps/{mod,plan,exec}.rs`, `GatedTransport`, the deadline wrapper, commands, hooks, disable/startup wipe, registration | AC1/8/9/12 (6-worker) + repo switch; real `HostEnv` via `std::env::vars_os()` (drop non-matching values, never copy them) [P125b review]; src-tauri's `bonsai-deps` dependency must **not** enable the `test-support` feature (it exposes the fail-open `FakeHostEnv`); **[P125c1 review]** real HostEnv resolves `UserConfig::{PnpmGlobalYaml, PnpmAuthIni, GoEnv}` paths (incl. Linux `~/.config/go/env`) and canonicalizes `$HOME`/`$CARGO_HOME` exactly like workdir ancestors (else Q9 silently never follows); **precondition: the 13 symlink tests that skip on Windows have passed on a Linux run** (CI or local) — seen, not assumed (now 16+ after P125c2) — **MET 2026-09-28** at `7619cba`: Linux container (rust 1.97.1, uid 1000) 357 passed / 0 failed / 2 ignored, 0 skips, 17 symlink tests ok; **[P125c2 review]** real HostEnv implements `read_user_files` for the NuGet machine-wide `*.config` set (no default body — compile-enforced) and reads `$VIRTUAL_ENV/pip.{conf,ini}` as a user file; **[P125d review]** serialise cache saves per repo (the tmp name is fixed per key); re-run the serialized-view token/sentinel check on `DepTrackerView` (P125d ran it on `DepReport`); real `HostEnv::var_present_ci` checks keys from `vars_os()` only (never `var_os(name)`, which copies the value); resolve the `NpmBuiltinRc` path (ASSUMED); **[P125f review]** the setting item landed early in P125f (Rust `DepTracker` in `settings/prefs.rs`, `clamp_dep_tracker` in `settings/clamp.rs`); emit `Cleared` **per open repo** with that repo's `repoId` (the hook drops non-matching payloads); add an `apply_patch` / `set_ui_settings` echo test for `dep_tracker` next to `tests_ui_settings_patch.rs`; the fixed `CheckFailure.message` table must match `DEP_FAILURE_MESSAGES` in `src/ipc/fixtures/depTracker.ts` (or update it in the same increment); `registration_tests.rs` lists both commands. **Precondition: P125c1 (i)(ii) and P125d's builtin-npmrc / `pnpm_config__auth` items landed.** **security-auditor pass.** — gates: LOW-1 | `cargo nextest run -p bonsai deps::`; `cargo nextest run -p bonsai settings_defaults_parity`; `cargo clippy -p bonsai -- -D warnings` |
| **P125f** | frontend IPC (types per E6), mock + all fixtures (E5, incl. `mirror`, `large` and the long-content rows [UI-c]), `useDepTracker`, settings plumbing | AC13 vitest; `tsc`; settings parity | `pnpm vitest run src/ipc/mock/handlers/depTracker.test.ts src/hooks/useDepTracker.test.ts src/settings`; `pnpm tsc --noEmit` |
| **P125g** | UI per `P125-ui.md` | harness proof of every `?deps=` state, both themes | `pnpm gate --frontend` |

---

## 11. Open questions (flagged for the orchestrator)

- **Q1 — native-registry fallback.** Rec: none in v1 (2 hosts; deps.dev 404 ⇒ "unknown"). Alt: add
  per-ecosystem registries (npmjs, crates.io sparse index, pypi.org, proxy.golang.org, api.nuget.org)
  — authoritative yanked/deprecated data, +5 hosts in the consent copy. Revisit if the §3.3 yanked
  probe shows deps.dev flags a yanked version `isDefault`.
- **Q2 — corporate mirrors.** Any non-public registry/index/source (including a mirror of the public
  one) makes the affected deps `private` (conservative D7 reading). Repos behind Artifactory/Nexus
  may show mostly "private — not checked". Rec: accept for v1; a "treat host X as a public mirror"
  setting is a later phase.
- **Q3 — update kind for 0.x.** Rec: literal positions (`0.1 → 0.2` = minor). Alt: ecosystem
  compatibility semantics (Cargo/npm caret: `0.1 → 0.2` = major).
- **Q4 — multi-tab scoping.** Rec: active tab only; "opened" = first activation per process; tab
  switches rely on the interval. Alt: check every open tab on its own interval (conflicts with the
  spec's single-repo non-goal).
- **Q5 — Go private modules not listed in `GOPRIVATE`** will be sent (the same exposure `go` itself
  has to proxy.golang.org / sum.golang.org). Rec: accept, and say so in the consent copy (ui-designer).
- **Q6 — user-level config reads** (`~/.npmrc`, `pip.conf`, `NuGet.Config`, `$CARGO_HOME/config.toml`,
  Go env file) happen only to *withhold* more deps; their values never leave the machine. Rec: keep.

**RULED 2026-09-28 (user): all six recommendations accepted.** Q1 no native-registry fallback in
v1 · Q2 mirrors ⇒ `private` (mirror-allowlist setting is a later phase) · Q3 literal positions ·
Q4 active tab only · Q5 accept, disclose in consent copy · Q6 keep user-level config reads.

**Open after rev 2 (new):**
- **Q7 — config files above the workdir** (§0.2 residual). Cargo and NuGet walk to the filesystem
  root; the brief bounds our walk at the workdir, so a registry configured in, for example,
  `D:\Data\Repos\.cargo\config.toml` or `D:\NuGet.Config` is missed, and those deps are treated as
  public (fail-open). Rec: extend the **config-only** walk to the filesystem root for Cargo and
  NuGet, using the user-file read rules (bounded, regular file, key-filtered). Alt: keep the workdir
  bound and disclose it in the consent copy.
- **Q8 — OSV operator copy [UI-a].** No page explicitly names the operator (§0.1). Rec: use the
  UI contract's neutral fallback, "Both are public, free services for open-source package data."

**RULED 2026-09-28 (user): both recommendations accepted.** Q7 extend the config-only walk to the
filesystem root for Cargo and NuGet (user-file read rules: bounded, regular file, key-filtered) ·
Q8 neutral consent sentence, not "run by Google" (recorded in `P125-ui.md` §11.1).

**Q9 — RULED 2026-09-28 (user; raised in P125c1): symlinked config files ABOVE the workdir are
followed.** Applies to the above-workdir config-only walk (Q7 + the P125c1 npm/pnpm ancestor
walk) and the fixed user-level config files. Constraints (all mandatory): the final target must be
a regular file **after open**; read through `take(cap+1)`; key-filtered extraction only; **reject
Windows UNC / `\\?\UNC` / network-share targets** (would trigger SMB/NTLM); a dangling or
unresolvable symlink ⇒ that ecosystem fail-closed. **Inside the workdir, no-follow is unchanged
(MEDIUM-3).** Rationale: config signals can only withhold names; repo-controlled content is still
never followed. This amends §4.4 / §4.5 Q7 text and E4 row 6(f); `security-auditor` re-passes
the P125c1 round-2 diff.

**Q9 NARROWED — RULED 2026-09-28 (user; audit Q9-1, Unix automounter `/net/<host>` reach):**
symlinks are followed **only** for the fixed user/system config files and for ancestor
directories equal to `$HOME` / `$CARGO_HOME` (and the Go/pnpm config dirs they resolve to). Any
other symlinked config above the workdir — e.g. in a parent monorepo / superproject — makes that
ecosystem **fail closed**. All other Q9 constraints stand.

**P125h follow-ups → `P125-followups.md`** (2026-09-29). Rulings R1 (relax `registry.default` /
`CARGO_REGISTRY_DEFAULT`), R2 (all-malformed transitive ⇒ `not verified` row), R3
(network-profile lead from the exact flag only) and LOW-E1 (UNC-only; mapped drives deferred) are
recorded in its §10 and supersede the conflicting text here and in `P125-ecosystems.md` E3.
