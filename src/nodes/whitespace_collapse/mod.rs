//! 未採用の実験ノード。契約はexamples/nodes/whitespace_collapse/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.whitespace-collapse";
#[derive(Debug, Clone, Copy, Default)]
pub struct WhitespaceCollapse;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    replacement: String,
    trim: bool,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    const KEYS: &[&str] = &["replacement", "trim"];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;

    Ok(settings)
}
fn transform(text: &str, settings: &Settings) -> Result<String, CoreError> {
    let mut out = String::new();
    let mut pending = false;
    for character in text.chars() {
        if character.is_whitespace() {
            pending = true;
        } else {
            if pending && (!settings.trim || !out.is_empty()) {
                out.push_str(&settings.replacement);
            }
            pending = false;
            out.push(character);
        }
    }
    if pending && !settings.trim {
        out.push_str(&settings.replacement);
    }
    Ok(out)
}
impl Node for WhitespaceCollapse {
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
        definition.default_config = json!({"replacement":" ","trim":false});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["replacement","trim"],"properties":{"replacement":{"type":"string"},"trim":{"type":"boolean"}}});
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
                let converted = transform(text, &settings)?;
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
