//! 未採用の実験ノード。契約はexamples/nodes/text_date_format/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-date-format";
#[derive(Debug, Clone, Copy, Default)]
pub struct TextDateFormat;
#[derive(serde::Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Format {
    Iso,
    Slash,
    Japanese,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    from: Format,
    to: Format,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 2 || !object.contains_key("from") || !object.contains_key("to") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))
}
fn invalid(message: &str) -> CoreError {
    CoreError::new("invalid-date", message)
}
fn parse_fixed(text: &str, digits: usize) -> Option<u32> {
    if text.len() == digits && text.bytes().all(|b| b.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}
fn parse_short(text: &str) -> Option<u32> {
    if (1..=2).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}
fn parse_date(text: &str, format: Format) -> Result<(u32, u32, u32), CoreError> {
    let (year, month, day) = match format {
        Format::Iso => {
            let mut parts = text.split('-');
            let (Some(y), Some(m), Some(d), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return Err(invalid("expected YYYY-MM-DD"));
            };
            (
                parse_fixed(y, 4).ok_or_else(|| invalid("year must be 4 digits"))?,
                parse_fixed(m, 2).ok_or_else(|| invalid("month must be 2 digits"))?,
                parse_fixed(d, 2).ok_or_else(|| invalid("day must be 2 digits"))?,
            )
        }
        Format::Slash => {
            let mut parts = text.split('/');
            let (Some(y), Some(m), Some(d), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return Err(invalid("expected YYYY/M/D"));
            };
            (
                parse_fixed(y, 4).ok_or_else(|| invalid("year must be 4 digits"))?,
                parse_short(m).ok_or_else(|| invalid("month must be 1-2 digits"))?,
                parse_short(d).ok_or_else(|| invalid("day must be 1-2 digits"))?,
            )
        }
        Format::Japanese => {
            let Some((y, rest)) = text.split_once('年') else {
                return Err(invalid("expected YYYY年M月D日"));
            };
            let Some((m, d)) = rest.split_once('月') else {
                return Err(invalid("expected YYYY年M月D日"));
            };
            let Some(d) = d.strip_suffix('日') else {
                return Err(invalid("expected YYYY年M月D日"));
            };
            (
                parse_fixed(y, 4).ok_or_else(|| invalid("year must be 4 digits"))?,
                parse_short(m).ok_or_else(|| invalid("month must be 1-2 digits"))?,
                parse_short(d).ok_or_else(|| invalid("day must be 1-2 digits"))?,
            )
        }
    };
    validate_date(year, month, day)?;
    Ok((year, month, day))
}
fn validate_date(year: u32, month: u32, day: u32) -> Result<(), CoreError> {
    if !(1..=9999).contains(&year) {
        return Err(invalid("year out of range"));
    }
    if !(1..=12).contains(&month) {
        return Err(invalid("month out of range"));
    }
    let leap =
        (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day < 1 || day > days {
        return Err(invalid("day does not exist in this month"));
    }
    Ok(())
}
fn render_date(date: (u32, u32, u32), format: Format) -> String {
    let (year, month, day) = date;
    match format {
        Format::Iso => format!("{year:04}-{month:02}-{day:02}"),
        Format::Slash => format!("{year:04}/{month:02}/{day:02}"),
        Format::Japanese => format!("{year}年{month}月{day}日"),
    }
}
fn convert(text: &str, settings: &Settings) -> Result<String, CoreError> {
    Ok(render_date(parse_date(text, settings.from)?, settings.to))
}
impl Node for TextDateFormat {
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
        definition.default_config = json!({"from":"iso","to":"japanese"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["from","to"],"properties":{"from":{"type":"string","enum":["iso","slash","japanese"]},"to":{"type":"string","enum":["iso","slash","japanese"]}}});
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
                let converted = convert(text, &settings)?;
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
