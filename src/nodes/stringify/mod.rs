//! 選択済みText / Sequenceを文字列へまとめる実用ノード。
//! Choiceの評価・選択は行わず、未選択の候補があれば失敗する。
use std::sync::Arc;

use serde_json::Value;

use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};

pub const KIND: &str = "text.stringify";

/// 入力Packetを検証し、Textを順序どおり連結する。入力やmetaは変更しない。
pub fn stringify(packet: &Packet) -> Result<String, CoreError> {
    packet
        .validate()
        .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
    let mut pending = vec![&packet.root];
    let mut text = String::new();
    while let Some(candidate) = pending.pop() {
        match candidate {
            Candidate::Text { text: part, .. } => text.push_str(part),
            Candidate::Sequence { items } => pending.extend(items.iter().rev()),
            Candidate::Choice { .. } => {
                return Err(CoreError::new(
                    "unresolved-choice",
                    "select candidates before stringifying",
                ));
            }
        }
    }
    Ok(text)
}

/// `text.stringify@1`。設定は空オブジェクトだけを許可する。
pub struct Stringify;

impl Node for Stringify {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(Port::packet("in").with_language("ja"))],
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(RootConstraint::Text),
            ],
        )
    }

    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        self.validate_config(config)?;
        let value = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: stringify(packet)?,
                    meta: None,
                })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "in is required")),
        };
        Ok([("out".into(), value)].into())
    }
}
