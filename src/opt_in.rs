//! Unadopted processing nodes available in opt-in builds.
use crate::node::{CoreError, Registry};

#[path = "nodes/dictionary_prefix/mod.rs"]
pub mod dictionary_prefix;
#[path = "nodes/dictionary_reverse/mod.rs"]
pub mod dictionary_reverse;
#[path = "nodes/text_compare/mod.rs"]
pub mod text_compare;
#[path = "nodes/text_date_format/mod.rs"]
pub mod text_date_format;
#[path = "nodes/text_hiragana_to_romaji/mod.rs"]
pub mod text_hiragana_to_romaji;
#[path = "nodes/text_kana_width/mod.rs"]
pub mod text_kana_width;
#[path = "nodes/text_length/mod.rs"]
pub mod text_length;
#[path = "nodes/text_mapping/mod.rs"]
pub mod text_mapping;
#[path = "nodes/text_number_notation/mod.rs"]
pub mod text_number_notation;
#[path = "nodes/text_regex_replace/mod.rs"]
pub mod text_regex_replace;
#[path = "nodes/text_regex_search/mod.rs"]
pub mod text_regex_search;
#[path = "nodes/text_time_format/mod.rs"]
pub mod text_time_format;
#[path = "nodes/text_unicode_normalize/mod.rs"]
pub mod text_unicode_normalize;
#[path = "nodes/text_unit_notation/mod.rs"]
pub mod text_unit_notation;

/// Register the opt-in Nodes in a development registry.
pub fn register(registry: &mut Registry) -> Result<(), CoreError> {
    registry.register(text_unicode_normalize::TextUnicodeNormalize)?;
    registry.register(text_compare::TextCompare)?;
    registry.register(dictionary_prefix::DictionaryPrefix)?;
    registry.register(text_regex_replace::TextRegexReplace)?;
    registry.register(text_regex_search::TextRegexSearch)?;
    registry.register(text_mapping::TextMapping)?;
    registry.register(text_kana_width::TextKanaWidth)?;
    registry.register(dictionary_reverse::DictionaryReverse)?;
    registry.register(text_hiragana_to_romaji::TextHiraganaToRomaji)?;
    registry.register(text_number_notation::TextNumberNotation)?;
    registry.register(text_date_format::TextDateFormat)?;
    registry.register(text_time_format::TextTimeFormat)?;
    registry.register(text_unit_notation::TextUnitNotation)?;
    registry.register(text_length::TextLength)
}
