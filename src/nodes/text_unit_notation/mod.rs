//! 未採用の実験ノード。契約はexamples/nodes/text_unit_notation/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-unit-notation";
/// 固定の単位表（記号 → 日本語名）。大小文字は区別する。
const UNITS: &[(&str, &str)] = &[
    ("km/h", "キロメートル毎時"),
    ("m/s", "メートル毎秒"),
    ("km²", "平方キロメートル"),
    ("cm²", "平方センチメートル"),
    ("m²", "平方メートル"),
    ("m³", "立方メートル"),
    ("km", "キロメートル"),
    ("cm", "センチメートル"),
    ("mm", "ミリメートル"),
    ("μm", "マイクロメートル"),
    ("nm", "ナノメートル"),
    ("kg", "キログラム"),
    ("mg", "ミリグラム"),
    ("μg", "マイクログラム"),
    ("kL", "キロリットル"),
    ("mL", "ミリリットル"),
    ("dL", "デシリットル"),
    ("ms", "ミリ秒"),
    ("min", "分"),
    ("kcal", "キロカロリー"),
    ("cal", "カロリー"),
    ("hPa", "ヘクトパスカル"),
    ("Pa", "パスカル"),
    ("kW", "キロワット"),
    ("kHz", "キロヘルツ"),
    ("MHz", "メガヘルツ"),
    ("Hz", "ヘルツ"),
    ("KB", "キロバイト"),
    ("MB", "メガバイト"),
    ("GB", "ギガバイト"),
    ("TB", "テラバイト"),
    ("m", "メートル"),
    ("g", "グラム"),
    ("t", "トン"),
    ("L", "リットル"),
    ("s", "秒"),
    ("h", "時間"),
    ("W", "ワット"),
    ("V", "ボルト"),
    ("A", "アンペア"),
    ("J", "ジュール"),
    ("N", "ニュートン"),
    ("B", "バイト"),
    ("℃", "度"),
    ("%", "パーセント"),
];
#[derive(Debug, Clone, Copy, Default)]
pub struct TextUnitNotation;
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
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}
/// 数値の終端（排他）を返す。桁区切りカンマ（3桁組）と小数部を含む。
fn number_end(chars: &[char], start: usize) -> usize {
    let mut end = start;
    while end < chars.len() && is_digit(chars[end]) {
        end += 1;
    }
    while end + 4 <= chars.len()
        && chars[end] == ','
        && (end + 1..end + 4).all(|i| is_digit(chars[i]))
        && chars.get(end + 4).is_none_or(|c| !is_digit(*c))
    {
        end += 4;
    }
    if end + 1 < chars.len() && chars[end] == '.' && is_digit(chars[end + 1]) {
        end += 1;
        while end < chars.len() && is_digit(chars[end]) {
            end += 1;
        }
    }
    end
}
/// `position` から始まる最長の単位記号と、その名前・文字数を返す。
/// 単位の直後が英数字の場合は単語の一部とみなして一致させない。
fn unit_at(chars: &[char], position: usize) -> Option<(&'static str, usize)> {
    let mut best: Option<(&'static str, usize)> = None;
    for (symbol, name) in UNITS {
        let symbol_chars: Vec<char> = symbol.chars().collect();
        let len = symbol_chars.len();
        if chars.len() < position + len {
            continue;
        }
        if chars[position..position + len] != symbol_chars[..] {
            continue;
        }
        if chars
            .get(position + len)
            .is_some_and(|c| c.is_ascii_alphanumeric())
        {
            continue;
        }
        if best.is_none_or(|(_, best_len)| len > best_len) {
            best = Some((name, len));
        }
    }
    best
}
fn convert(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let starts_number = is_digit(chars[index])
            && (index == 0 || !matches!(chars[index - 1], '0'..='9' | '.' | ','));
        if !starts_number {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        let end = number_end(&chars, index);
        let mut unit_start = end;
        if chars.get(unit_start) == Some(&' ') {
            unit_start += 1;
        }
        match unit_at(&chars, unit_start) {
            Some((name, len)) => {
                output.extend(&chars[index..end]);
                output.push_str(name);
                index = unit_start + len;
            }
            None => {
                output.extend(&chars[index..end]);
                index = end;
            }
        }
    }
    output
}
impl Node for TextUnitNotation {
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
                let converted = convert(text);
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
