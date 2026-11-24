//! 未採用の実験ノード。契約はexamples/nodes/text_compare/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-compare";
#[derive(Debug, Clone, Copy, Default)]
pub struct TextCompare;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    mode: Mode,
}
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Exact,
    Contains,
    Prefix,
    Suffix,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    const KEYS: &[&str] = &["mode"];
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
fn matches(subject: &str, pattern: &str, settings: &Settings) -> bool {
    match settings.mode {
        Mode::Exact => subject == pattern,
        Mode::Contains => subject.contains(pattern),
        Mode::Prefix => subject.starts_with(pattern),
        Mode::Suffix => subject.ends_with(pattern),
    }
}
fn input_packet(value: &OutputValue) -> Result<Option<&Arc<Packet>>, CoreError> {
    match value {
        OutputValue::NoOutput => Ok(None),
        OutputValue::Data(packet) => {
            packet
                .validate()
                .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
            match &packet.root {
                Candidate::Text { .. } => Ok(Some(packet)),
                _ => Err(CoreError::new("invalid-root", "expected Text")),
            }
        }
    }
}
impl Node for TextCompare {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Text)
        };
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![
                InputPort::required(port("a")),
                InputPort::required(port("b")),
            ],
            vec![port("matched"), port("unmatched")],
        );
        definition.default_config = json!({"mode":"exact"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["mode"],"properties":{"mode":{"type":"string","enum":["exact","contains","prefix","suffix"]}}});
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
        let a = inputs
            .get("a")
            .ok_or_else(|| CoreError::new("missing-input", "required input: a"))?;
        let b = inputs
            .get("b")
            .ok_or_else(|| CoreError::new("missing-input", "required input: b"))?;
        let a = input_packet(a)?;
        let b = input_packet(b)?;
        let (matched, unmatched) = match (a, b) {
            (Some(packet), Some(pattern)) => {
                let Candidate::Text { text: subject, .. } = &packet.root else {
                    unreachable!("root was checked by input_packet")
                };
                let Candidate::Text { text: pattern, .. } = &pattern.root else {
                    unreachable!("root was checked by input_packet")
                };
                if matches(subject, pattern, &settings) {
                    (OutputValue::Data(Arc::clone(packet)), OutputValue::NoOutput)
                } else {
                    (OutputValue::NoOutput, OutputValue::Data(Arc::clone(packet)))
                }
            }
            _ => (OutputValue::NoOutput, OutputValue::NoOutput),
        };
        Ok([("matched".into(), matched), ("unmatched".into(), unmatched)].into())
    }
}
