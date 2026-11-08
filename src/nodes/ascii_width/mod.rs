//! Literal ASCII width conversion. Not NFKC or kana normalization.
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.ascii-width";
pub struct AsciiWidth;
#[derive(Clone, Copy)]
pub enum Mode {
    Fullwidth,
    Halfwidth,
}
fn config(value: &Value) -> Result<(Mode, bool), CoreError> {
    let invalid = || {
        CoreError::new(
            "invalid-config",
            "expected {mode: fullwidth|halfwidth, space: boolean}",
        )
    };
    let object = value
        .as_object()
        .filter(|object| object.len() == 2)
        .ok_or_else(invalid)?;
    let mode = match object.get("mode").and_then(Value::as_str) {
        Some("fullwidth") => Mode::Fullwidth,
        Some("halfwidth") => Mode::Halfwidth,
        _ => return Err(invalid()),
    };
    let space = object
        .get("space")
        .and_then(Value::as_bool)
        .ok_or_else(invalid)?;
    Ok((mode, space))
}
pub fn convert(input: &str, mode: Mode, space: bool) -> String {
    input
        .chars()
        .map(|ch| match mode {
            Mode::Fullwidth => match ch {
                '!'..='~' => char::from_u32(ch as u32 + 0xfee0).expect("fullwidth ASCII scalar"),
                ' ' if space => '\u{3000}',
                _ => ch,
            },
            Mode::Halfwidth => match ch {
                '\u{ff01}'..='\u{ff5e}' => {
                    char::from_u32(ch as u32 - 0xfee0).expect("ASCII scalar")
                }
                '\u{3000}' if space => ' ',
                _ => ch,
            },
        })
        .collect()
}
impl Node for AsciiWidth {
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
                    .with_root(RootConstraint::Text),
            ],
        );
        definition.default_config = json!({"mode":"fullwidth","space":true});
        definition.config_schema = json!({"type":"object","properties":{"mode":{"type":"string","enum":["fullwidth","halfwidth"]},"space":{"type":"boolean"}},"required":["mode","space"],"additionalProperties":false});
        definition
    }
    fn validate_config(&self, value: &Value) -> Result<(), CoreError> {
        config(value).map(|_| ())
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        value: &Value,
    ) -> Result<Outputs, CoreError> {
        let (mode, space) = config(value)?;
        let result = match inputs.get("in") {
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: convert(text, mode, space),
                    meta: None,
                })))
            }
            None => return Err(CoreError::new("missing-input", "in is required")),
        };
        Ok([("out".into(), result)].into())
    }
}
