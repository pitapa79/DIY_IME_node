//! 未採用の実験ノード。契約はexamples/nodes/text_language_detect/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-language-detect";
#[derive(Debug, Clone, Copy, Default)]
pub struct TextLanguageDetect;
fn settings(config: &Value) -> Result<(), CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if !object.is_empty() {
        return Err(CoreError::new(
            "invalid-config",
            "this node declares no settings",
        ));
    }
    Ok(())
}
fn detect(text: &str) -> &'static str {
    let mut kana = false;
    let mut other_letters = false;
    let mut latin = false;
    for ch in text.chars() {
        match ch {
            '\u{3040}'..='\u{30FF}' | '\u{FF66}'..='\u{FF9D}' if ch.is_alphabetic() => {
                kana = true;
            }
            'A'..='Z' | 'a'..='z' => latin = true,
            '々' => other_letters = true,
            _ if ch.is_alphabetic() => other_letters = true,
            _ => {}
        }
    }
    if kana {
        "ja"
    } else if latin && !other_letters {
        "en"
    } else {
        "und"
    }
}
impl Node for TextLanguageDetect {
    fn definition(&self) -> NodeDefinition {
        // 英語も受け付けるため、ポートに言語制約は付けない。
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(
                Port::packet("in").with_root(RootConstraint::Text),
            )],
            vec![Port::packet("out").with_root(RootConstraint::Text)],
        );
        definition.default_config = json!({});
        definition.config_schema = json!({"type":"object","additionalProperties":false});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        settings(config)
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        settings(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: detect(text).into(),
                    meta: None,
                })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
