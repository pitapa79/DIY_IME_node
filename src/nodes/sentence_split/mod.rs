//! Experimental scalar-based splitting, not linguistic sentence recognition.
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};
pub const KIND: &str = "experimental.sentence-split";
pub struct SentenceSplit;
fn terminators(config: &Value) -> Result<&str, CoreError> {
    config
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("terminators"))
        .and_then(Value::as_str)
        .ok_or_else(|| CoreError::new("invalid-config", "expected {terminators: string}"))
}
/// Preserve every byte; adjacent terminators stay in the same preceding chunk.
pub fn split(input: &str, terminators: &str) -> Vec<String> {
    let terminators: BTreeSet<char> = terminators.chars().collect();
    let mut start = 0;
    let mut previous_terminator = false;
    let mut result = vec![];
    for (index, ch) in input.char_indices() {
        let current = terminators.contains(&ch);
        if previous_terminator && !current {
            result.push(input[start..index].to_owned());
            start = index;
        }
        previous_terminator = current;
    }
    result.push(input[start..].to_owned());
    result
}
impl Node for SentenceSplit {
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
        definition.default_config = json!({"terminators":"。！？!?"});
        definition.config_schema = json!({"type":"object","properties":{"terminators":{"type":"string"}},"required":["terminators"],"additionalProperties":false});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        terminators(config).map(|_| ())
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let terminators = terminators(config)?;
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
                    items: split(text, terminators)
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
