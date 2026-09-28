//! Adapter surface (contract §4.2). P125a carries only the two enums that
//! `privacy::classify` takes; `EcosystemAdapter`, `ParsedGroup`, the dep and
//! lock-entry structs and `ADAPTERS` land with the adapters in P125b.

/// Where a dependency (or lock entry) resolves from, as the adapter read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepSource {
    Registry(RegistryRef),
    Path,
    Git,
    Workspace,
    Url,
}

/// Which registry a `DepSource::Registry` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryRef {
    /// The ecosystem's default registry (subject to config overrides).
    Default,
    /// A named registry, e.g. Cargo `registry = "corp"`.
    Named(String),
    /// A lockfile host. `extract_host` output only (lowercase host, no
    /// userinfo/port/path).
    Host(String),
    /// An npm scope (`@corp`) the adapter saw routed to a scope registry.
    Scope(String),
}
