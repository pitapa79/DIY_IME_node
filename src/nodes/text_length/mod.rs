//! 未採用の実験ノード。契約はexamples/nodes/text_length/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;
pub const KIND: &str = "experimental.text-length";
#[derive(Debug, Clone, Copy, Default)]
pub struct TextLength;
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Unit {
    Scalar,
    Byte,
    Grapheme,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    unit: Unit,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("unit") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))
}
fn measure(text: &str, unit: Unit) -> usize {
    match unit {
        Unit::Scalar => text.chars().count(),
        Unit::Byte => text.len(),
        Unit::Grapheme => text.graphemes(true).count(),
    }
}
impl Node for TextLength {
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
        definition.default_config = json!({"unit":"scalar"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["unit"],"properties":{"unit":{"type":"string","enum":["scalar","byte","grapheme"]}}});
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
                // The count is a new fact about the input: the output never
                // inherits the input meta, even when the digits coincide.
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: measure(text, settings.unit).to_string(),
                    meta: None,
                })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
