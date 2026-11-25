//! 未採用の実験ノード。契約はexamples/nodes/text_hiragana_to_romaji/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-hiragana-to-romaji";
/// 1文字のひらがな音節とヘボン式ローマ字。促音（っ）と撥音（ん）は制御規則のため含まない。
/// 小書きの ゃゅょ は拗音にならない位置での単独表記として含む。
const SINGLE: &[(&str, &str)] = &[
    ("あ", "a"),
    ("い", "i"),
    ("う", "u"),
    ("え", "e"),
    ("お", "o"),
    ("か", "ka"),
    ("き", "ki"),
    ("く", "ku"),
    ("け", "ke"),
    ("こ", "ko"),
    ("さ", "sa"),
    ("し", "shi"),
    ("す", "su"),
    ("せ", "se"),
    ("そ", "so"),
    ("た", "ta"),
    ("ち", "chi"),
    ("つ", "tsu"),
    ("て", "te"),
    ("と", "to"),
    ("な", "na"),
    ("に", "ni"),
    ("ぬ", "nu"),
    ("ね", "ne"),
    ("の", "no"),
    ("は", "ha"),
    ("ひ", "hi"),
    ("ふ", "fu"),
    ("へ", "he"),
    ("ほ", "ho"),
    ("ま", "ma"),
    ("み", "mi"),
    ("む", "mu"),
    ("め", "me"),
    ("も", "mo"),
    ("や", "ya"),
    ("ゆ", "yu"),
    ("よ", "yo"),
    ("ら", "ra"),
    ("り", "ri"),
    ("る", "ru"),
    ("れ", "re"),
    ("ろ", "ro"),
    ("わ", "wa"),
    ("を", "wo"),
    ("が", "ga"),
    ("ぎ", "gi"),
    ("ぐ", "gu"),
    ("げ", "ge"),
    ("ご", "go"),
    ("ざ", "za"),
    ("じ", "ji"),
    ("ず", "zu"),
    ("ぜ", "ze"),
    ("ぞ", "zo"),
    ("だ", "da"),
    ("ぢ", "ji"),
    ("づ", "zu"),
    ("で", "de"),
    ("ど", "do"),
    ("ば", "ba"),
    ("び", "bi"),
    ("ぶ", "bu"),
    ("べ", "be"),
    ("ぼ", "bo"),
    ("ぱ", "pa"),
    ("ぴ", "pi"),
    ("ぷ", "pu"),
    ("ぺ", "pe"),
    ("ぽ", "po"),
    ("ゔ", "vu"),
    ("ゐ", "wi"),
    ("ゑ", "we"),
    ("ぁ", "a"),
    ("ぃ", "i"),
    ("ぅ", "u"),
    ("ぇ", "e"),
    ("ぉ", "o"),
    ("ゎ", "wa"),
    ("ゃ", "ya"),
    ("ゅ", "yu"),
    ("ょ", "yo"),
];
/// 拗音（基底カナ+小書き ゃゅょ）とヘボン式ローマ字。
const YOON: &[(&str, &str)] = &[
    ("きゃ", "kya"),
    ("きゅ", "kyu"),
    ("きょ", "kyo"),
    ("しゃ", "sha"),
    ("しゅ", "shu"),
    ("しょ", "sho"),
    ("ちゃ", "cha"),
    ("ちゅ", "chu"),
    ("ちょ", "cho"),
    ("にゃ", "nya"),
    ("にゅ", "nyu"),
    ("にょ", "nyo"),
    ("ひゃ", "hya"),
    ("ひゅ", "hyu"),
    ("ひょ", "hyo"),
    ("みゃ", "mya"),
    ("みゅ", "myu"),
    ("みょ", "myo"),
    ("りゃ", "rya"),
    ("りゅ", "ryu"),
    ("りょ", "ryo"),
    ("ぎゃ", "gya"),
    ("ぎゅ", "gyu"),
    ("ぎょ", "gyo"),
    ("じゃ", "ja"),
    ("じゅ", "ju"),
    ("じょ", "jo"),
    ("ぢゃ", "ja"),
    ("ぢゅ", "ju"),
    ("ぢょ", "jo"),
    ("びゃ", "bya"),
    ("びゅ", "byu"),
    ("びょ", "byo"),
    ("ぴゃ", "pya"),
    ("ぴゅ", "pyu"),
    ("ぴょ", "pyo"),
];
#[derive(Debug, Clone, Copy, Default)]
pub struct TextHiraganaToRomaji;
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum System {
    Hepburn,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    system: System,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("system") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))
}
/// Position `index` から始まる音節の (ローマ字, 文字数)。促音・撥音は音節として扱わない。
fn syllable_at(chars: &[char], index: usize) -> Option<(&'static str, usize)> {
    let first = *chars.get(index)?;
    if first == 'っ' || first == 'ん' {
        return None;
    }
    if let Some(second) = chars.get(index + 1) {
        let pair: String = [first, *second].iter().collect();
        if let Some((_, romaji)) = YOON.iter().find(|(kana, _)| *kana == pair) {
            return Some((romaji, 2));
        }
    }
    let single = first.to_string();
    SINGLE
        .iter()
        .find(|(kana, _)| *kana == single)
        .map(|(_, romaji)| (*romaji, 1))
}
fn convert(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let current = chars[index];
        if current == 'っ' {
            // 促音: 後続音節の頭子音を重ねる。ch の前は t を重ねる。
            // 後続が音節でない（母音・ん・文末・非ひらがな）場合は ' とする。
            match syllable_at(&chars, index + 1) {
                Some((romaji, len)) => {
                    if let Some(stripped) = romaji.strip_prefix("ch") {
                        output.push('t');
                        output.push_str("ch");
                        output.push_str(stripped);
                    } else {
                        let head = romaji.chars().next().unwrap_or_default();
                        output.push(head);
                        output.push_str(romaji);
                    }
                    index += 1 + len;
                }
                None => {
                    output.push('\'');
                    index += 1;
                }
            }
            continue;
        }
        if current == 'ん' {
            // 撥音: 母音と y の前では n' として区切りを曖昧にしない。
            match syllable_at(&chars, index + 1) {
                Some((romaji, _)) if romaji.starts_with(['a', 'i', 'u', 'e', 'o', 'y']) => {
                    output.push_str("n'");
                }
                _ => output.push('n'),
            }
            index += 1;
            continue;
        }
        match syllable_at(&chars, index) {
            Some((romaji, len)) => {
                output.push_str(romaji);
                index += len;
            }
            None => {
                output.push(current);
                index += 1;
            }
        }
    }
    output
}
impl Node for TextHiraganaToRomaji {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(
                Port::packet("in")
                    .with_language("ja")
                    .with_root(RootConstraint::Text),
            )],
            vec![
                Port::packet("out")
                    .with_language("ja-Latn")
                    .with_root(RootConstraint::Text),
            ],
        );
        definition.default_config = json!({"system":"hepburn"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["system"],"properties":{"system":{"type":"string","enum":["hepburn"]}}});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        settings(config).map(|_| ())
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let settings = settings(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                let converted = match settings.system {
                    System::Hepburn => convert(text),
                };
                if converted == *text {
                    OutputValue::Data(Arc::clone(packet))
                } else {
                    OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                        text: converted,
                        meta: None,
                    })))
                }
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
