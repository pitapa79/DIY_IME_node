//! 未採用の実験ノード。契約はexamples/nodes/text_kana_width/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-kana-width";
const HALFWIDTH_DAKUTEN: char = '\u{ff9e}';
const HALFWIDTH_HANDAKUTEN: char = '\u{ff9f}';
const COMBINING_DAKUTEN: char = '\u{3099}';
const COMBINING_HANDAKUTEN: char = '\u{309a}';
/// 半角（JIS X 0201）から全角への1文字対応。濁点・半濁点マークは含まない。
const HALF_TO_FULL: &[(char, char)] = &[
    ('｡', '。'),
    ('｢', '「'),
    ('｣', '」'),
    ('､', '、'),
    ('･', '・'),
    ('ｦ', 'ヲ'),
    ('ｧ', 'ァ'),
    ('ｨ', 'ィ'),
    ('ｩ', 'ゥ'),
    ('ｪ', 'ェ'),
    ('ｫ', 'ォ'),
    ('ｬ', 'ャ'),
    ('ｭ', 'ュ'),
    ('ｮ', 'ョ'),
    ('ｯ', 'ッ'),
    ('ｰ', 'ー'),
    ('ｱ', 'ア'),
    ('ｲ', 'イ'),
    ('ｳ', 'ウ'),
    ('ｴ', 'エ'),
    ('ｵ', 'オ'),
    ('ｶ', 'カ'),
    ('ｷ', 'キ'),
    ('ｸ', 'ク'),
    ('ｹ', 'ケ'),
    ('ｺ', 'コ'),
    ('ｻ', 'サ'),
    ('ｼ', 'シ'),
    ('ｽ', 'ス'),
    ('ｾ', 'セ'),
    ('ｿ', 'ソ'),
    ('ﾀ', 'タ'),
    ('ﾁ', 'チ'),
    ('ﾂ', 'ツ'),
    ('ﾃ', 'テ'),
    ('ﾄ', 'ト'),
    ('ﾅ', 'ナ'),
    ('ﾆ', 'ニ'),
    ('ﾇ', 'ヌ'),
    ('ﾈ', 'ネ'),
    ('ﾉ', 'ノ'),
    ('ﾊ', 'ハ'),
    ('ﾋ', 'ヒ'),
    ('ﾌ', 'フ'),
    ('ﾍ', 'ヘ'),
    ('ﾎ', 'ホ'),
    ('ﾏ', 'マ'),
    ('ﾐ', 'ミ'),
    ('ﾑ', 'ム'),
    ('ﾒ', 'メ'),
    ('ﾓ', 'モ'),
    ('ﾔ', 'ヤ'),
    ('ﾕ', 'ユ'),
    ('ﾖ', 'ヨ'),
    ('ﾗ', 'ラ'),
    ('ﾘ', 'リ'),
    ('ﾙ', 'ル'),
    ('ﾚ', 'レ'),
    ('ﾛ', 'ロ'),
    ('ﾜ', 'ワ'),
    ('ﾝ', 'ン'),
];
/// 全角の基底カナと濁点合成形。
const DAKUTEN: &[(char, char)] = &[
    ('ウ', 'ヴ'),
    ('カ', 'ガ'),
    ('キ', 'ギ'),
    ('ク', 'グ'),
    ('ケ', 'ゲ'),
    ('コ', 'ゴ'),
    ('サ', 'ザ'),
    ('シ', 'ジ'),
    ('ス', 'ズ'),
    ('セ', 'ゼ'),
    ('ソ', 'ゾ'),
    ('タ', 'ダ'),
    ('チ', 'ヂ'),
    ('ツ', 'ヅ'),
    ('テ', 'デ'),
    ('ト', 'ド'),
    ('ハ', 'バ'),
    ('ヒ', 'ビ'),
    ('フ', 'ブ'),
    ('ヘ', 'ベ'),
    ('ホ', 'ボ'),
    ('ワ', 'ヷ'),
    ('ヲ', 'ヺ'),
];
/// 全角の基底カナと半濁点合成形。
const HANDAKUTEN: &[(char, char)] = &[
    ('ハ', 'パ'),
    ('ヒ', 'ピ'),
    ('フ', 'プ'),
    ('ヘ', 'ペ'),
    ('ホ', 'ポ'),
];
#[derive(Debug, Clone, Copy, Default)]
pub struct TextKanaWidth;
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Fullwidth,
    Halfwidth,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    mode: Mode,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("mode") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))
}
fn half_to_full(c: char) -> Option<char> {
    HALF_TO_FULL.iter().find(|(h, _)| *h == c).map(|(_, f)| *f)
}
fn full_to_half(c: char) -> Option<char> {
    HALF_TO_FULL.iter().find(|(_, f)| *f == c).map(|(h, _)| *h)
}
fn compose(base: char, table: &[(char, char)]) -> Option<char> {
    table.iter().find(|(b, _)| *b == base).map(|(_, c)| *c)
}
fn decompose(c: char) -> Option<(char, char)> {
    if let Some((base, _)) = DAKUTEN.iter().find(|(_, composed)| *composed == c) {
        return Some((*base, HALFWIDTH_DAKUTEN));
    }
    if let Some((base, _)) = HANDAKUTEN.iter().find(|(_, composed)| *composed == c) {
        return Some((*base, HALFWIDTH_HANDAKUTEN));
    }
    None
}
fn to_fullwidth(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == HALFWIDTH_DAKUTEN {
            output.push(COMBINING_DAKUTEN);
            continue;
        }
        if c == HALFWIDTH_HANDAKUTEN {
            output.push(COMBINING_HANDAKUTEN);
            continue;
        }
        let Some(full) = half_to_full(c) else {
            output.push(c);
            continue;
        };
        match chars.peek() {
            Some(&HALFWIDTH_DAKUTEN) => {
                if let Some(composed) = compose(full, DAKUTEN) {
                    output.push(composed);
                    chars.next();
                    continue;
                }
            }
            Some(&HALFWIDTH_HANDAKUTEN) => {
                if let Some(composed) = compose(full, HANDAKUTEN) {
                    output.push(composed);
                    chars.next();
                    continue;
                }
            }
            _ => {}
        }
        output.push(full);
    }
    output
}
fn to_halfwidth(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for c in text.chars() {
        if let Some((base, mark)) = decompose(c) {
            // Decomposed bases always have a halfwidth form.
            output.push(full_to_half(base).unwrap_or(base));
            output.push(mark);
        } else if let Some(half) = full_to_half(c) {
            output.push(half);
        } else if c == COMBINING_DAKUTEN || c == '\u{309b}' {
            output.push(HALFWIDTH_DAKUTEN);
        } else if c == COMBINING_HANDAKUTEN || c == '\u{309c}' {
            output.push(HALFWIDTH_HANDAKUTEN);
        } else {
            output.push(c);
        }
    }
    output
}
fn transform(text: &str, settings: &Settings) -> String {
    match settings.mode {
        Mode::Fullwidth => to_fullwidth(text),
        Mode::Halfwidth => to_halfwidth(text),
    }
}
impl Node for TextKanaWidth {
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
        definition.default_config = json!({"mode":"fullwidth"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["mode"],"properties":{"mode":{"type":"string","enum":["fullwidth","halfwidth"]}}});
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
                let converted = transform(text, &settings);
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
