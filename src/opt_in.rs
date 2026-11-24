//! Unadopted processing nodes available in opt-in builds.
use crate::node::{CoreError, Registry};

#[path = "nodes/text_compare/mod.rs"]
pub mod text_compare;
#[path = "nodes/text_unicode_normalize/mod.rs"]
pub mod text_unicode_normalize;

/// Register the opt-in Nodes in a development registry.
pub fn register(registry: &mut Registry) -> Result<(), CoreError> {
    registry.register(text_unicode_normalize::TextUnicodeNormalize)?;
    registry.register(text_compare::TextCompare)
}
