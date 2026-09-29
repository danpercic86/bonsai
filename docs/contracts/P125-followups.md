# P125h — Dependency-tracker follow-ups (addendum to `P125-dependency-tracker.md` rev 3)

Scope: TODO.md 575–577 and 634–639, plus `P125-ui.md` §11.5. The rulings live here; the main
contract, `P125-ecosystems.md` and `P125-ui.md` are amended only as listed in §8. Wire changes are
**additive**: new fields use `#[serde(default)]`, so v1 cache files still load (`CACHE_SCHEMA` stays
`1`, which keeps AC7's instant report). No new command, event or channel. The mock IPC changes are
fixture-only (§7).

## 0. Shared wire delta (Rust `bonsai-deps/src/model.rs` ↔ TS `src/ipc/types/depTracker.ts`)

```rust
#[serde(rename_all = "camelCase")]
pub enum PrivateReason { CustomRegistry, ScopedRegistry, GoPrivate, SourceMapping, ConfigUnreadable,
                         LocalFeed /* §3: folder / file / UNC package source; registry_host always None */ }

#[serde(rename_all = "camelCase")]
pub enum UnknownReason { #[default] NotFound /* 404 or versionKey.name mismatch */, TooLarge /* §2 */ }

#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Availability {
    Checked,
    Unknown { #[serde(default)] reason: UnknownReason },   // was a unit variant; v1 JSON {"kind":"unknown"} ⇒ NotFound
    Local { reason: LocalReason },
    Private { reason: PrivateReason, registry_host: Option<String> },
}

#[serde(rename_all = "camelCase")]
pub enum VulnsUnverifiedReason {
    NoVersion,          // no resolved version (current None / version failed valid_version_chars)
    NameCasing,         // NuGet id casing unconfirmed and 0 OSV hits (MEDIUM-D1, rows.rs:268)
    MalformedAdvisory,  // ≥1 OSV hit for this package failed mint_id and was dropped (§1)
    ResponseTooLarge,   // ≥1 of its advisory records exceeded OSV_VULN_BODY_CAP (§2)
}

pub struct DepRow { /* …existing… */ #[serde(default)] pub vulns_unverified: Option<VulnsUnverifiedReason> }
pub struct DepReport { /* …existing… */ #[serde(default)] pub network_profile: bool /* §4 */ }
```
**Row invariant** (assert in `model_tests.rs`): `availability ∈ {Checked, Unknown}` ⇒
`vulns_checked == vulns_unverified.is_none()`; `Local | Private` ⇒ `vulns_checked == false &&
vulns_unverified == None` (the availability pill already explains it).

```ts
export type DepPrivateReason = 'customRegistry' | 'scopedRegistry' | 'goPrivate' | 'sourceMapping'
  | 'configUnreadable' | 'localFeed';
export type DepUnknownReason = 'notFound' | 'tooLarge';
export type DepVulnsUnverifiedReason = 'noVersion' | 'nameCasing' | 'malformedAdvisory' | 'responseTooLarge';
export type DepAvailability =
  | { kind: 'checked' } | { kind: 'unknown'; reason: DepUnknownReason }
  | { kind: 'local'; reason: DepLocalReason }
  | { kind: 'private'; reason: DepPrivateReason; registryHost: string | null };
// DepRow gains:    vulnsUnverified: DepVulnsUnverifiedReason | null;
// DepReport gains: networkProfile: boolean;
```
Rust always serializes the new fields, so TS declares them required. Old cache rows arrive as
`vulnsUnverified: null` / `networkProfile: false` / `reason: 'notFound'`; the UI fallbacks are in §4.
Known, accepted consequence of keeping `CACHE_SCHEMA = 1` (h1 review NIT, 2026-09-29): a report
cached *before* P125h that hit the §1 bug (`vulnsChecked: true`, `vulns: []` for an all-malformed
package) still loads and displays as clean until the next network check replaces it.

---

## 1. PRIORITY — all-malformed OSV hits read as clean (fail-open)

**Current.** `remote/osv.rs:139-149` `mint_refs` drops ids failing `mint_id` (`filter_map`).
`check/vulns.rs:93-103` builds the per-package list from the minted hits only. `check/rows.rs:268`:
`vulns_checked = version.is_some() && (osv_name_exact || !vulns.is_empty())`. So a package with
`osv_name_exact == true` whose every hit was dropped gets `vulns: []`, `vulns_checked: true` ⇒
"no vulnerabilities". (Non-exact NuGet casing already reads `false`.) A transitive package with
the same result gets no row (`rows.rs:299-300`) but is still counted in `transitive_checked`
(`rows.rs:386-389`).

**What the contract says.** Rev 3 is **silent** on this. §7.3 gives only the `mint_id` regex,
§5.3 maps "malformed JSON" to `BadJson`, and §7.5 has no rule on rejected ids. The claim "§7.5 said
such ids are dropped" is only in the audit (`audit-2026-09-28-P125-deps-contract.md:675`, LOW-D2)
and in the code comment `osv.rs:137-138`. Round 2 (audit line 693) accepted **drop, don't abort**.
This change keeps that and adds a row-level marker, so it contradicts no written rule. **No user
ruling is needed for the Rust behavior.** §8 writes the rule into §7.5. The transitive half is R2.

**Change (Rust owns it).**
```rust
// remote/osv.rs
pub struct PackageHits { pub refs: Vec<VulnRef>, pub dropped: u32 }
fn mint_refs(r: Vec<VulnRefDto>) -> (Vec<VulnRef>, u32);                  // dropped = rejected ids
pub fn query_chunk(..) -> Result<Vec<PackageHits>, CheckError>;           // dropped summed across pages
// check/rows.rs
pub(crate) struct VulnResult { pub advisories: Vec<Advisory>, pub unverified: Option<VulnsUnverifiedReason> }
pub(crate) type AdvisoryMap = BTreeMap<(Ecosystem, String, String), VulnResult>;
```
```
fetch_advisories, per package p with version:
  advs = [details[h.id].advisory for h in p.refs]; sort_advisories
  unverified = if p.dropped > 0                        → MalformedAdvisory
               elif any details[h.id].too_large (§2)   → ResponseTooLarge
               else None
direct_row (Public p):
  r = adv.get(key)                                     // None iff no version
  vulns_unverified = if p.version().is_none()                     → NoVersion
                     elif let Some(x) = r.unverified              → x
                     elif !d.osv_name_exact && r.advisories.is_empty() → NameCasing
                     else None
  vulns = r.advisories (or []);  vulns_checked = vulns_unverified.is_none()
transitive_row (Public p with version):
  emit iff !r.advisories.is_empty() || r.unverified.is_some()   // [R2]
  vulns_checked = r.unverified.is_none(); vulns_unverified = r.unverified
group.transitive_checked = |{ public transitive with version and r.unverified == None }|
```
`summarize` is unchanged: `vulnerable` counts rows with non-empty `vulns`, so an
all-malformed transitive row is shown but not counted. A malformed page token stays
`Service(BadJson)` (fail-safe; out of scope).

**Tests.** `remote/osv_tests.rs`: `mint_refs_counts_dropped` (2 good + 1 bad ⇒ 2 refs, dropped
1); pages accumulate `dropped`. `tests/check_tests.rs`:
`all_malformed_direct_is_not_verified` (`vulns_checked=false`, `malformedAdvisory`, `vulns=[]`);
`partial_malformed_keeps_good_advisories_and_is_not_verified`;
`all_malformed_transitive_row_is_emitted_unverified_and_not_counted` (R2; `transitive_checked`
excludes it, `summary.vulnerable` unchanged). `model_tests.rs`: row-invariant proptest/table.

---

## 2. Per-package `TooLarge` instead of aborting the whole check

**Current.** Every `TransportError::TooLarge` becomes `Service(TooLarge)` (`check/error.rs:44`)
through the `?` at `depsdev.rs:105` / `osv_vuln.rs:127` / `osv.rs:176`. `versions.len() >
MAX_VERSIONS` ⇒ `Service(TooLarge)` (`depsdev.rs:78-80`). `pool::run` aborts on the first closure
`Err` (`pool.rs:35-63`).

**Rule.**
| Cap | Constant | New behavior |
|---|---|---|
| deps.dev body 8 MiB | `DEPSDEV_BODY_CAP` | that package ⇒ `Unknown { reason: TooLarge }` (OSV still queried for it, as for any Unknown) |
| deps.dev `versions[] > 20 000` | `MAX_VERSIONS` | same as above |
| OSV vuln record 1 MiB | `OSV_VULN_BODY_CAP` | that advisory ⇒ `unknown_advisory(r)` (the 404 path), flagged `too_large`; each row carrying it ⇒ `ResponseTooLarge` |
| OSV querybatch 16 MiB | `OSV_BATCH_BODY_CAP` | **whole-check abort (unchanged)**. It covers ≤ 1000 packages and can't be tied to one. At ~80 B per `{id,modified}` it would already exceed `MAX_ADVISORIES`. |
| `MAX_ADVISORIES`, `OSV_MAX_PAGES`, `MAX_MANIFESTS`, `MAX_PARSE_BYTES` | — | unchanged (abort / ParseError as today) |
| **breaker** | `MAX_DEGRADED_RESPONSES = 32` (new, `check/mod.rs`) | once the per-check count of degraded responses (both rows above) goes past this ⇒ `Service(TooLarge)` abort. This catches a proxy that inflates every body. |

```rust
// remote/depsdev.rs
pub enum VersionsLookup { Found(Vec<RegistryVersion>), NotFound, TooLarge }
pub fn fetch_versions(t, p) -> Result<VersionsLookup, CheckError>;   // Err(TransportError::TooLarge) ⇒ Ok(TooLarge)
// remote/osv_vuln.rs
pub enum VulnDetail { Record(Advisory), TooLarge(Advisory /* = unknown_advisory(r) */) }
pub fn fetch_vuln(t, r) -> Result<VulnDetail, CheckError>;
// check/rows.rs
pub(crate) type LatestMap = BTreeMap<(Ecosystem, String), VersionsLookup>;
```
- The closures passed to `pool::run` return `Ok(TooLarge)`, never `Err`, so pool semantics do not
  change. A shared `AtomicU32` counts degraded responses. The closure that pushes it past the
  breaker returns `Err(Service(TooLarge))`.
- `direct_row`: `Found` ⇒ Checked; `NotFound` ⇒ `Unknown{NotFound}`; `TooLarge` ⇒ `Unknown{TooLarge}`.
- **Reuse guard.** `fetch_advisories` must not reuse a `TooLarge` placeholder from `ctx.prev`. The
  prev index (`vulns.rs:68-75`) **skips every advisory of a prev row whose
  `vulns_unverified.is_some()`** (any reason — a row with both a dropped id and an oversize advisory
  is tagged `MalformedAdvisory`, so matching only `ResponseTooLarge` would reuse its placeholder
  forever). This is conservative and costs at most a re-fetch.
- §5.3 row "oversize body ⇒ `Service(TooLarge)`" is amended to "querybatch only, or the breaker".

**Tests.** `remote_tests.rs`: `depsdev_too_large_is_a_lookup_not_an_error` (body and version
count); `vuln_too_large_is_placeholder`. `check_tests.rs`: `one_oversize_package_degrades_only_it`
(the other rows stay Checked, and the check succeeds);
`oversize_advisory_marks_rows_response_too_large`; `breaker_aborts_after_32_degraded`;
`querybatch_too_large_still_aborts`; `too_large_placeholder_is_refetched_next_run`.

---

## 3. Hostless NuGet `RestoreSources` ⇒ private `LocalFeed` (was Closed)

**Current.** `nuget_cfg.rs:219-222` `RestoreSink::text`: any value without a URL host ⇒
`Closed` ⇒ every NuGet dep is `Private(ConfigUnreadable)`. By contrast, a hostless
`packageSources` entry is already a private source (`nuget_cfg.rs:112-118`, `nuget_chain.rs:15-16`).
It ends up as `CustomRegistry` with no host (`nuget_chain.rs:133-135, 181-185` →
`registry_ctx.rs:233-235`), and so does `fallbackPackageFolders` (`nuget_chain.rs:130-131`).

**Change (Rust).**
```rust
// registry_ctx/nuget_cfg.rs (new, pure, text-only — no I/O on the value)
pub(crate) enum FeedRef { Host(String), LocalPath, Unresolved }
pub(crate) fn feed_ref(v: &str) -> Option<FeedRef>;   // None ⇒ Closed
```
```
feed_ref(v):
  extract_host(v) = Some(h)                                  → Host(h)
  v contains "$(" | "@(" | "%(" (MSBuild expansion)          → None (Closed, unchanged; could expand to a URL)
  v contains '%' (a %VAR% in nuget.config)                   → Unresolved
  v starts with "file:" | "\\" | "//" | "/" | "./" | "../" | ".\" | "..\"
    or matches ^[A-Za-z]:[\\/]                                → LocalPath
  v has no "scheme:" prefix (bare relative dir, e.g. "packages") → LocalPath
  else (ftp:, other scheme, garbage)                          → None (Closed)
```
- `RestoreSink.hosts: Vec<String>` becomes `feeds: Vec<FeedRef>`, and `Chain.restore_hosts`
  (`nuget_chain.rs:45`) becomes `restore_feeds: Vec<FeedRef>`. `NugetFile.sources` holds
  `(key, FeedRef)` instead of `(key, Option<String>)`.
- `FeedRef` holds **no path text**, only the variant (MEDIUM-1: never materialise a path).
- `EcoRegistry` (`registry_ctx.rs:57-78`) gains `pub(crate) all_reason: PrivateReason`. It defaults to
  `CustomRegistry`, and `private_reason` (`:233-235`) returns `(e.all_reason, e.default_host)`.
- `nuget_chain::evaluate` → `Outcome::All(PrivateReason, Option<String>)`. The reason is
  `CustomRegistry` when any non-public source in the chain is `Host(_)` or `Unresolved`, and
  `LocalFeed` when **every** non-public source is `LocalPath` (fallbackPackageFolders count as
  `LocalPath`). `resolve` folds the chains: if any chain gives `CustomRegistry`, the result is
  `CustomRegistry`; otherwise `LocalFeed`. The host is the first private host, else None.
- Source-mapped private patterns stay `SourceMapping` (unchanged).
- A UNC share is `LocalPath` by text: the value is never opened, so no SMB traffic.
- The other hostless sources (Cargo `directory` / `local-registry` `Def::Hostless`, pip
  `--find-links` paths) stay `CustomRegistry` in P125h. Extending `LocalFeed` to them is a
  follow-up, not a ruling.

**Tests** (`registry_ctx/nuget_*_tests.rs`): a `feed_ref` table covering every arm (incl. `C:\f`,
`\\srv\s`, `file:///x`, `packages`, `%FEED%`, `$(Foo)`, `ftp://x`); RestoreSources `./feed` ⇒ NuGet
not fail-closed, deps `Private(LocalFeed, None)`; a mix of local and `https://corp` ⇒
`CustomRegistry` + host; `fallbackPackageFolders` ⇒ `LocalFeed`; `$(Foo)` still `Closed`. The
`privacy_sentinel`/`token_sentinel` addition: a sentinel `ZzSentinel.Local` behind a RestoreSources
folder is never sent, and the folder path text appears in no DTO, cache byte or `Debug` output.

---

## 4. UI model gaps (`P125-ui.md` §11.5)

**(a) Local-feed reason** is `PrivateReason::LocalFeed` (§0, §3).

**(b) Report-level network profile.**
```rust
// registry_ctx/host_env.rs — trait HostEnv, NO default body (compile-enforced, like read_user_files)
/// True iff a user/system config base dir this env resolves (HOME/USERPROFILE, APPDATA,
/// LOCALAPPDATA, XDG_CONFIG_HOME, CARGO_HOME, PIP/uv/Go/NuGet user dirs) is a network path per
/// `fsread::follow::is_network_path` on its TEXT. No I/O, no canonicalize.
fn profile_on_network(&self) -> bool;
```
- Implementors to update (all four break until done): `RealHostEnv`
  (`src-tauri/src/deps/host_env.rs:197`, via `with_inputs` bases), `TestEnv`
  (`src-tauri/src/deps/test_support.rs:26`), `FakeHostEnv` (`registry_ctx/fake_env.rs:137`, builder
  `.network_profile(bool)`, default false), and `NoEnv` (`privacy_tests.rs:124`, false).
- `run_check` step 11 sets `report.network_profile = ctx.env.profile_on_network()`.
- **ASSUMED gap:** a mapped network drive letter (`H:\`, `DRIVE_REMOTE`) is not detected by a text
  check. That is the LOW-E1 follow-up (needs `windows-sys`), so the flag can be a false negative,
  never a false positive.
- **TS (`depsView.ts:268-284`):** `reportWideConfigLead(groups, networkProfile)` returns the
  `configUnreadable` ecosystems iff `networkProfile && ≥1 configUnreadable row`. The ≥2-ecosystem
  inference is **removed** (rec, R3). Otherwise per-group leads apply. For old cached reports
  (`networkProfile:false`) this only means the per-group leads show until the next check.
- Copy: ui-designer rewrites the §8.3 report-wide lead as an exact statement (the profile folder
  is on a network share). This is a required amendment before h4.

**(c) Why `vulnsChecked` is false** is `DepRow.vulns_unverified` (§0). TS `notVerifiedCause(row)`
(`depsView.ts:254-258`) becomes a lookup: `noVersion` ⇒ `NOT_VERIFIED_NO_VERSION`, `nameCasing` ⇒
`NOT_VERIFIED_NUGET`, `malformedAdvisory` / `responseTooLarge` ⇒ new strings (ui-designer), and
`null` (old cache) ⇒ `NOT_VERIFIED_OTHER`. `availabilityWords` `unknown` branch
(`depsView.ts:235-236`) picks the reason text by `a.reason` (new `tooLarge` string: ui-designer).
`privateReasonText` gains `localFeed` in `PRIVATE_REASON_NO_HOST`, with no with-host entry.
ui-designer should also revisit the `customRegistry`/null copy `custom or local feed` (§8.4 †).
After P125h, a null host there means an unknown host, an unresolved `%VAR%` or an invalid public
name, but no longer a local folder.

**Tests.** `model_tests.rs` (next to `:242`): wire names `localFeed`, `{"kind":"unknown","reason":
"tooLarge"}`, `vulnsUnverified`, `networkProfile`; **v1 back-compat**: v1 JSON
(`{"kind":"unknown"}`, a row without `vulnsUnverified`, a report without `networkProfile`)
deserializes to `NotFound` / `None` / `false`; `cache_tests.rs`: a v1 cache file still loads.
`host_env_tests.rs`: `APPDATA=\\srv\p\AppData\Roaming` ⇒ true; local ⇒ false.
`check_tests.rs`: the `FakeHostEnv.network_profile(true)` flag reaches the report. `depsView.test.ts`:
the lead appears only with `networkProfile`; there is no lead for 2 `configUnreadable` ecosystems
without it; the `notVerifiedCause` table; `localFeed` / `unknown.tooLarge` words.

---

## 5. `CARGO_REGISTRY_DEFAULT` / `registry.default` (user ruling R1)

**Current.** `cargo_cfg.rs:231-240` `check_default` sends any non-`crates-io` value ⇒ `Closed`.
Its callers are at `:251-253` (config key) and `:303-304` (env). `keep_cargo_env` keeps the var
(`:291`), and the test at `cargo_go_load_tests.rs:280-287` pins it. E3 row "Cargo" records it as
"VERIFIED, publish-only … read anyway, non-crates-io ⇒ fail closed".

**Finding (fetched 2026-09-29).**
- The Cargo config reference, `config.html#registrydefault`: type string, default `crates-io`, env
  `CARGO_REGISTRY_DEFAULT`. It is the registry used by default for registry commands such as
  `cargo publish`, and can be overridden with `--registry`. Nothing there ties it to dependency
  resolution.
- `specifying-dependencies.html`: dependencies are looked up on crates.io by default. A
  non-crates.io registry is selected per dependency with the `registry` key.
- `registries.html`: mirroring or vendoring crates.io is done with source replacement, which we
  already read.
- Independently, every locked package's real origin is in `Cargo.lock` `source`, and E2 already
  checks it.
- Conclusion: the key can only produce false "private" rows.

**Recommendation (relax).**
- Delete `check_default` and `ENV_DEFAULT`, and stop keeping `CARGO_REGISTRY_DEFAULT` in
  `keep_cargo_env`.
- Remove the `registry` table (`default`) from `ConfigDto`, so the key is **no longer
  materialised**. Reading fewer keys is the stricter MEDIUM-1 posture.
- A `[registry]` table is ignored, as is any other unmodelled key. A TOML syntax error still fails
  closed.
- Flip the test: `CARGO_REGISTRY_DEFAULT=corp` and `[registry] default="corp"` ⇒ **not** closed,
  and a crate with no `registry` key stays Public. Keep the `CARGO_REGISTRIES_CORP_INDEX` rows
  closed as today.
- Amend the E3 Cargo row. **This flips a written E3 rule, so it needs the user's ruling.**

---

## 6. E5 transitive+dev row; §2.3 wording

**Current.**
- `rows.rs:304` hard-codes `scope: DepScope::Normal` on transitive rows, and `Transitive`
  (`rows.rs:32-36`) has no scope.
- Only npm lock v2/v3 carries a dev flag. Cargo, Go, NuGet and uv locks don't.
- The fixture `depTrackerPopulated.ts:64-79` (`esbuild`, `transitive` + `dev`) is a row Rust can't
  emit.
- `rowTags` (`depsView.ts:199-204`) already shows only `transitive`.

**Ruling (architect; no user input needed): drop it from E5.**
- In `depTrackerModel.ts` `row()` (`:195`), force `scope: 'normal'` when the row is transitive,
  following the pattern already used for `declared`/`latest`/`update`.
- Remove `scope: 'dev'` from the esbuild row.
- The dev tag is still covered by the direct `vite`/`typescript` rows (`:30-31`).
- Keep `depsView.test.ts:113` as a guard.
- Carrying scope onto transitive rows is not worth it: one ecosystem out of five, a new adapter
  field, for a double tag.

**§2.3 wording fix:** "Derived UI states (computed in `deriveState`, `src/components/deps/depsView.ts`
— not in Rust or the hook)".

---

## 7. Mock / fixtures (E5 delta)

`RowSpec.vulnsChecked` is replaced by `vulnsUnverified?: DepVulnsUnverifiedReason`. `row()` sets
the fields as follows:
- `vulnsUnverified` = `!linkable ? null : current == null ? 'noVersion' : spec.vulnsUnverified ?? null`;
- `vulnsChecked` = `linkable && vulnsUnverified === null` (the §0 invariant, by construction);
- `unknown` availability defaults to `reason: 'notFound'`.

New modes are added. `DEP_VIEW_POPULATED` does not change, so its counts and tests stay the same:
| Mode / export | Content |
|---|---|
| `?deps=unverified` / `DEP_VIEW_UNVERIFIED` | one public row per `DepVulnsUnverifiedReason` (incl. an all-malformed **transitive** row with `vulns: []`), one `unknown` `tooLarge` row, one `private` `localFeed` row |
| `?deps=netprofile` / `DEP_VIEW_NETWORK_PROFILE` | `networkProfile: true`; npm + PyPI + NuGet groups, every row `private`/`configUnreadable` |

`depTracker.test.ts` (AC13) covers both modes, and the summaries match `summarizeFixture()`.

Existing `vulnsChecked: false` specs in `depTrackerPopulated.ts` must be rewritten: the NuGet rows
(`:146-147`) become `vulnsUnverified: 'nameCasing'`; the PyPI rows (`:182-183`) get `noVersion`
automatically from `row()`. The populated view renders the same.

---

## 8. Doc amendments (orchestrator / senior-dev applies them with `Edit`, one anchor at a time)
- Main §2.1 gets the §0 types. §5.3 gets the TooLarge row (§2). §7.5 gets "an OSV id failing
  `mint_id` is dropped; its package is `vulnsUnverified: malformedAdvisory` (P125h §1)", and its
  rule "`versions[]` > 20 000 ⇒ `Service`" (main contract line ~767) becomes a per-package
  `Unknown{TooLarge}` (§2) — the test at `remote_tests.rs:103` flips accordingly. §2.3 gets
  the §6 wording. §10/§11 get a pointer: "P125h follow-ups → `P125-followups.md`".
- E3 Cargo row: R1. E5: §6 + §7. E6: the §0 TS.
- `P125-ui.md` §8.3 / §8.4 / §11.5: ui-designer (new strings for `localFeed`, `tooLarge`,
  `malformedAdvisory`, `responseTooLarge`, the exact network-profile lead, and a revisit of the
  † copy).

---

## 9. Sub-increments (senior-dev)

| # | Scope | Files |
|---|---|---|
| **h1** | §1 + §2 + §4c Rust, §0 model | `crates/bonsai-deps/src/model.rs`, `model_tests.rs`, `remote/osv.rs`, `remote/osv_tests.rs`, `remote/depsdev.rs`, `remote/osv_vuln.rs`, `remote/remote_tests.rs`, `check/mod.rs` (breaker const), `check/vulns.rs`, `check/rows.rs` (split the verdict logic into `check/verdict.rs` if it would pass 500 lines), `cache_tests.rs`, `tests/check_tests.rs` |
| **h2** | §3 + §5 | `registry_ctx.rs`, `registry_ctx/nuget_cfg.rs`, `registry_ctx/nuget_chain.rs`, `registry_ctx/load_nuget.rs`, `registry_ctx/cargo_cfg.rs`, `registry_ctx/cargo_go_load_tests.rs`, the NuGet load tests, `tests/privacy_sentinel.rs`, `tests/token_sentinel.rs` — **h2's §5 part waits for R1** |
| **h3** | §4b | `registry_ctx/host_env.rs`, `registry_ctx/fake_env.rs`, `privacy_tests.rs`, `check.rs` (step 11), `src-tauri/src/deps/host_env.rs`, `host_env_tests.rs`, `test_support.rs` |
| — | ui-designer amends `P125-ui.md` (§8) | docs only |
| **h4** | TS mirror + UI + fixtures | `src/ipc/types/depTracker.ts`, `src/ipc/fixtures/depTrackerModel.ts`, `depTrackerPopulated.ts`, a new `src/ipc/fixtures/depTrackerUnverified.ts` (both new modes), `src/ipc/mock/handlers/depTracker.ts` (+test), `src/components/deps/depsView.ts` (+test), `depsCopy.ts`, the `DependenciesDialog` lead call site |

- **Gates:**
  - h1–h3: `cargo nextest run -p bonsai-deps`, `cargo test -p bonsai-deps --doc`, and clippy on
    both crates. h3 also runs `cargo nextest run -p bonsai deps::`.
  - h4: `pnpm vitest run src/components/deps src/ipc/mock/handlers/depTracker.test.ts`,
    `pnpm tsc --noEmit`, and harness `?deps=unverified|netprofile`.
  - `dep-tree.txt` must stay byte-identical (no new dependency).
- **Reviews:**
  - h1–h3 touch the D7/MEDIUM-1 surface (`registry_ctx`, `remote`). No mandatory path trigger
    fires, but I recommend one `security-auditor` pass over h1–h3 combined.
  - h4 gets `reviewer` + `ui-designer`.

## 10. Rulings needed from the user

| # | Question | Recommendation |
|---|---|---|
| **R1** | Relax `CARGO_REGISTRY_DEFAULT` / `registry.default` (stop reading it). This flips E3's "read anyway ⇒ fail closed". | **Relax.** Cargo documents it for publish-type commands only. Resolution defaults to crates.io, with a per-dependency `registry` key. The lock `source` is already checked. |
| **R2** | A transitive package whose OSV hits were all malformed: show it as a `not verified` transitive row (vulns empty), or hide it (AC4 literal: "no advisory ⇒ not listed")? | **Show it** and exclude it from `transitive_checked`. OSV did report hits; hiding them re-creates the fail-open for transitives. |
| **R3** | The network-profile lead: exact flag only, or keep the ≥ 2-ecosystem inference as a fallback? | **Flag only.** The inference already blamed the wrong folder once (P125g review MUST-FIX). A false-negative mapped drive falls back to the accurate per-group leads. |

**RULED 2026-09-29 (user):** R1 **relax**; R2 **show the `not verified` row**; R3 **flag only**.
Also ruled: LOW-E1 (skip Windows `DRIVE_REMOTE` PATH roots) — **UNC-only rejection stays as is;
mapped network drive letters deferred** (no `unsafe`, no new `windows-sys` dep).

Not rulings (architect decisions): `Unknown.reason` is added (§0), `CACHE_SCHEMA` stays 1, the
querybatch cap stays an abort, the breaker is set at 32, and E5 drops the dev tag from the
transitive row.
