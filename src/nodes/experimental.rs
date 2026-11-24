//! 実験ノードの明示的な登録。既存kindの意味は変更しない。
use crate::node::{CoreError, Registry};
pub const NODE_COUNT: usize = 35;
pub fn register(registry: &mut Registry) -> Result<(), CoreError> {
    registry.register(super::text_case::TextCase)?;
    registry.register(super::text_trim::TextTrim)?;
    registry.register(super::whitespace_collapse::WhitespaceCollapse)?;
    registry.register(super::line_endings::LineEndings)?;
    registry.register(super::text_extract::TextExtract)?;
    registry.register(super::text_prefix::TextPrefix)?;
    registry.register(super::text_suffix::TextSuffix)?;
    registry.register(super::character_filter::CharacterFilter)?;
    registry.register(super::json_escape::JsonEscape)?;
    registry.register(super::hiragana_katakana::HiraganaKatakana)?;
    registry.register(super::katakana_hiragana::KatakanaHiragana)?;
    registry.register(super::text_replace::TextReplace)?;
    registry.register(super::text_split::TextSplit)?;
    registry.register(super::choice_deduplicate::ChoiceDeduplicate)?;
    registry.register(super::choice_top_n::ChoiceTopN)?;
    registry.register(super::choice_sort::ChoiceSort)?;
    registry.register(super::choice_limit::ChoiceLimit)?;
    registry.register(super::choice_threshold::ChoiceThreshold)?;
    registry.register(super::choice_remove::ChoiceRemove)?;
    registry.register(super::score_set::ScoreSet)?;
    registry.register(super::score_add::ScoreAdd)?;
    registry.register(super::score_multiply::ScoreMultiply)?;
    registry.register(super::score_normalize::ScoreNormalize)?;
    registry.register(super::choice_extract::ChoiceExtract)?;
    registry.register(super::sequence_extract::SequenceExtract)?;
    registry.register(super::packet_passthrough::PacketPassthrough)?;
    registry.register(super::packet_depth_limit::PacketDepthLimit)?;
    registry.register(super::packet_rebuild::PacketRebuild)?;
    registry.register(super::text_concat::TextConcat)?;
    registry.register(super::choice_build::ChoiceBuild)?;
    registry.register(super::choice_merge::ChoiceMerge)?;
    registry.register(super::sequence_build::SequenceBuild)?;
    registry.register(super::sentence_split::SentenceSplit)?;
    registry.register(super::ascii_width::AsciiWidth)?;
    registry.register(super::japanese_punctuation::JapanesePunctuation)?;
    Ok(())
}
