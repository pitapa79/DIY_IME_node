use diy_ime2_nodes::{
    compat,
    error_input::ErrorAwareNode,
    node::{Node, Registry},
    nodes,
};
use serde_json::Value;
#[test]
fn every_migrated_definition_and_registration_matches_the_original() {
    let baseline: Value =
        serde_json::from_str(include_str!("../docs/legacy-node-inventory.json")).unwrap();
    let expected: std::collections::BTreeMap<_, _> = baseline["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["classification"] != "system")
        .map(|n| (n["definition"]["kind"].as_str().unwrap().to_string(), n))
        .collect();
    let definitions = vec![
        compat::demo::Branch.definition(),
        compat::demo::Identity.definition(),
        compat::demo::SelectText.definition(),
        compat::demo::Uppercase.definition(),
        compat::initial_e2e::KanaKanjiCandidates.definition(),
        compat::initial_e2e::RomajiToHiragana.definition(),
        compat::initial_e2e::Segment.definition(),
        compat::initial_e2e::SelectCandidates.definition(),
        compat::initial_e2e::Stringify.definition(),
        nodes::ascii_width::AsciiWidth.definition(),
        nodes::character_filter::CharacterFilter.definition(),
        nodes::choice_build::ChoiceBuild.definition(),
        nodes::choice_deduplicate::ChoiceDeduplicate.definition(),
        nodes::choice_extract::ChoiceExtract.definition(),
        nodes::choice_limit::ChoiceLimit.definition(),
        nodes::choice_merge::ChoiceMerge.definition(),
        nodes::choice_remove::ChoiceRemove.definition(),
        nodes::choice_sort::ChoiceSort.definition(),
        nodes::choice_threshold::ChoiceThreshold.definition(),
        nodes::choice_top_n::ChoiceTopN.definition(),
        nodes::hiragana_katakana::HiraganaKatakana.definition(),
        nodes::japanese_punctuation::JapanesePunctuation.definition(),
        nodes::json_escape::JsonEscape.definition(),
        nodes::katakana_hiragana::KatakanaHiragana.definition(),
        nodes::line_endings::LineEndings.definition(),
        nodes::packet_depth_limit::PacketDepthLimit.definition(),
        nodes::packet_passthrough::PacketPassthrough.definition(),
        nodes::packet_rebuild::PacketRebuild.definition(),
        nodes::paragraph_split::ParagraphSplit.definition(),
        nodes::score_add::ScoreAdd.definition(),
        nodes::score_multiply::ScoreMultiply.definition(),
        nodes::score_normalize::ScoreNormalize.definition(),
        nodes::score_set::ScoreSet.definition(),
        nodes::sentence_split::SentenceSplit.definition(),
        nodes::sequence_build::SequenceBuild.definition(),
        nodes::sequence_extract::SequenceExtract.definition(),
        nodes::text_case::TextCase.definition(),
        nodes::text_concat::TextConcat.definition(),
        nodes::text_extract::TextExtract.definition(),
        nodes::text_prefix::TextPrefix.definition(),
        nodes::text_replace::TextReplace.definition(),
        nodes::text_split::TextSplit.definition(),
        nodes::text_suffix::TextSuffix.definition(),
        nodes::text_trim::TextTrim.definition(),
        nodes::whitespace_collapse::WhitespaceCollapse.definition(),
        nodes::dictionary_exact::DictionaryExact.definition(),
        nodes::recovery::RecoverOriginal.definition(),
        nodes::romaji::RomajiToHiragana.definition(),
        nodes::candidate_selection::SelectCandidates.definition(),
        nodes::stringify::Stringify.definition(),
    ];
    let actual: std::collections::BTreeMap<_, _> = definitions
        .iter()
        .map(|d| (d.kind.clone(), serde_json::to_value(d).unwrap()))
        .collect();
    assert_eq!(actual.len(), 50);
    assert_eq!(actual.len(), expected.len());
    for (kind, d) in &actual {
        assert_eq!(d, &expected[kind]["definition"], "{kind}");
    }
    let mut registry = Registry::new();
    diy_ime2_nodes::register_nodes(&mut registry).unwrap();
    let registered: std::collections::BTreeSet<_> =
        registry.definitions().map(|d| d.kind.clone()).collect();
    let expected_registered: std::collections::BTreeSet<_> = expected
        .iter()
        .filter(|(_, n)| n["registered"] == true)
        .map(|(k, _)| k.clone())
        .collect();
    assert_eq!(registered.len(), 48);
    assert_eq!(registered, expected_registered);
    for n in baseline["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["classification"] == "system")
    {
        assert!(!actual.contains_key(n["definition"]["kind"].as_str().unwrap()));
    }
}
