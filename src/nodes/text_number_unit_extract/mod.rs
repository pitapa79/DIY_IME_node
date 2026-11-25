//! 未採用の実験ノード。契約はexamples/nodes/text_number_unit_extract/README.md。
use crate::{
    Candidate, Packet, TextMeta,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-number-unit-extract";
pub const MAX_LIMIT: usize = 1024;
const DEFAULT_LIMIT: usize = 64;
/// 固定の単位表。記号単位（SI等）と日本語の助数詞。最長一致で選ぶ。
const UNITS: &[&str] = &[
    "km/h", "m/s", "km²", "cm²", "m²", "m³", "km", "cm", "mm", "μm", "nm", "kg", "mg", "μg", "kL",
    "mL", "dL", "ms", "min", "kcal", "cal", "hPa", "Pa", "kW", "kHz", "MHz", "Hz", "KB", "MB",
    "GB", "TB", "時間", "週間", "m", "g", "t", "L", "s", "h", "W", "V", "A", "J", "N", "B", "℃",
    "%", "円", "人", "個", "歳", "年", "月", "日", "時", "分", "秒", "回", "本", "枚", "台", "件",
    "名", "匹", "頭", "羽", "冊", "杯", "軒", "階", "棟", "粒", "袋", "箱", "番", "割",
];
#[derive(Debug, Clone, Copy, Default)]
pub struct TextNumberUnitExtract;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    limit: usize,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("limit") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;
    if !(1..=MAX_LIMIT).contains(&settings.limit) {
        return Err(CoreError::new(
            "invalid-config",
            "limit must be an integer from 1 to 1024",
        ));
    }
    Ok(settings)
}
fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}
/// 数値の終端（排他）。整数部と小数部のみ（桁区切りカンマは含まない）。
fn number_end(chars: &[char], start: usize) -> usize {
    let mut end = start;
    while end < chars.len() && is_digit(chars[end]) {
        end += 1;
    }
    if end + 1 < chars.len() && chars[end] == '.' && is_digit(chars[end + 1]) {
        end += 1;
        while end < chars.len() && is_digit(chars[end]) {
            end += 1;
        }
    }
    end
}
/// `position` から始まる最長の単位と、その文字数を返す。ASCII記号単位は、
/// 直後が英数字なら単語の一部とみなして一致させない。
fn unit_at(chars: &[char], position: usize) -> Option<usize> {
    let mut best: Option<usize> = None;
    for unit in UNITS {
        let unit_chars: Vec<char> = unit.chars().collect();
        let len = unit_chars.len();
        if chars.len() < position + len {
            continue;
        }
        if chars[position..position + len] != unit_chars[..] {
            continue;
        }
        if unit.chars().next().is_some_and(|c| c.is_ascii())
            && chars
                .get(position + len)
                .is_some_and(|c| c.is_ascii_alphanumeric())
        {
            continue;
        }
        if best.is_none_or(|best_len| len > best_len) {
            best = Some(len);
        }
    }
    best
}
fn extract(text: &str, limit: usize) -> Vec<(String, [usize; 2])> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index < chars.len() && found.len() < limit {
        let previous = if index == 0 {
            None
        } else {
            Some(chars[index - 1])
        };
        let sign_start = matches!(chars[index], '+' | '-')
            && previous.is_none_or(|p| !is_digit(p) && p != '.' && p != '+' && p != '-')
            && chars.get(index + 1).is_some_and(|c| is_digit(*c));
        let digit_start =
            is_digit(chars[index]) && previous.is_none_or(|p| !is_digit(p) && p != '.');
        if !sign_start && !digit_start {
            index += 1;
            continue;
        }
        let digits_start = if sign_start { index + 1 } else { index };
        let end = number_end(&chars, digits_start);
        let mut unit_start = end;
        if chars.get(unit_start) == Some(&' ') {
            unit_start += 1;
        }
        match unit_at(&chars, unit_start) {
            Some(len) => {
                let item_end = unit_start + len;
                found.push((chars[index..item_end].iter().collect(), [index, item_end]));
                index = item_end;
            }
            None => index = end,
        }
    }
    found
}
impl Node for TextNumberUnitExtract {
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
                    .with_language("ja")
                    .with_root(RootConstraint::Sequence),
            ],
        );
        definition.default_config = json!({"limit": DEFAULT_LIMIT});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["limit"],"properties":{"limit":{"type":"integer","minimum":1,"maximum":MAX_LIMIT}}});
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
                let items = extract(text, settings.limit)
                    .into_iter()
                    .map(|(item_text, span)| Candidate::Text {
                        text: item_text,
                        meta: Some(TextMeta {
                            reading: None,
                            source_span: Some(span),
                        }),
                    })
                    .collect();
                OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
