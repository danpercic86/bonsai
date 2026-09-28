//! NuGet project / props / lock / config fixtures (E2 + E3 NuGet rows).

/// An SDK-style project: attribute and child-element versions, a floating
/// range, an MSBuild property, an `Update=` item, a project reference.
pub(crate) const CSPROJ: &str = "\u{FEFF}<Project Sdk=\"Microsoft.NET.Sdk\">\r\n\
  <PropertyGroup>\r\n\
    <TargetFramework>net8.0</TargetFramework>\r\n\
    <PackageId>ZzSentinel.App</PackageId>\r\n\
  </PropertyGroup>\r\n\
  <ItemGroup>\r\n\
    <PackageReference Include=\"Newtonsoft.Json\" Version=\"13.0.3\" />\r\n\
    <PackageReference Include=\"Serilog\">\r\n\
      <Version>[3.1,4.0)</Version>\r\n\
    </PackageReference>\r\n\
    <packagereference include=\"Polly\" version=\"8.*\" />\r\n\
    <PackageReference Include=\"ZzSentinel.Pkg\" Version=\"$(CorpVersion)\" />\r\n\
    <PackageReference Update=\"Newtonsoft.Json\" PrivateAssets=\"all\" />\r\n\
    <PackageReference Include=\"xunit;xunit.runner.visualstudio\" VersionOverride=\"2.9.0\" Version=\"2.0.0\" />\r\n\
    <ProjectReference Include=\"..\\Lib\\ZzSentinel.Lib.csproj\" />\r\n\
    <PackageDownload Include=\"Some.Tool\" Version=\"[1.0.0]\" />\r\n\
  </ItemGroup>\r\n\
</Project>\r\n";

pub(crate) const PACKAGES_PROPS: &str = r#"<Project>
  <PropertyGroup><ManagePackageVersionsCentrally>true</ManagePackageVersionsCentrally></PropertyGroup>
  <ItemGroup>
    <PackageVersion Include="Newtonsoft.Json" Version="13.0.3" />
    <GlobalPackageReference Include="StyleCop.Analyzers" Version="1.1.118" />
  </ItemGroup>
</Project>"#;

pub(crate) const PACKAGES_CONFIG: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<packages>
  <package id="jQuery" version="3.7.1" targetFramework="net48" />
  <package id="NUnit" version="3.14.0" developmentDependency="true" />
</packages>"#;

pub(crate) const LOCK: &str = r#"{
  "version": 1,
  "dependencies": {
    "net8.0": {
      "Newtonsoft.Json": { "type": "Direct", "requested": "[13.0.3, )", "resolved": "13.0.3" },
      "Serilog": { "type": "Direct", "requested": "[3.1, 4.0)", "resolved": "3.1.1" },
      "ZzSentinel.Trans": { "type": "Transitive", "resolved": "0.9.0" },
      "ZzSentinel.Lib": { "type": "Project" }
    },
    "net48": {
      "newtonsoft.json": { "type": "Direct", "resolved": "13.0.3" }
    }
  }
}"#;

/// A repo-level config: `<clear/>`, a private feed, a disabled feed, a
/// source mapping, and credentials that must never be read.
pub(crate) const CONFIG_MAPPED: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <clear />
    <add key="nuget.org" value="https://api.nuget.org/v3/index.json" protocolVersion="3" />
    <add key="corp" value="https://u:zz-sentinel-token@nuget.corp.invalid/v3/index.json" />
    <add key="old" value="https://old.corp.invalid/v3/index.json" />
  </packageSources>
  <disabledPackageSources>
    <add key="old" value="true" />
  </disabledPackageSources>
  <packageSourceMapping>
    <packageSource key="nuget.org">
      <package pattern="*" />
    </packageSource>
    <packageSource key="corp">
      <package pattern="ZzSentinel.*" />
    </packageSource>
  </packageSourceMapping>
  <packageSourceCredentials>
    <corp>
      <add key="Username" value="u" />
      <add key="ClearTextPassword" value="zz-sentinel-token" />
    </corp>
  </packageSourceCredentials>
  <apikeys>
    <add key="https://nuget.corp.invalid/" value="zz-sentinel-token" />
  </apikeys>
</configuration>"#;

/// A private feed with no mapping.
pub(crate) const CONFIG_PRIVATE: &str = r#"<configuration>
  <packageSources>
    <add key="corp" value="https://nuget.corp.invalid/v3/index.json" />
  </packageSources>
</configuration>"#;

/// `<clear/>` + nuget.org only: farther (user / machine) feeds drop out.
pub(crate) const CONFIG_CLEAR_PUBLIC: &str = r#"<configuration>
  <packageSources>
    <clear />
    <add key="nuget.org" value="https://api.nuget.org/v3/index.json" />
  </packageSources>
</configuration>"#;

/// Not XML.
pub(crate) const CONFIG_BROKEN: &str =
    "<configuration><packageSources><add key=\"x\" value=\"zz-sentinel-token\"></configuration>";
