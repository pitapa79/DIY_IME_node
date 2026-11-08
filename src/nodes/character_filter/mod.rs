//! 未採用の実験ノード。契約はexamples/nodes/character_filter/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.character-filter";
#[derive(Debug, Clone, Copy, Default)]
pub struct CharacterFilter;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    class: Class,
    keep: bool,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Class {
    AsciiDigits,
    AsciiLetters,
    AsciiAlphanumeric,
    Whitespace,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    const KEYS: &[&str] = &["class", "keep"];
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
    Ok(text
        .chars()
        .filter(|c| {
            let matches = match settings.class {
                Class::AsciiDigits => c.is_ascii_digit(),
                Class::AsciiLetters => c.is_ascii_alphabetic(),
                Class::AsciiAlphanumeric => c.is_ascii_alphanumeric(),
                Class::Whitespace => c.is_whitespace(),
            };
            matches == settings.keep
        })
        .collect())
}
impl Node for CharacterFilter {
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
        definition.default_config = json!({"class":"ascii-digits","keep":true});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["class","keep"],"properties":{"class":{"type":"string","enum":["ascii-digits","ascii-letters","ascii-alphanumeric","whitespace"]},"keep":{"type":"boolean"}}});
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
