//! Literal Japanese punctuation styles; ASCII punctuation is never inferred.
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.japanese-punctuation";
pub struct JapanesePunctuation;
#[derive(Clone, Copy)]
pub enum Style {
    Japanese,
    Fullwidth,
    Ascii,
}
fn style(config: &Value) -> Result<Style, CoreError> {
    match config
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("style"))
        .and_then(Value::as_str)
    {
        Some("japanese") => Ok(Style::Japanese),
        Some("fullwidth") => Ok(Style::Fullwidth),
        Some("ascii") => Ok(Style::Ascii),
        _ => Err(CoreError::new(
            "invalid-config",
            "expected {style: japanese|fullwidth|ascii}",
        )),
    }
}
pub fn convert(input: &str, style: Style) -> String {
    let (comma, period) = match style {
        Style::Japanese => ('、', '。'),
        Style::Fullwidth => ('，', '．'),
        Style::Ascii => (',', '.'),
    };
    input
        .chars()
        .map(|ch| match ch {
            '、' | '，' => comma,
            '。' | '．' => period,
            _ => ch,
        })
        .collect()
}
impl Node for JapanesePunctuation {
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
        definition.default_config = json!({"style":"japanese"});
        definition.config_schema = json!({"type":"object","properties":{"style":{"type":"string","enum":["japanese","fullwidth","ascii"]}},"required":["style"],"additionalProperties":false});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        style(config).map(|_| ())
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let style = style(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: convert(text, style),
                    meta: None,
                })))
            }
            None => return Err(CoreError::new("missing-input", "in is required")),
        };
        Ok([("out".into(), output)].into())
    }
}
