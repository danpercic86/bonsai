//! The P125d live-fixture corpus `[LOW-4]`: one known-vulnerable pinned
//! version per ecosystem, the §3.3 yanked probe (tokio, whose 1.39.0 is
//! yanked; requests, whose 2.32.0 / 2.32.1 are yanked) and clean controls.
//! Every name is a well-known public package chosen here — nothing is
//! repo-derived. `tests/live_record.rs` (ignored) records the deps.dev / OSV
//! responses for exactly this repo into [`fixtures_dir`]; every other test
//! replays them offline through `FakeTransport::with_fixtures`.

use std::path::PathBuf;

use super::fixture::FixtureRepo;

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("remote")
}

pub const PACKAGE_JSON: &str = r#"{
  "name": "corpus-web",
  "dependencies": { "lodash": "^4.17.20" }
}"#;

/// lodash 4.17.20: GHSA-35jh-r3h4-6jhm (command injection) et al.
/// is-number 7.0.0: a clean transitive entry (never a row).
pub const PNPM_LOCK: &str = "lockfileVersion: '9.0'

importers:

  .:
    dependencies:
      lodash:
        specifier: ^4.17.20
        version: 4.17.20

packages:

  lodash@4.17.20:
    resolution: {integrity: sha512-a}

  is-number@7.0.0:
    resolution: {integrity: sha512-b}
";

pub const CARGO_TOML: &str = r#"[package]
name = "corpus-app"
version = "0.1.0"

[dependencies]
serde = "1"
tokio = "1.38"
"#;

/// smallvec 1.6.0: RUSTSEC-2021-0003 (a vulnerable TRANSITIVE entry);
/// itoa 1.0.11: a clean transitive entry.
pub const CARGO_LOCK: &str = r#"version = 4

[[package]]
name = "corpus-app"
version = "0.1.0"
dependencies = ["serde", "tokio"]

[[package]]
name = "serde"
version = "1.0.200"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "tokio"
version = "1.38.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["smallvec", "itoa"]

[[package]]
name = "smallvec"
version = "1.6.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "itoa"
version = "1.0.11"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

/// requests 2.19.1: PYSEC-2018-28 / GHSA-x84v-xcm2-53pg.
pub const REQUIREMENTS: &str = "requests==2.19.1\n";

/// gorilla/websocket v1.4.0: GO-2020-0019 (closes the Go `v`-prefix row).
pub const GO_MOD: &str =
    "module example.com/corpus\n\ngo 1.22\n\nrequire github.com/gorilla/websocket v1.4.0\n";

/// Newtonsoft.Json 12.0.3: GHSA-5crp-9r3c-p9vr.
pub const CSPROJ: &str = r#"<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="12.0.3" />
  </ItemGroup>
</Project>
"#;

pub fn corpus_repo() -> FixtureRepo {
    let r = FixtureRepo::new();
    r.track("web/package.json", PACKAGE_JSON)
        .track("web/pnpm-lock.yaml", PNPM_LOCK)
        .track("Cargo.toml", CARGO_TOML)
        .track("Cargo.lock", CARGO_LOCK)
        .track("py/requirements.txt", REQUIREMENTS)
        .track("go/go.mod", GO_MOD)
        .track("dotnet/App/App.csproj", CSPROJ);
    r
}
