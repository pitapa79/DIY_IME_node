//! 未採用の実験ノード。契約はexamples/nodes/text_jis_kana/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-jis-kana";
#[derive(Debug, Clone, Copy, Default)]
pub struct TextJisKana;
fn settings(config: &Value) -> Result<(), CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if !object.is_empty() {
        return Err(CoreError::new(
            "invalid-config",
            "this node declares no settings",
        ));
    }
    Ok(())
}
/// JIS X 6002 のキー配列に基づくキートップ記号→かなの固定表。
/// 英字キーのシフト面は大文字、数字・記号キーのシフト面はシフト側の刻印で表す。
fn key_kana(symbol: char) -> Option<char> {
    Some(match symbol {
        '1' => 'ぬ',
        '2' => 'ふ',
        '3' => 'あ',
        '4' => 'う',
        '5' => 'え',
        '6' => 'お',
        '7' => 'や',
        '8' => 'ゆ',
        '9' => 'よ',
        '0' => 'わ',
        '-' => 'ほ',
        '^' => 'へ',
        '¥' => 'ー',
        'q' => 'た',
        'w' => 'て',
        'e' => 'い',
        'r' => 'す',
        't' => 'か',
        'y' => 'ん',
        'u' => 'な',
        'i' => 'に',
        'o' => 'ら',
        'p' => 'せ',
        '@' => '゛',
        '[' => '゜',
        'a' => 'ち',
        's' => 'と',
        'd' => 'し',
        'f' => 'は',
        'g' => 'き',
        'h' => 'く',
        'j' => 'ま',
        'k' => 'の',
        'l' => 'り',
        ';' => 'れ',
        ':' => 'け',
        ']' => 'む',
        'z' => 'つ',
        'x' => 'さ',
        'c' => 'そ',
        'v' => 'ひ',
        'b' => 'こ',
        'n' => 'み',
        'm' => 'も',
        ',' => 'ね',
        '.' => 'る',
        '/' => 'め',
        '\\' => 'ろ',
        '#' => 'ぁ',
        '$' => 'ぅ',
        '%' => 'ぇ',
        '&' => 'ぉ',
        '\'' => 'ゃ',
        '(' => 'ゅ',
        ')' => 'ょ',
        'E' => 'ぃ',
        'Z' => 'っ',
        '<' => '、',
        '>' => '。',
        '?' => '・',
        'を' => 'を',
        _ => return None,
    })
}
fn with_dakuten(kana: char) -> Option<char> {
    Some(match kana {
        'か' => 'が',
        'き' => 'ぎ',
        'く' => 'ぐ',
        'け' => 'げ',
        'こ' => 'ご',
        'さ' => 'ざ',
        'し' => 'じ',
        'す' => 'ず',
        'せ' => 'ぜ',
        'そ' => 'ぞ',
        'た' => 'だ',
        'ち' => 'ぢ',
        'つ' => 'づ',
        'て' => 'で',
        'と' => 'ど',
        'は' => 'ば',
        'ひ' => 'び',
        'ふ' => 'ぶ',
        'へ' => 'べ',
        'ほ' => 'ぼ',
        'う' => 'ゔ',
        _ => return None,
    })
}
fn with_handakuten(kana: char) -> Option<char> {
    Some(match kana {
        'は' => 'ぱ',
        'ひ' => 'ぴ',
        'ふ' => 'ぷ',
        'へ' => 'ぺ',
        'ほ' => 'ぽ',
        _ => return None,
    })
}
fn convert(text: &str) -> Result<String, CoreError> {
    let mut out: Vec<char> = Vec::new();
    for symbol in text.chars() {
        // 濁点・半濁点そのもの（U+309B/U+309C）も @ / [ キーと同じ合成記号として扱う。
        let kana = if symbol == '\u{309B}' || symbol == '\u{309C}' {
            symbol
        } else {
            key_kana(symbol)
                .ok_or_else(|| CoreError::new("unknown-key", "symbol is not a JIS key top"))?
        };
        if kana == '\u{309B}' || kana == '\u{309C}' {
            let previous = out
                .pop()
                .ok_or_else(|| CoreError::new("invalid-input", "dakuten has no preceding kana"))?;
            let composed = if kana == '\u{309B}' {
                with_dakuten(previous)
            } else {
                with_handakuten(previous)
            }
            .ok_or_else(|| {
                CoreError::new("invalid-input", "preceding kana cannot take the mark")
            })?;
            out.push(composed);
        } else {
            out.push(kana);
        }
    }
    Ok(out.into_iter().collect())
}
impl Node for TextJisKana {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Text)
        };
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(port("in"))],
            vec![port("out")],
        );
        definition.default_config = json!({});
        definition.config_schema = json!({"type":"object","additionalProperties":false});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        settings(config)
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        settings(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: convert(text)?,
                    meta: None,
                })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
