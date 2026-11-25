//! 未採用の実験ノード。契約はexamples/nodes/text_number_notation/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-number-notation";
const DIGITS: [char; 10] = ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
const MAX_DIGITS: usize = 16;
#[derive(Debug, Clone, Copy, Default)]
pub struct TextNumberNotation;
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    KanjiPositional,
    KanjiDigit,
    Arabic,
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
fn invalid(message: &str) -> CoreError {
    CoreError::new("invalid-number", message)
}
struct ParsedNumber {
    negative: bool,
    integer: String,
    fraction: Option<String>,
}
/// Strict Arabic notation: optional `-`, 1-16 integer digits without leading
/// zeros, and an optional fraction of 1-16 digits. Negative zero is rejected.
fn parse_arabic(text: &str) -> Result<ParsedNumber, CoreError> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (integer, fraction) = match body.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (body, None),
    };
    if integer.is_empty()
        || integer.len() > MAX_DIGITS
        || !integer.bytes().all(|b| b.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
    {
        return Err(invalid("integer part is not a bounded Arabic numeral"));
    }
    if let Some(fraction) = fraction
        && (fraction.is_empty()
            || fraction.len() > MAX_DIGITS
            || !fraction.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(invalid("fraction part is not a bounded Arabic numeral"));
    }
    let zero =
        integer.bytes().all(|b| b == b'0') && fraction.is_none_or(|f| f.bytes().all(|b| b == b'0'));
    if negative && zero {
        return Err(invalid("negative zero has no kanji notation"));
    }
    Ok(ParsedNumber {
        negative,
        integer: integer.to_string(),
        fraction: fraction.map(str::to_string),
    })
}
fn digit_value(c: char) -> Option<u64> {
    DIGITS.iter().position(|d| *d == c).map(|v| v as u64)
}
fn parse_kanji_fraction(text: &str) -> Result<String, CoreError> {
    if text.is_empty() || text.chars().count() > MAX_DIGITS {
        return Err(invalid("fraction part is not bounded kanji digits"));
    }
    let mut out = String::new();
    for c in text.chars() {
        let value = digit_value(c).ok_or_else(|| invalid("fraction must be digit-style kanji"))?;
        out.push(char::from_digit(value as u32, 10).unwrap());
    }
    Ok(out)
}
fn parse_kanji_integer(text: &str) -> Result<String, CoreError> {
    if text == "零" {
        return Ok("0".to_string());
    }
    if text.contains('零') {
        return Err(invalid("零 is only valid as the whole integer"));
    }
    if text
        .chars()
        .any(|c| matches!(c, '十' | '百' | '千' | '万' | '億' | '兆'))
    {
        parse_kanji_positional(text)
    } else {
        // Digit-style: every character is a kanji digit.
        if text.is_empty() || text.chars().count() > MAX_DIGITS {
            return Err(invalid("digit-style integer is not bounded"));
        }
        if text.starts_with('〇') && text.chars().count() > 1 {
            return Err(invalid("digit-style integer has a leading zero"));
        }
        let mut out = String::new();
        for c in text.chars() {
            let value = digit_value(c).ok_or_else(|| invalid("not a kanji numeral"))?;
            out.push(char::from_digit(value as u32, 10).unwrap());
        }
        Ok(out)
    }
}
fn parse_kanji_positional(text: &str) -> Result<String, CoreError> {
    let mut total: u64 = 0;
    let mut group: u64 = 0;
    let mut pending: Option<u64> = None;
    let mut last_big: u64 = u64::MAX;
    let mut used_small: Vec<char> = Vec::new();
    for c in text.chars() {
        match c {
            '一' | '二' | '三' | '四' | '五' | '六' | '七' | '八' | '九' => {
                if pending.is_some() {
                    return Err(invalid("consecutive digits in positional notation"));
                }
                pending = digit_value(c);
            }
            '十' | '百' | '千' => {
                if used_small.contains(&c) {
                    return Err(invalid("a small unit appears twice in one group"));
                }
                used_small.push(c);
                let unit = match c {
                    '十' => 10,
                    '百' => 100,
                    _ => 1000,
                };
                group += pending.take().unwrap_or(1) * unit;
            }
            '万' | '億' | '兆' => {
                let unit: u64 = match c {
                    '万' => 10_000,
                    '億' => 100_000_000,
                    _ => 1_000_000_000_000,
                };
                if unit >= last_big {
                    return Err(invalid("big units must descend"));
                }
                last_big = unit;
                let section = group + pending.take().unwrap_or(0);
                if section == 0 {
                    return Err(invalid("a big unit needs a value"));
                }
                total += section * unit;
                group = 0;
                used_small.clear();
            }
            _ => return Err(invalid("not a kanji numeral")),
        }
    }
    total += group + pending.unwrap_or(0);
    Ok(total.to_string())
}
/// Kanji notation: optional マイナス prefix, integer, optional 点 fraction.
fn parse_kanji(text: &str) -> Result<ParsedNumber, CoreError> {
    let (negative, body) = match text.strip_prefix("マイナス") {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if body.is_empty() {
        return Err(invalid("empty kanji numeral"));
    }
    let (integer, fraction) = match body.split_once('点') {
        Some((integer, fraction)) => (integer, Some(parse_kanji_fraction(fraction)?)),
        None => (body, None),
    };
    if integer.is_empty() {
        return Err(invalid("missing integer part"));
    }
    let integer = parse_kanji_integer(integer)?;
    let zero = integer.bytes().all(|b| b == b'0')
        && fraction
            .as_ref()
            .is_none_or(|f| f.bytes().all(|b| b == b'0'));
    if negative && zero {
        return Err(invalid("negative zero is not accepted"));
    }
    Ok(ParsedNumber {
        negative,
        integer,
        fraction,
    })
}
fn render_group(value: u64, out: &mut String) {
    let thousands = value / 1000;
    let hundreds = value / 100 % 10;
    let tens = value / 10 % 10;
    let ones = value % 10;
    if thousands == 1 {
        out.push('千');
    } else if thousands > 1 {
        out.push(DIGITS[thousands as usize]);
        out.push('千');
    }
    if hundreds == 1 {
        out.push('百');
    } else if hundreds > 1 {
        out.push(DIGITS[hundreds as usize]);
        out.push('百');
    }
    if tens == 1 {
        out.push('十');
    } else if tens > 1 {
        out.push(DIGITS[tens as usize]);
        out.push('十');
    }
    if ones > 0 {
        out.push(DIGITS[ones as usize]);
    }
}
fn to_kanji_positional(number: &ParsedNumber) -> String {
    let mut out = String::new();
    if number.negative {
        out.push_str("マイナス");
    }
    let value: u64 = number.integer.parse().unwrap_or(0);
    if value == 0 {
        out.push('零');
    } else {
        let groups: [(u64, &str); 4] = [
            (value / 1_000_000_000_000 % 10_000, "兆"),
            (value / 100_000_000 % 10_000, "億"),
            (value / 10_000 % 10_000, "万"),
            (value % 10_000, ""),
        ];
        for (group, unit) in groups {
            if group > 0 {
                render_group(group, &mut out);
                out.push_str(unit);
            }
        }
    }
    if let Some(fraction) = &number.fraction {
        out.push('点');
        for b in fraction.bytes() {
            out.push(DIGITS[(b - b'0') as usize]);
        }
    }
    out
}
fn to_kanji_digit(number: &ParsedNumber) -> String {
    let mut out = String::new();
    if number.negative {
        out.push_str("マイナス");
    }
    for b in number.integer.bytes() {
        out.push(DIGITS[(b - b'0') as usize]);
    }
    if let Some(fraction) = &number.fraction {
        out.push('点');
        for b in fraction.bytes() {
            out.push(DIGITS[(b - b'0') as usize]);
        }
    }
    out
}
fn to_arabic(number: &ParsedNumber) -> String {
    let mut out = String::new();
    if number.negative {
        out.push('-');
    }
    out.push_str(&number.integer);
    if let Some(fraction) = &number.fraction {
        out.push('.');
        out.push_str(fraction);
    }
    out
}
fn convert(text: &str, mode: Mode) -> Result<String, CoreError> {
    match mode {
        Mode::KanjiPositional => Ok(to_kanji_positional(&parse_arabic(text)?)),
        Mode::KanjiDigit => Ok(to_kanji_digit(&parse_arabic(text)?)),
        Mode::Arabic => Ok(to_arabic(&parse_kanji(text)?)),
    }
}
impl Node for TextNumberNotation {
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
        definition.default_config = json!({"mode":"kanji-positional"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["mode"],"properties":{"mode":{"type":"string","enum":["kanji-positional","kanji-digit","arabic"]}}});
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
                let converted = convert(text, settings.mode)?;
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
