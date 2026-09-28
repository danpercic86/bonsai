//! PyPI + NuGet sentinel fixtures (E4 rows 6 / 6′, P125c2).

/// Its own `--index-url` routes `zz-sentinel-idx` (E4 row 6).
pub const REQ_ROOT: &str = "--index-url https://u:zz-sentinel-token@pypi.corp.invalid/simple\n\
zz-sentinel-idx==1.0\n";

pub const REQ_DEV: &str = "six==1.16.0\n\
-e git+https://git.corp.invalid/zz.git#egg=zz_sentinel_pyvcs\n";

pub const PYPROJECT: &str = r#"[project]
name = "zz-sentinel-pyapp"
dependencies = ["httpx>=0.27", "zz-sentinel-uvpin"]

[tool.uv.sources]
zz-sentinel-uvpin = { index = "corp" }

[[tool.uv.index]]
name = "corp"
url = "https://u:zz-sentinel-token@uv.corp.invalid/simple"
explicit = true
"#;

pub const UV_LOCK: &str = r#"version = 1

[[package]]
name = "httpx"
version = "0.27.2"
source = { registry = "https://pypi.org/simple" }

[[package]]
name = "zz-sentinel-pyapp"
version = "0.1.0"
source = { editable = "." }

[[package]]
name = "zz-sentinel-uvpin"
version = "2.0.0"
source = { registry = "https://uv.corp.invalid/simple" }

[[package]]
name = "zz-sentinel-uvtrans"
version = "1.0.0"
source = { registry = "https://uv.corp.invalid/simple" }
"#;

/// `ZzSentinel.Above` is PUBLIC unless the `[Q7]` parent config applies.
pub const CSPROJ: &str = r#"<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="13.0.3" />
    <PackageReference Include="ZzSentinel.Above" Version="1.0.0" />
    <PackageReference Include="ZzSentinel.Proj" Version="1.0.0" />
  </ItemGroup>
</Project>
"#;

/// A sibling project: its file stem declares `ZzSentinel.Proj`.
pub const CSPROJ_LIB: &str = "<Project Sdk=\"Microsoft.NET.Sdk\"/>\n";

/// A tracked, mapped config: `ZzSentinel.*` comes from the corp feed.
pub const NUGET_MAPPED: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <clear />
    <add key="nuget.org" value="https://api.nuget.org/v3/index.json" />
    <add key="corp" value="https://u:zz-sentinel-token@nuget.corp.invalid/v3/index.json" />
  </packageSources>
  <packageSourceMapping>
    <packageSource key="nuget.org"><package pattern="*" /></packageSource>
    <packageSource key="corp"><package pattern="ZzSentinel.*" /></packageSource>
  </packageSourceMapping>
  <packageSourceCredentials>
    <corp>
      <add key="Username" value="u" />
      <add key="ClearTextPassword" value="zz-sentinel-token" />
    </corp>
  </packageSourceCredentials>
</configuration>
"#;

/// `[Q7]` E4 row 6 (f): a parent-dir `NuGet.Config` adding the corp feed.
pub const Q7_NUGET_CONFIG: &str = r#"<configuration>
  <packageSources>
    <add key="corp" value="https://nuget.corp.invalid/v3/index.json" />
  </packageSources>
  <packageSourceCredentials>
    <corp><add key="ClearTextPassword" value="zz-sentinel-token" /></corp>
  </packageSourceCredentials>
</configuration>
"#;

/// E4 row 6 (d): not XML.
pub const NUGET_BROKEN: &str = "<configuration><packageSources><add key=\"corp\" value=\"https://nuget.corp.invalid/\"></configuration>\n";
