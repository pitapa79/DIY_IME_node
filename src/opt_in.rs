//! Unadopted processing nodes available in opt-in builds.
use crate::node::{CoreError, Registry};

#[path = "nodes/dictionary_prefix/mod.rs"]
pub mod dictionary_prefix;
#[path = "nodes/text_compare/mod.rs"]
pub mod text_compare;
#[path = "nodes/text_mapping/mod.rs"]
pub mod text_mapping;
#[path = "nodes/text_regex_replace/mod.rs"]
pub mod text_regex_replace;
#[path = "nodes/text_regex_search/mod.rs"]
pub mod text_regex_search;
#[path = "nodes/text_unicode_normalize/mod.rs"]
pub mod text_unicode_normalize;

/// Register the opt-in Nodes in a development registry.
pub fn register(registry: &mut Registry) -> Result<(), CoreError> {
    registry.register(text_unicode_normalize::TextUnicodeNormalize)?;
    registry.register(text_compare::TextCompare)?;
    registry.register(dictionary_prefix::DictionaryPrefix)?;
    registry.register(text_regex_replace::TextRegexReplace)?;
    registry.register(text_regex_search::TextRegexSearch)?;
    registry.register(text_mapping::TextMapping)
}
