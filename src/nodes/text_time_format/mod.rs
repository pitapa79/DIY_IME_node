//! 未採用の実験ノード。契約はexamples/nodes/text_time_format/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-time-format";
#[derive(Debug, Clone, Copy, Default)]
pub struct TextTimeFormat;
#[derive(serde::Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Format {
    H24,
    H12,
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
    CoreError::new("invalid-time", message)
}
/// A time of day. `hour` 24 is the end-of-day marker and only ever carries
/// minute 0 and second 0/None.
#[derive(Clone, Copy, PartialEq)]
struct Time {
    hour: u32,
    minute: u32,
    second: Option<u32>,
}
fn digits(text: &str, min: usize, max: usize) -> Option<u32> {
    if (min..=max).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}
fn parse_colon_parts(text: &str) -> Result<(u32, u32, Option<u32>), CoreError> {
    let mut parts = text.split(':');
    let (Some(h), Some(m), second, None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(invalid("expected H:MM[:SS]"));
    };
    let hour = digits(h, 1, 2).ok_or_else(|| invalid("hour must be 1-2 digits"))?;
    let minute = digits(m, 2, 2).ok_or_else(|| invalid("minute must be 2 digits"))?;
    let second = match second {
        Some(s) => Some(digits(s, 2, 2).ok_or_else(|| invalid("second must be 2 digits"))?),
        None => None,
    };
    Ok((hour, minute, second))
}
fn check_minute_second(minute: u32, second: Option<u32>) -> Result<(), CoreError> {
    if minute > 59 {
        return Err(invalid("minute out of range"));
    }
    if second.is_some_and(|s| s > 59) {
        return Err(invalid("second out of range"));
    }
    Ok(())
}
fn parse_time(text: &str, format: Format) -> Result<Time, CoreError> {
    match format {
        Format::H24 => {
            let (hour, minute, second) = parse_colon_parts(text)?;
            check_minute_second(minute, second)?;
            if hour == 24 {
                if minute != 0 || second.is_some_and(|s| s != 0) {
                    return Err(invalid("only 24:00 is a valid 24-hour time"));
                }
            } else if hour > 24 {
                return Err(invalid("hour out of range"));
            }
            Ok(Time {
                hour,
                minute,
                second,
            })
        }
        Format::H12 => {
            let (body, pm) = if let Some(body) = text.strip_suffix(" AM") {
                (body, false)
            } else if let Some(body) = text.strip_suffix(" PM") {
                (body, true)
            } else {
                return Err(invalid("expected an AM or PM suffix"));
            };
            let (hour, minute, second) = parse_colon_parts(body)?;
            check_minute_second(minute, second)?;
            if !(1..=12).contains(&hour) {
                return Err(invalid("12-hour clock hour must be 1-12"));
            }
            let hour = match (hour, pm) {
                (12, false) => 0,
                (12, true) => 12,
                (h, false) => h,
                (h, true) => h + 12,
            };
            Ok(Time {
                hour,
                minute,
                second,
            })
        }
        Format::Japanese => {
            let (body, pm) = if let Some(body) = text.strip_prefix("午前") {
                (body, false)
            } else if let Some(body) = text.strip_prefix("午後") {
                (body, true)
            } else {
                return Err(invalid("expected a 午前 or 午後 prefix"));
            };
            let Some((h, rest)) = body.split_once('時') else {
                return Err(invalid("expected H時M分"));
            };
            let Some((m, rest)) = rest.split_once('分') else {
                return Err(invalid("expected H時M分"));
            };
            let hour = digits(h, 1, 2).ok_or_else(|| invalid("hour must be 1-2 digits"))?;
            let minute = digits(m, 1, 2).ok_or_else(|| invalid("minute must be 1-2 digits"))?;
            let second = if rest.is_empty() {
                None
            } else {
                let s = rest
                    .strip_suffix('秒')
                    .ok_or_else(|| invalid("expected S秒"))?;
                Some(digits(s, 1, 2).ok_or_else(|| invalid("second must be 1-2 digits"))?)
            };
            check_minute_second(minute, second)?;
            if !(1..=12).contains(&hour) {
                return Err(invalid("12-hour clock hour must be 1-12"));
            }
            let hour = match (hour, pm) {
                (12, false) => 0,
                (12, true) => 12,
                (h, false) => h,
                (h, true) => h + 12,
            };
            Ok(Time {
                hour,
                minute,
                second,
            })
        }
    }
}
fn render_time(time: Time, format: Format) -> Result<String, CoreError> {
    match format {
        Format::H24 => {
            let mut out = format!("{:02}:{:02}", time.hour, time.minute);
            if let Some(second) = time.second {
                out.push_str(&format!(":{second:02}"));
            }
            Ok(out)
        }
        Format::H12 | Format::Japanese => {
            if time.hour == 24 {
                return Err(invalid("24:00 has no 12-hour representation"));
            }
            let pm = time.hour >= 12;
            let hour12 = match time.hour % 12 {
                0 => 12,
                h => h,
            };
            if format == Format::H12 {
                let mut out = format!("{}:{:02}", hour12, time.minute);
                if let Some(second) = time.second {
                    out.push_str(&format!(":{second:02}"));
                }
                out.push_str(if pm { " PM" } else { " AM" });
                Ok(out)
            } else {
                let mut out = format!(
                    "{}{}時{:02}分",
                    if pm { "午後" } else { "午前" },
                    hour12,
                    time.minute
                );
                if let Some(second) = time.second {
                    out.push_str(&format!("{second:02}秒"));
                }
                Ok(out)
            }
        }
    }
}
fn convert(text: &str, settings: &Settings) -> Result<String, CoreError> {
    render_time(parse_time(text, settings.from)?, settings.to)
}
impl Node for TextTimeFormat {
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
        definition.default_config = json!({"from":"h24","to":"japanese"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["from","to"],"properties":{"from":{"type":"string","enum":["h24","h12","japanese"]},"to":{"type":"string","enum":["h24","h12","japanese"]}}});
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
