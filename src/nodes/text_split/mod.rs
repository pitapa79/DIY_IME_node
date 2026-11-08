//! 未採用のリテラル区切り文字列分割ノード。
use std::sync::Arc;

use serde_json::{Value, json};

use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};

pub const KIND: &str = "experimental.text-split";

#[derive(Debug, Clone, Copy, Default)]
pub struct TextSplit;

fn separator(config: &Value) -> Result<&str, CoreError> {
    let invalid = || CoreError::new("invalid-config", "expected a nonempty string separator");
    let object = config.as_object().ok_or_else(invalid)?;
    if object.len() != 1 {
        return Err(invalid());
    }
    object
        .get("separator")
        .and_then(Value::as_str)
        .filter(|separator| !separator.is_empty())
        .ok_or_else(invalid)
}

impl Node for TextSplit {
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
                    .with_root(RootConstraint::Sequence),
            ],
        );
        definition.default_config = json!({"separator":","});
        definition.config_schema = json!({
            "type":"object",
            "required":["separator"],
            "additionalProperties":false,
            "properties":{"separator":{"type":"string","minLength":1}}
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
        let value = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                let items = text
                    .split(separator)
                    .map(|part| Candidate::Text {
                        text: part.into(),
                        meta: None,
                    })
                    .collect();
                OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), value)].into())
    }
}
