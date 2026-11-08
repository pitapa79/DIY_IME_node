//! 未採用のSequence生成ノード。入力の候補木を平坦化しません。
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
    tree::map_text,
};

pub const KIND: &str = "experimental.sequence-build";

#[derive(Debug, Clone, Copy, Default)]
pub struct SequenceBuild;

fn configured_packet(config: &Value) -> Result<Packet, CoreError> {
    let invalid = || CoreError::new("invalid-config", "expected exactly an items array");
    let object = config.as_object().ok_or_else(invalid)?;
    if object.len() != 1 {
        return Err(invalid());
    }
    let value = object.get("items").ok_or_else(invalid)?;
    let items = Vec::<Candidate>::deserialize(value)
        .map_err(|error| CoreError::new("invalid-config", error.to_string()))?;
    let packet = Packet::new(Candidate::Sequence { items });
    packet
        .validate()
        .map_err(|error| CoreError::new("invalid-config", error.to_string()))?;
    Ok(packet)
}

impl Node for SequenceBuild {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            ["a", "b"]
                .into_iter()
                .map(|id| InputPort::optional(Port::packet(id).with_language("ja")))
                .collect(),
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(RootConstraint::Sequence),
            ],
        );
        definition.default_config = json!({"items":[]});
        definition.config_schema = json!({
            "type":"object",
            "required":["items"],
            "additionalProperties":false,
            "properties":{"items":{"type":"array","items":{"type":"object"}}}
        });
        definition
    }

    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        configured_packet(config).map(|_| ())
    }

    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let mut packet = configured_packet(config)?;
        // コピー前に両入力を検査し、不正な入力から部分結果を作らない。
        for id in ["a", "b"] {
            if let Some(OutputValue::Data(input)) = inputs.get(id) {
                input
                    .validate()
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
            }
        }
        let Candidate::Sequence { items } = &mut packet.root else {
            unreachable!("configured_packet always constructs a Sequence")
        };
        for id in ["a", "b"] {
            if let Some(OutputValue::Data(input)) = inputs.get(id) {
                items.push(map_text(&input.root, |text, meta| Candidate::Text {
                    text: text.into(),
                    meta: meta.cloned(),
                }));
            }
        }
        Ok([("out".into(), OutputValue::Data(Arc::new(packet)))].into())
    }
}
