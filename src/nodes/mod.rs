//! 実用ノード用の配置場所。以下の4種を実装し、標準Registryへ登録済み。
pub mod candidate_selection;
pub mod romaji;
pub mod stringify;

/// 完全一致辞書Nodeと純粋API。形式はこのNode固有。
pub mod dictionary_exact;

pub mod character_filter;
pub mod choice_deduplicate;
pub mod choice_extract;
pub mod choice_limit;
pub mod choice_remove;
pub mod choice_sort;
pub mod choice_threshold;
pub mod choice_top_n;
/// ユーザーが後で取捨選択する、検査済みの実験ノード群。
pub mod experimental;
pub mod hiragana_katakana;
pub mod json_escape;
pub mod katakana_hiragana;
pub mod line_endings;
pub mod packet_depth_limit;
pub mod packet_passthrough;
pub mod packet_rebuild;
pub mod score_add;
pub mod score_multiply;
pub mod score_normalize;
pub mod score_set;
pub mod sequence_extract;
pub mod text_case;
pub mod text_extract;
pub mod text_prefix;
pub mod text_replace;
pub mod text_split;
pub mod text_suffix;
pub mod text_trim;
pub mod whitespace_collapse;

pub mod ascii_width;
pub mod choice_build;
pub mod choice_merge;
pub mod japanese_punctuation;
pub mod sentence_split;
pub mod sequence_build;
pub mod text_concat;

/// Unregistered prototypes; intentionally excluded from register_nodes.
pub mod paragraph_split;
pub mod recovery;
