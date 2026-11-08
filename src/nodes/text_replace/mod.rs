//! 未採用の文字列置換ノード。正規表現や暗黙のUnicode正規化は行いません。
use std::sync::Arc;

use serde_json::{Value, json};

use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};

pub const KIND: &str = "experimental.text-replace";

#[derive(Debug, Clone, Copy, Default)]
pub struct TextReplace;

fn settings(config: &Value) -> Result<(&str, &str), CoreError> {
    let invalid = || {
        CoreError::new(
            "invalid-config",
            "expected exactly string settings from (nonempty) and to",
        )
    };
    let object = config.as_object().ok_or_else(invalid)?;
    if object.len() != 2 {
        return Err(invalid());
    }
    let from = object
        .get("from")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    let to = object
        .get("to")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if from.is_empty() {
        return Err(invalid());
    }
    Ok((from, to))
}

impl Node for TextReplace {
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
        definition.default_config = json!({"from":"置換前", "to":"置換後"});
        definition.config_schema = json!({
            "type":"object",
            "required":["from","to"],
            "additionalProperties":false,
            "properties":{
                "from":{"type":"string","minLength":1},
                "to":{"type":"string"}
            }
        });
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
        let (from, to) = settings(config)?;
        let value = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                let replaced = text.replace(from, to);
                if replaced == *text {
                    OutputValue::Data(Arc::clone(packet))
                } else {
                    OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                        text: replaced,
                        meta: None,
                    })))
                }
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), value)].into())
    }
}
