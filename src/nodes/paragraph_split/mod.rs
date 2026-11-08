//! Literal paragraph splitting by runs of at least two line endings.
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;

pub const KIND: &str = "experimental.paragraph-split";
pub struct ParagraphSplit;

/// CRLF is one line ending; lone CR and LF each count as one.
/// Keep the complete separator run in the preceding chunk, preserving all bytes.
pub fn split(input: &str) -> Vec<String> {
    let bytes = input.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if !matches!(bytes[index], b'\r' | b'\n') {
            index += 1;
            continue;
        }
        let mut endings = 0;
        while index < bytes.len() && matches!(bytes[index], b'\r' | b'\n') {
            if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index += 2;
            } else {
                index += 1;
            }
            endings += 1;
        }
        if endings >= 2 {
            // ASCII line endings always end at a UTF-8 character boundary.
            chunks.push(input[start..index].to_owned());
            start = index;
        }
    }
    if start < bytes.len() || chunks.is_empty() {
        chunks.push(input[start..].to_owned());
    }
    chunks
}

impl Node for ParagraphSplit {
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
        definition.default_config = json!({});
        definition.config_schema =
            json!({"type":"object","properties":{},"additionalProperties":false});
        definition
    }

    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        self.validate_config(config)?;
        let value = match inputs.get("in") {
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence {
                    items: split(text)
                        .into_iter()
                        .map(|text| Candidate::Text { text, meta: None })
                        .collect(),
                })))
            }
            None => return Err(CoreError::new("missing-input", "in is required")),
        };
        Ok([("out".into(), value)].into())
    }
}
