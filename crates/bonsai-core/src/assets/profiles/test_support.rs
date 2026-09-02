//! Shared profile builders for the `profiles` test modules.

use super::{ContextProfile, ProfileTarget};

pub(super) fn target(asset_id: &str, content: &str) -> ProfileTarget {
    ProfileTarget {
        asset_id: asset_id.to_string(),
        content: content.to_string(),
    }
}

pub(super) fn profile(name: &str, targets: Vec<ProfileTarget>) -> ContextProfile {
    ContextProfile {
        name: name.to_string(),
        description: None,
        model: None,
        targets,
    }
}
