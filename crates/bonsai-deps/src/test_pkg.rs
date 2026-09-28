//! `cfg(test)` only: a `PublicPackage` through the real `classify`, for the
//! `remote/` unit tests (which may not name adapter types, §7.3 item 4).

use crate::adapter::{DepSource, RegistryRef};
use crate::model::Ecosystem;
use crate::privacy::{classify, LocalNames, PublicPackage, Visibility};
use crate::registry_ctx::RegistryContext;

pub(crate) fn public(eco: Ecosystem, name: &str, version: Option<&str>) -> PublicPackage {
    let reg = RegistryContext::no_signals();
    let src = DepSource::Registry(RegistryRef::Default);
    match classify(eco, name, version, &src, &reg, &LocalNames::new()) {
        Visibility::Public(p) => p,
        other => panic!("{name}: {other:?}"),
    }
}
