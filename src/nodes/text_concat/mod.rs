//! 未採用の文字列連結ノード。入力元の位置や読みを推測して統合しません。
use std::sync::Arc;

use serde_json::{Value, json};

use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};

pub const KIND: &str = "experimental.text-concat";

#[derive(Debug, Clone, Copy, Default)]
pub struct TextConcat;

fn separator(config: &Value) -> Result<&str, CoreError> {
    let invalid = || CoreError::new("invalid-config", "expected exactly a string separator");
    let object = config.as_object().ok_or_else(invalid)?;
    if object.len() != 1 {
        return Err(invalid());
    }
    object
        .get("separator")
        .and_then(Value::as_str)
        .ok_or_else(invalid)
}

fn input_text(value: &OutputValue) -> Result<Option<&str>, CoreError> {
    match value {
        OutputValue::NoOutput => Ok(None),
        OutputValue::Data(packet) => {
            packet
                .validate()
                .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
            match &packet.root {
                Candidate::Text { text, .. } => Ok(Some(text)),
                _ => Err(CoreError::new("invalid-root", "expected Text")),
            }
        }
    }
}

impl Node for TextConcat {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            ["a", "b"]
                .into_iter()
                .map(|id| {
                    InputPort::required(
                        Port::packet(id)
                            .with_language("ja")
                            .with_root(RootConstraint::Text),
                    )
                })
                .collect(),
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(RootConstraint::Text),
            ],
        );
        definition.default_config = json!({"separator":""});
        definition.config_schema = json!({
            "type":"object",
            "required":["separator"],
            "additionalProperties":false,
            "properties":{"separator":{"type":"string"}}
        });
        definition
    }

    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        separator(config).map(|_| ())
    }

    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let separator = separator(config)?;
        let a = inputs
            .get("a")
            .ok_or_else(|| CoreError::new("missing-input", "required input: a"))?;
        let b = inputs
            .get("b")
            .ok_or_else(|| CoreError::new("missing-input", "required input: b"))?;
        let a = input_text(a)?;
        let b = input_text(b)?;
        let value = match (a, b) {
            (Some(a), Some(b)) => OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                text: format!("{a}{separator}{b}"),
                meta: None,
            }))),
            _ => OutputValue::NoOutput,
        };
        Ok([("out".into(), value)].into())
    }
}
