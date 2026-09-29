# P125 — Companion tables (split from `P125-dependency-tracker.md`, rev 3)

This file holds material moved out of the main contract so that it stays under the size cap:
- **E1–E2:** the per-ecosystem tables, read by the `version/*`, `eco/*` and `registry_ctx.rs`
  implementers (P125a/b/c1/c2).
- **E3:** the registry-config inventory (main §0.2).
- **E4:** the AC → test table (main §9).
- **E5:** the mock fixture spec (main §8, read by P125f).
- **E6:** the TypeScript mirror of the main §2.1/§2.2 types (main §2.3, read by P125f).

Everything here is subordinate to the main contract. That file keeps the Rust types, the privacy
rules (§7.3), the file-read rules (§4.5, MEDIUM-3) and the YAML limits (§4.2, HIGH-2).

## E1. Version schemes (main §3)

| Scheme | Used by | Parse | Pre-release | Order |
|---|---|---|---|---|
| `semver_s` | npm, Cargo | `semver::Version` after trimming a leading `v`/`=` (npm loose) | `!pre.is_empty()` | SemVer 2 (build ignored) |
| `pep440` | PyPI | `[N!]N(.N)*[{a|b|rc}N][.postN][.devN][+local]`, case-insensitive; normalizes `alpha/beta/c/pre/preview` and the `-`/`_` separators. **Iterative, no recursion (HIGH-2).** | has `a/b/rc` or `.dev` | PEP 440 total order (epoch, release zero-padded, pre < none < post, dev before its release) |
| `go` | Go | `vMAJOR.MINOR.PATCH[-pre][+incompatible]`; pseudo-versions (`vX.Y.Z-yyyymmddhhmmss-abcdef123456`) parse as pre-release | `-pre` or pseudo | SemVer 2 on the `v`-stripped core; `+incompatible` ignored for order |
| `nuget` | NuGet | `N.N[.N[.N]][-pre][+meta]`, 4th segment allowed, case-insensitive prerelease labels | `-pre` | NuGet SemVer 2: numeric segments (missing = 0), then prerelease (case-insensitive) |

`segments` = the first three numeric release components (missing = 0). A PEP 440 epoch ≠ 0 ⇒ Major.
A NuGet 4th segment ⇒ Patch.

## E2. Ecosystem rules (main §4.3)

Every file read goes through `safe_read` (main §4.5). Every registry config is read by the
key-filtered extractor (main §4.4, MEDIUM-1) from the E3 inventory. A config that is unreadable,
oversize, symlinked or unparseable **fails closed** (HIGH-1). **Local-name rule (HIGH-1):** before
any row below applies, a dependency whose normalized name is declared by any manifest of the same
ecosystem in the repo is `Local(Workspace)`.

| Eco | Manifests | Lockfiles (resolved + transitive) | Direct deps | declared ⇒ resolved? | Local | Private |
|---|---|---|---|---|---|---|
| npm | `package.json` (declared name: `name`) | `package-lock.json`/`npm-shrinkwrap.json` (v2/v3 `packages{}`; v1 `dependencies{}` tree), `pnpm-lock.yaml` (v6–v9 `importers[<rel dir>]` + `packages`/`snapshots`), `yarn.lock` (v1 custom format, **iterative line parser**; berry = YAML with `__metadata`) | `dependencies`, `devDependencies`(dev), `optionalDependencies`(optional), `peerDependencies`(peer) | no | spec `file:` `link:` `portal:` ⇒ Path; `workspace:` ⇒ Workspace; `git+…`, `github:`, `user/repo`, `git://` ⇒ Git; `http(s)://` tarball ⇒ Url; npm-lock `link:true` or `resolved` `file:` ⇒ Workspace/Path | lock `resolved`/`tarball` host ∉ {`registry.npmjs.org`, `registry.yarnpkg.com`} ⇒ CustomRegistry. `@scope/x` with a scope registry in any npm/yarn/pnpm config or env (E3) ⇒ ScopedRegistry. A non-public default registry in any config/env ⇒ every unscoped dep without a public lock URL is CustomRegistry. **pnpm v6–v9 locks omit the registry host (HIGH-1), so for pnpm the config signal is the only signal.** |
| Cargo | `Cargo.toml` (`[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, `[target.*.…]`, `[workspace.dependencies]`; declared name: `[package].name`) | `Cargo.lock` (`[[package]]` name/version/source) | the tables listed | no | `path=` ⇒ Path; `git=` ⇒ Git; `workspace = true` ⇒ resolved via the root `[workspace.dependencies]`; a lock entry without `source` ⇒ Workspace | `registry = "x"` ⇒ CustomRegistry. Lock `source` ∉ {`registry+https://github.com/rust-lang/crates.io-index`, `sparse+https://index.crates.io/`} ⇒ CustomRegistry. `[source.crates-io] replace-with` (in any `.cargo/config{,.toml}` of the E3 walk — in-repo ancestors **and, per Q7, every ancestor above the workdir to the filesystem root** — or `$CARGO_HOME`, or env `CARGO_SOURCE_CRATES_IO_REPLACE_WITH`) ⇒ **all** CustomRegistry. |
| PyPI | `requirements*.txt`, `pyproject.toml` (PEP 621 `project.dependencies`/`optional-dependencies`, PEP 735 `dependency-groups`, `tool.poetry.*dependencies`/groups; declared name: `project.name` / `tool.poetry.name`) | `poetry.lock`, `uv.lock` (TOML `[[package]]`) | as listed; `requirements` lines skip `-r`/`-c`/options | `==X` (no wildcard) ⇒ resolved | `-e`, a local path, `name @ file:`, `{path=…}`, uv `source={path|directory|editable|virtual}` ⇒ Path/Workspace; `git+`/`{git=…}` ⇒ Git; `name @ https://…` ⇒ Url | requirements `--index-url`/`--extra-index-url`/`-i` non-pypi ⇒ **every dep in that file** is CustomRegistry. Poetry `[[tool.poetry.source]]` non-pypi with priority primary/default/supplemental ⇒ all CustomRegistry; `explicit` ⇒ only the deps pinned to it. poetry.lock `[package.source]` ⇒ CustomRegistry. uv `source={registry=X}` with X ≠ `https://pypi.org/simple` ⇒ CustomRegistry. `[[tool.uv.index]]` with `default = true` non-pypi ⇒ all CustomRegistry. Any pip/uv env var or config in E3 naming a non-pypi index ⇒ all CustomRegistry. Names are PEP 503-normalized. `[P125c2 review]` PDM `[[tool.pdm.source]]` and Rye `[[tool.rye.sources]]` with a non-pypi `url` ⇒ all CustomRegistry; a hostless or malformed entry ⇒ fail closed (Q2) — ASSUMED from the tools' docs (no local install). An includer's (`-r`/`-c`) own index / `--no-index` / find-links options ⇒ all CustomRegistry (they also route the included files' groups). A bare archive line (`*.whl`, `*.tar.gz`, `*.tgz`, `*.tar.bz2`, `*.zip`) is a location, never a registry name. |
| Go | `go.mod` (declared name: `module`) | `go.mod` itself (≥1.17 lists `// indirect`); `go.sum` ignored | `require` without `// indirect` = direct; with it = transitive | **yes** (MVS-selected versions) | `replace X => ./path` ⇒ Path | A module path matching a glob in `GOPRIVATE`/`GONOPROXY`/`GONOSUMDB` (env, else the GOENV file, E3) ⇒ GoPrivate. `replace X => Y vN` ⇒ query Y. |
| NuGet | `*.csproj`/`*.fsproj`/`*.vbproj` `<PackageReference Include Version>`, `Directory.Packages.props` `<PackageVersion>` (CPM), `packages.config` (declared name: `<PackageId>`, else the project file stem) | `packages.lock.json` (`dependencies.<tfm>.<id>{type: Direct|Transitive|Project, resolved}`) | PackageReference / packages.config | a plain `1.2.3` and packages.config ⇒ resolved (NuGet lowest-applicable); ranges/floating (`[..)`, `*`) ⇒ declared | `<ProjectReference>` is never a dep; lock `type: Project` ⇒ Workspace | Effective sources come from the nuget.config chain (E3 — in-repo ancestors, **every ancestor above the workdir to the filesystem root per Q7**, user, machine-wide), honouring `<clear/>` and `<disabledPackageSources>`, plus `<RestoreSources>`/`<RestoreAdditionalProjectSources>` in project files and `Directory.Build.props`. Any effective source ≠ `https://api.nuget.org/v3/index.json` ⇒ every package is CustomRegistry, **unless** `<packageSourceMapping>` maps its id pattern only to nuget.org. A package mapped to a private source ⇒ SourceMapping. The XML DTO does **not** model `<packageSourceCredentials>` (MEDIUM-1). |

**Not tracked (`catalog.rs`):** `pom.xml`→"Maven", `build.gradle`/`build.gradle.kts`→"Gradle",
`composer.json`→"Composer", `pubspec.yaml`→"Pub", `Gemfile`→"RubyGems", `Package.swift`→"SwiftPM",
`Podfile`→"CocoaPods", `mix.exs`→"Hex". One `NotTracked` group per file, zero rows, no parse.

**Group assembly:** one group per manifest. Transitive lock entries attach to the group whose
manifest sits in the lockfile's directory. If there is none, they get a group keyed by the lockfile
path. Transitive = lock entries not matched (by name and version) by any direct dep of that
lockfile's manifests.

## E3. Registry-config inventory — every path and env var read (main §0.2) `[HIGH-1 item 4]` `[Q7]`

> **P125h amendments (2026-09-29):** Cargo — `[registry] default` and `CARGO_REGISTRY_DEFAULT`
> are **no longer read** (user ruling R1; publish-only per the Cargo reference); the "read anyway ⇒
> fail closed" rule for them in the Cargo row is superseded. NuGet — hostless feeds (folder, UNC,
> `file:`, relative) make packages private `localFeed` instead of failing closed (P125h §3).

All entries are **ASSUMED** from each tool's published configuration docs; none were fetched in the
contract session. P125b/c1/c2 must verify each row against the tool docs and flip it to VERIFIED here.

The walk columns:
- **Repo walk:** a `safe_read` ancestor walk from each manifest's directory **up to and including
  the workdir** (main §4.5), whether the file is tracked, untracked or **gitignored**.
- **Above-workdir walk (Q7, Cargo and NuGet only):** every ancestor directory *above* the workdir, up
  to and including the filesystem root (drive root on Windows). **Config files only** — manifests and
  lockfiles stay workdir-bounded (MEDIUM-3).

Read rules for the above-workdir walk (`HostEnv::read_above_workdir`) and for user/system files
(`HostEnv::read_user_file`):
- regular file only;
- `[Q9-narrowed]` symlinks **followed** only for the fixed user/system files and for above-workdir entries in `$HOME` / `$CARGO_HOME`; any other symlinked above-workdir config ⇒ Unreadable; a UNC / network / DOS-device target, a dangling link or a non-regular target ⇒ Unreadable (in-workdir reads stay no-follow); a workdir on a UNC share fails the whole above-workdir walk closed (intentional);
- a 1 MiB bounded `take` read;
- the key-filtered extractor with host-only `extract_host`, userinfo stripped (main §4.4);
- any Unreadable / TooLarge / unparseable result ⇒ **fail-closed** `Private(ConfigUnreadable)` for
  the whole ecosystem.

`~` is the user home. `%X%` is Windows. `$XDG_CONFIG_HOME` defaults to `~/.config`.

| Eco | Repo walk (file names) | Above-workdir walk (Q7) | User / system files (per OS) | Env vars |
|---|---|---|---|---|
| npm / pnpm | `.npmrc` — **VERIFIED** (npm docs `npmrc.md`: per-project file at the project root; P125b reads every in-repo ancestor, a superset). **`pnpm-workspace.yaml` — VERIFIED for pnpm 11, not 10** (vendored pnpm 11.17.0 `CHANGELOG.md`: 11.0.0 adds `registries:` with `default` + `"@scope"` keys; 11.1.0 adds `namedRegistries:` alias → URL, used as `alias:` specifier protocols; `registry` is also read from it). P125b reads `registry`, `registries`, `namedRegistries` fail-closed like `.npmrc`; any alias or anchor in the file fails closed. | — | `$NPM_CONFIG_USERCONFIG` else `~/.npmrc` — **VERIFIED** (npm docs `userconfig`). Global `$PREFIX/etc/npmrc` — **VERIFIED** (npm docs `globalconfig`); the per-OS default prefix ("where the node executable is installed"), hence `%APPDATA%\npm\etc\npmrc` / `/usr/local/etc/npmrc`, stays ASSUMED. pnpm rc paths: ASSUMED. **Found in P125b:** npm's builtin `/path/to/npm/npmrc` — **read since P125d** (`UserConfig::NpmBuiltinRc`, npmrc key filter, unreadable ⇒ fail closed; the path, next to the `node` executable, is resolved by the P125e real `HostEnv`: ASSUMED); pnpm 11 no longer reads `$PREFIX/etc/npmrc`. **pnpm 11 global `config.yaml` — VERIFIED, read since P125c1** (`UserConfig::PnpmGlobalYaml`; vendored pnpm 11.17.0 `config/reader/lib/dirs.js getConfigDir`: `$XDG_CONFIG_HOME/pnpm` when set, else mac `~/Library/Preferences/pnpm`, other Unix `~/.config/pnpm`, Win `%LOCALAPPDATA%\pnpm\config` (else `~/.config/pnpm`); file `config.yaml`, read with the workspace-manifest reader, its `registries` merged). Its `npmrcAuthFile` key redirects the user npmrc ⇒ fail closed. Also found in P125c1, not read: pnpm 11 merges `<configDir>/auth.ini` as a full npmrc (can carry `@scope:registry`). | `npm_config_*`, case-insensitive — **VERIFIED** (npm docs `config.md`). **`pnpm_config_*` — VERIFIED** (pnpm 11 reads it *instead of* `npm_config_*`); P125b applies the same key filter (`…registry`, `…@*:registry`) to both prefixes. `pnpm_config__auth` (routes registries but carries tokens): **since P125d a presence-only check** (`HostEnv::var_present_ci`, the value is never read) fails npm closed. |
| yarn v1 / berry | `.yarnrc` (v1): ASSUMED. `.yarnrc.yml` — **VERIFIED** (yarn 4.7.0 bundle: `DEFAULT_RC_FILENAME = ".yarnrc.yml"`, renamable only via `YARN_RC_FILENAME`, which P125b treats as fail-closed) | **berry — VERIFIED, deviation from "—":** yarn 4.7.0 `findRcFiles` reads `.yarnrc.yml` in every directory from the cwd up to the filesystem root. P125b reads it above the workdir through `read_above_workdir` (needs ratification). v1: ASSUMED none. | `~/.yarnrc`, `~/.yarnrc.yml` (all OS): ASSUMED | `yarn_` prefix, case-insensitive — **VERIFIED** (yarn 4.7.0). `YARN_NPM_REGISTRY_SERVER` — **VERIFIED** (setting `npmRegistryServer`, default `https://registry.yarnpkg.com`); `npmScopes` is a MAP setting — **VERIFIED**, but its env encoding is not, so `YARN_NPM_SCOPES` set ⇒ fail closed. `YARN_REGISTRY` (v1): ASSUMED. |
| Cargo | `.cargo/config.toml`, `.cargo/config` (legacy) — **VERIFIED** (cargo book `reference/config.html`, 1.97 toolchain docs: "Hierarchical structure"; the extensionless file is still read and wins when both exist — P125c1 reads both, a superset) | `.cargo/config.toml`, `.cargo/config` — **VERIFIED** (same section: cargo probes every parent directory up to `/.cargo/config.toml`). P125c1 no-follows every component, incl. a linked `.cargo` dir. | `$CARGO_HOME/config.toml`, `$CARGO_HOME/config`; `CARGO_HOME` defaults to `~/.cargo` — **VERIFIED** (Windows `%USERPROFILE%\.cargo`, Unix `$HOME/.cargo`). `credentials{,.toml}` is never read (no `UserConfig` variant). Keys read: `source.*.{replace-with,registry}`, `registries.*.index`, `registry.default`; a top-level `include`, `[patch]` or `paths` ⇒ fail closed (config `include` / `[patch]` VERIFIED in the same page). | `CARGO_REGISTRIES_<name>_INDEX` — **VERIFIED**. `CARGO_REGISTRY_DEFAULT` — **VERIFIED, publish-only** ("registry commands like cargo publish"); read anyway, non-crates-io ⇒ fail closed. `CARGO_SOURCE_CRATES_IO_REPLACE_WITH` — **not honoured by cargo** (`source.<name>.replace-with`: "Environment: not supported"); read anyway for the E4(b) sentinel (can only add private). `CARGO_HOME` is path resolution only (P125e). |
| pip | (requirements `-i`/`--index-url` inline; no repo file) | — | `$PIP_CONFIG_FILE`. Win `%APPDATA%\pip\pip.ini`, `~\pip\pip.ini`, `%PROGRAMDATA%\pip\pip.ini`. mac `~/Library/Application Support/pip/pip.conf`, `~/.config/pip/pip.conf`, `~/.pip/pip.conf`, `/Library/Application Support/pip/pip.conf`. Linux `$XDG_CONFIG_HOME/pip/pip.conf`, `~/.pip/pip.conf`, `/etc/pip.conf`, `/etc/xdg/pip/pip.conf` — **VERIFIED** (pip 25.0.1 source, `_internal/configuration.py` `get_configuration_files` + `utils/appdirs.py`: GLOBAL = `site_config_dirs` (+ `/etc` on Unix), USER = legacy `~/pip` (Win) / `~/.pip` + `user_config_dir`, mac user dir = `~/Library/Application Support/pip` if it exists else `~/.config/pip`; basename `pip.ini` on Windows, `pip.conf` elsewhere; INI keys lowercased, `_` → `-`, `--` prefix dropped). Found in P125c2, not read: pip skips the USER files when `PIP_CONFIG_FILE` exists (P125c2 reads them anyway, a superset), and pip's SITE file `sys.prefix/pip.{ini,conf}` (the virtualenv's) is not in this inventory and cannot be located without a process. `[P125c2 review]` the in-repo venv's SITE file IS read in the repo walk: `.venv/pip.{conf,ini}`, `venv/pip.{conf,ini}` (`safe_read`, no-follow, pip INI key filter); residual: a venv outside the repo (`$VIRTUAL_ENV`, P125e). Also read, `[P125c2 review]`, ASSUMED: `pyproject.toml` `[[tool.pdm.source]]` / `[[tool.rye.sources]]` `url` (repo walk). | `PIP_INDEX_URL`, `PIP_EXTRA_INDEX_URL`, `PIP_CONFIG_FILE` — **VERIFIED** (same file: `PIP_<NAME>` maps to option `<name>`); P125c2 also reads `PIP_FIND_LINKS` (can only add private) |
| uv / Poetry | `uv.toml`; `pyproject.toml` `[[tool.uv.index]]`, `[[tool.poetry.source]]` | — | uv: Win `%APPDATA%\uv\uv.toml`, `%PROGRAMDATA%\uv\uv.toml`; mac/Linux `$XDG_CONFIG_HOME/uv/uv.toml`, `/etc/uv/uv.toml` | `UV_INDEX_URL`, `UV_EXTRA_INDEX_URL`, `UV_INDEX`, `UV_DEFAULT_INDEX` |
| Go | — | — | `$GOENV` else Win `%APPDATA%\go\env`, mac `~/Library/Application Support/go/env`, Linux `$XDG_CONFIG_HOME/go/env`, else `~/.config/go/env` — ASSUMED (P125c1: no Go toolchain or docs available locally; the `KEY=VALUE` line format is ASSUMED too) | `GOPRIVATE`, `GONOPROXY`, `GONOSUMDB`, `GOINSECURE` (globs ⇒ GoPrivate), `GOPROXY` (any entry but `https://proxy.golang.org` / `direct` / `off` ⇒ all CustomRegistry with that host, Q2), `GOSUMDB` (`off` or non-`sum.golang.org` ⇒ fail closed), `GOENV` — ASSUMED; each read from env AND the GOENV file (union, a superset of Go's env-wins rule) |
| NuGet | `nuget.config` / `NuGet.Config` / `NuGet.config` (case-insensitive); `<RestoreSources>` / `<RestoreAdditionalProjectSources>` in `Directory.Build.props` and project files — **config walk VERIFIED on Windows** (.NET 10.0.303 SDK, `dotnet nuget config paths` + `dotnet nuget list source` on scratch configs under `D:\Data\Temp`: every ancestor of the project dir is read; `<clear/>` drops every farther level incl. the user file; `disabledPackageSources` key match is case-sensitive, value `true` case-insensitive; section names are case-sensitive; `add` any case; `<remove>` ignored; a DTD is rejected by NuGet itself). The three-name set on case-sensitive systems and RestoreSources: ASSUMED. | `nuget.config` / `NuGet.Config` / `NuGet.config` (case-insensitive) — **VERIFIED on Windows** (same probe: the walk continues above the repo to the drive root) | User: Win `%APPDATA%\NuGet\NuGet.Config` — **VERIFIED** (same probe); mac/Linux `~/.nuget/NuGet/NuGet.Config`: ASSUMED. Machine-wide: Win `%ProgramFiles(x86)%\NuGet\Config\*.config`, mac `/Library/Application Support/NuGet/Config/*.config`, Linux `/etc/opt/NuGet/Config/*.config`: ASSUMED (the dir does not exist on the probe host, and `config paths` lists existing files only). P125c2 reads the set through `HostEnv::read_user_files` (no default body: every HostEnv implements it). `[P125c2 review]`: (a) `Directory.Build.props` is also read in the Q7 above-workdir walk, RestoreSources-only (never a manifest there; Q9-narrowed read rules); (b) a feed with no URL host (local folder, UNC, `%VAR%`) is a hostless NON-PUBLIC source (no mapping ⇒ all private; mapped ⇒ only its patterns), and any `fallbackPackageFolders` entry ⇒ all private (not subject to mapping); malformed XML stays fail-closed; (c) files of one level disagreeing on `<clear/>` ⇒ fail closed. All ASSUMED beyond the Windows probe above. | — |

## E4. Test plan (AC → proof) (main §9)

| AC | Test (file) |
|---|---|
| 1 | `deps/exec_tests.rs::disabled_makes_zero_requests` — 0 FakeTransport sends across activation + 10 ticks + `deps_get_view`; `deps_check_now` rejects. Mock default ⇒ `DEP_VIEW_OFF`. |
| 2 | `check_tests.rs::npm_pnpm_plus_cargo_two_groups` — git2-built fixture repo + canned responses ⇒ 2 groups; `summary` equals an independent recount. |
| 3 | `version/*_tests.rs` tables (1.2.3 vs 1.2.5 / 1.4.0 / 2.0.0 per scheme; PEP 440 order; `select_latest` pre/deprecated/default cases); proptest `classify_update(v,v) == None`. |
| 4 | `check_tests.rs::transitive_only_when_vulnerable`, `unknown_row_keeps_vulns` [UI-e], `advisory_severity_mapping`; `cvss_tests.rs` (FIRST 3.1 vectors). |
| 5 | `check_tests.rs::not_tracked_gradle_isolated`; a malformed `Cargo.lock` ⇒ only that group `ParseError{invalidSyntax}`. |
| 6 | **`tests/privacy_sentinel.rs`** `[HIGH-1 item 5]`. A full `run_check` over a recording FakeTransport; no sentinel substring (case-insensitive) may appear in any URL or body — matched **after percent-decoding every URL and JSON-decoding every body, and additionally on each sentinel's bare marker** (e.g. `zzign`, so `%40zzign%2Fpkg` cannot pass) `[NEW-1]` — and the rows must read local/private. Sentinels: `zz-sentinel-path` (npm `file:`), `zz-sentinel-ws` (`workspace:*`), `@zzsentinel/scoped` (tracked `.npmrc` scope), `zz_sentinel_git` (Cargo `git=`), `zz-sentinel-reg` (Cargo `registry="corp"`), `zz-sentinel-idx` (requirements `--index-url`), `corp.invalid/zzsentinel/mod` (`GOPRIVATE`), `ZzSentinel.Pkg` (private nuget.config), a transitive private lock entry, **plus:** (a) `@zzign/pkg` whose scope lives only in a **gitignored** `.npmrc`, with a pnpm-v9 lock without tarball URLs; (b) one **env-only** registry per ecosystem (`npm_config_@zzenv:registry`, `CARGO_SOURCE_CRATES_IO_REPLACE_WITH`, `PIP_INDEX_URL`, `UV_DEFAULT_INDEX`, `GOPRIVATE`; NuGet has no env var, so an unparseable machine-wide config (FakeHostEnv) stands in); (c) a **yarn v1 workspace member** `@zzws/b: ^1.0.0` with no lock entry ⇒ Local(Workspace); (d) an **unparseable `nuget.config`** ⇒ every NuGet dep `Private(ConfigUnreadable)`; (e) a **symlinked** `.npmrc` ⇒ fail-closed; **(f) `[Q7]` the workdir is a subdirectory of a tempdir whose parent holds `.cargo/config.toml` with `[source.crates-io] replace-with = "corp"` + `[source.corp] registry = "https://u:zz-sentinel-token@cargo.corp.invalid/index"`, and whose parent holds a `NuGet.Config` adding `https://nuget.corp.invalid/v3/index.json` ⇒ Cargo crate `zz-sentinel-above` and NuGet `ZzSentinel.Above` are never sent, their rows read `private` with `registryHost` `cargo.corp.invalid` / `nuget.corp.invalid` (no userinfo), and `[Q9-narrowed]` a symlinked parent-dir config ⇒ `Private(ConfigUnreadable)` unless that parent is `$HOME` / `$CARGO_HOME` (then followed, same rows); a link to a directory / dangling / UNC target ⇒ `Private(ConfigUnreadable)` everywhere.** Plus both `compile_fail` doctests. |
| 6′ | **`tests/token_sentinel.rs`** `[MEDIUM-1]` — `zz-sentinel-token` goes into every FakeHostEnv user config (`_authToken`, pip `index-url = https://u:zz-sentinel-token@…`, NuGet `packageSourceCredentials`, Cargo `[registry] token`), into `PIP_INDEX_URL`, into a malformed repo manifest line, and into the Q7 parent-dir configs. It must be absent from outbound URLs/bodies (percent- and JSON-decoded first, per row 6 `[NEW-1]`), the cache file bytes, the serialized `DepTrackerView`, and `format!("{:?}")` of every `ParseError` / `CheckError`, and `registry_host` values contain no `@`. |
| 7 | `cache_tests.rs::roundtrip_and_rejects`; `exec_tests.rs::view_returns_cached_before_network`. |
| 8 | `deps/plan_tests.rs` table; `exec_tests.rs::manual_runs_regardless_of_interval`; `deadline_times_out_and_discards` [HIGH-2]. |
| 9 | `exec_tests.rs::manifest_edit_marks_stale_without_check`; `discover_tests.rs::probe_stale_*`. |
| 10 | spawn_blocking (reviewer); harness `?deps=stale` → `depsCheckNow`: old report visible, frame timing unchanged. |
| 11 | `tests/no_process.rs` (tokens + `bonsai_core` allowlist) [MEDIUM-4]; clippy `-D warnings`; the `dep-tree.txt` snapshot matches. |
| 12 | `exec_tests.rs::disable_cancels_and_wipes` — **all 6 workers blocked** in the FakeTransport [LOW-1]; after disable: zero new sends, no persist, no stale-epoch `Progress`, `Cleared`, directory absent; startup wipe. |
| 13 | `depTracker.test.ts` — every `?deps` mode (incl. `mirror`, `large`); summaries match recounts; event sequence. |
| — | `yaml_events_tests.rs` `[HIGH-2]`: `yaml_alias_bomb ⇒ ParseError{UnsupportedFeature}` in < 100 ms; `yaml_anchor_only ⇒ UnsupportedFeature`; `yaml_deep_nesting ⇒ LimitExceeded`; `toml_deep_nesting ⇒ LimitExceeded`; a 10 MB single-line yarn v1 file parses without recursion. |
| — | `fsread_tests.rs` `[MEDIUM-3]` (unix-gated where needed): symlink manifest ⇒ skipped; FIFO ⇒ `NotAFile` without blocking; `/dev/zero` symlink ⇒ `NotAFile`; symlink escaping the workdir ⇒ `NotAFile`; `..` / absolute / drive-prefix index paths ⇒ dropped; over-cap ⇒ `TooLarge` via `take`; `[Q7]` the above-workdir walk reads configs only (a parent-dir `Cargo.toml` / `package.json` is never discovered). |
| — | `privacy_tests.rs` `[LOW-2]`: `..`, `.`, `foo/../bar` (Go), a Go first element of `..`, NuGet `..` ⇒ not Public. `model_tests.rs` `[LOW-3]`: U+202E / U+200B are stripped, 257 chars → 256. |
| — | **P125d live fixtures** `[LOW-4]`: one known-vulnerable pinned version per ecosystem (npm, Cargo, PyPI, Go with a GO-/GHSA- id, NuGet); a test asserts ≥ 1 advisory each. The Go fixture closes the `v`-prefix ASSUMED row. |
| — | Repo switch: `exec_tests.rs::switch_cancels_old_run`. |

## E5. Mock fixtures (main §8; `src/ipc/mock/handlers/depTracker.ts` + `src/ipc/fixtures/depTracker.ts`)

> **P125h amendments:** transitive rows carry `scope: 'normal'` only (the transitive+dev row is
> dropped — Rust can't emit it); `RowSpec.vulnsChecked` is replaced by `vulnsUnverified`; new modes
> `?deps=unverified` and `?deps=netprofile` (`P125-followups.md` §6–§7).

Modes are `?deps=off|loading|populated|uptodate|stale|offline|error|empty|mirror|large` `[UI-c]`.
With no `?deps`, the mode follows mock settings `depTracker.enabled`. There is one fixture export per
mode:

| Export | Content |
|---|---|
| `DEP_VIEW_OFF` | tracker disabled |
| `DEP_VIEW_LOADING` | `running` set, `report: null` |
| `DEP_VIEW_POPULATED` | the groups and rows listed below |
| `DEP_VIEW_UPTODATE` | report with nothing to update |
| `DEP_VIEW_STALE` | `manifestsChanged: true` |
| `DEP_VIEW_OFFLINE` | `lastError.kind: 'offline'` + a 3 h-old report |
| `DEP_VIEW_ERROR` | `service` error, no report |
| `DEP_VIEW_EMPTY` | `groups: []` |
| `DEP_VIEW_MIRROR` `[UI-c]` | every row in two groups `private`, `reason: customRegistry`, `registryHost: 'npm.corp.example'` |
| `DEP_VIEW_LARGE` `[UI-c]` | one group of 1,240 rows |

`DEP_VIEW_POPULATED` contains:
- **groups:** `package.json` + `pnpm-lock.yaml`, `src-tauri/Cargo.toml` + `Cargo.lock`,
  `tools/requirements.txt` (`noLockfile`), `android/build.gradle` (`notTracked`), and `broken/go.mod`
  (`parseError`, `category: invalidSyntax`, `line: 3`, `col: 1`);
- **rows:** patch, minor and major updates; a vulnerable direct row (critical); a vulnerable
  transitive row (high); a `local` path dep; a `private` scoped dep with `registryHost`; an `unknown`
  row **with one advisory** `[UI-e]`;
- **long-content rows** `[UI-c]`: a 70-char scoped npm name; a Go pseudo-version
  `v0.0.0-20240101123456-abcdef123456`; a row with 4 advisories (one `unknown` severity with
  `summary: null`, one with 3 aliases); a `dev` + `transitive` row; the manifest path
  `packages/very/deeply/nested/workspace/member/package.json`.

Fixture summaries come from `summarizeFixture()` (the main §2.1 rules), and each group has a
plausible `transitiveChecked`. Mock `depsCheckNow` emits started → 3×progress → finished. Mock
disable emits `cleared`.

## E6. TypeScript mirror (`src/ipc/types/depTracker.ts`) (main §2.3) — exact mirror of main §2.1/§2.2

> **P125h additive delta:** `DepPrivateReason` gains `'localFeed'`; `unknown` availability gains
> `reason: 'notFound' | 'tooLarge'`; `DepRow.vulnsUnverified: DepVulnsUnverifiedReason | null`;
> `DepReport.networkProfile: boolean` — see `P125-followups.md` §0 (authoritative) and the code.

```ts
export type DepEcosystem = 'npm' | 'cargo' | 'pypi' | 'go' | 'nuget';
export type DepUpdateKind = 'patch' | 'minor' | 'major';
export type DepSeverity = 'critical' | 'high' | 'medium' | 'low' | 'unknown';
export type DepRelation = 'direct' | 'transitive';
export type DepScope = 'normal' | 'dev' | 'build' | 'optional' | 'peer';
export type DepLocalReason = 'path' | 'git' | 'workspace' | 'url';
export type DepPrivateReason = 'customRegistry' | 'scopedRegistry' | 'goPrivate' | 'sourceMapping' | 'configUnreadable';
export type DepAvailability =
  | { kind: 'checked' } | { kind: 'unknown' } /* may still carry vulns */
  | { kind: 'local'; reason: DepLocalReason }
  | { kind: 'private'; reason: DepPrivateReason; registryHost: string | null };
export interface DepAdvisory { id: string; aliases: string[]; summary: string | null /* plain text */;
  severity: DepSeverity; cvssScore: number | null; modified: string; url: string }
export interface DepRow { name: string; ecosystem: DepEcosystem; relation: DepRelation; scope: DepScope;
  declared: string | null; current: string | null; latest: string | null; update: DepUpdateKind | null;
  availability: DepAvailability; vulnsChecked: boolean; vulns: DepAdvisory[]; registryUrl: string | null }
export type DepParseErrorKind = 'tooLarge' | 'notAFile' | 'unreadable' | 'invalidSyntax'
  | 'unsupportedFeature' | 'unsupportedVersion' | 'limitExceeded' | 'internal';
export type DepGroupState = { kind: 'ok' } | { kind: 'noLockfile' }
  | { kind: 'parseError'; file: string; category: DepParseErrorKind; line: number | null; col: number | null; message: string }
  | { kind: 'notTracked'; label: string };
export interface DepManifestGroup { manifestPath: string; lockfilePath: string | null;
  ecosystem: DepEcosystem | null; state: DepGroupState; rows: DepRow[]; transitiveChecked: number }
export interface DepSeverityCounts { critical: number; high: number; medium: number; low: number; unknown: number }
export interface DepSummary { major: number; minor: number; patch: number; vulnerable: number;
  vulnerableBySeverity: DepSeverityCounts; upToDate: number; unknown: number; local: number;
  private: number; notTracked: number; parseErrors: number }
export interface DepReport { repoId: string; checkedAtMs: number; groups: DepManifestGroup[]; summary: DepSummary }
export type DepCheckFailureKind = 'offline' | 'rateLimited' | 'service' | 'timeout' | 'internal';
export interface DepCheckFailure { kind: DepCheckFailureKind; httpStatus: number | null; message: string; atMs: number }
/** done/total = deps.dev lookups + OSV batches — NOT packages [UI-b]. */
export interface DepRunningCheck { checkId: number; startedAtMs: number; done: number; total: number }
export interface DepTrackerView { repoId: string; enabled: boolean; report: DepReport | null;
  manifestsChanged: boolean; running: DepRunningCheck | null; lastError: DepCheckFailure | null;
  nextCheckMs: number | null }
export type DepsChangedKind = 'started' | 'progress' | 'finished' | 'failed' | 'cancelled' | 'stale' | 'cleared';
export interface DepsChangedPayload { repoId: string; checkId: number | null; kind: DepsChangedKind;
  done: number | null; total: number | null }
export interface DepTrackerSettings { enabled: boolean; intervalHours: number } // UiSettings.depTracker
```
`check_id` is a per-process `u64` counter starting at 1.
