//! 未採用の実験ノード。契約はexamples/nodes/packet_compare/README.md。
use crate::{
    Candidate,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.packet-compare";
#[derive(Debug, Clone, Copy, Default)]
pub struct PacketCompare;
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Exact,
    Content,
    Structure,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    mode: Mode,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("mode") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))
}
/// meta をすべて無視した内容比較。Choice の score は内容の一部として比較する。
fn content_eq(a: &Candidate, b: &Candidate) -> bool {
    match (a, b) {
        (Candidate::Text { text: x, .. }, Candidate::Text { text: y, .. }) => x == y,
        (Candidate::Sequence { items: x }, Candidate::Sequence { items: y }) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(u, v)| content_eq(u, v))
        }
        (Candidate::Choice { options: x }, Candidate::Choice { options: y }) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(u, v)| u.score == v.score && content_eq(&u.value, &v.value))
        }
        _ => false,
    }
}
/// 形だけの比較: 根の種類、Sequence の長さ、Choice の候補数を再帰的に見る。
fn structure_eq(a: &Candidate, b: &Candidate) -> bool {
    match (a, b) {
        (Candidate::Text { .. }, Candidate::Text { .. }) => true,
        (Candidate::Sequence { items: x }, Candidate::Sequence { items: y }) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(u, v)| structure_eq(u, v))
        }
        (Candidate::Choice { options: x }, Candidate::Choice { options: y }) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(u, v)| structure_eq(&u.value, &v.value))
        }
        _ => false,
    }
}
impl Node for PacketCompare {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![
                InputPort::required(Port::packet("a")),
                InputPort::required(Port::packet("b")),
            ],
            vec![Port::packet("matched"), Port::packet("unmatched")],
        );
        definition.default_config = json!({"mode":"exact"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["mode"],"properties":{"mode":{"type":"string","enum":["exact","content","structure"]}}});
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
        let (OutputValue::Data(packet_a), OutputValue::Data(packet_b)) = (a, b) else {
            return Ok([
                ("matched".into(), OutputValue::NoOutput),
                ("unmatched".into(), OutputValue::NoOutput),
            ]
            .into());
        };
        packet_a
            .validate()
            .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
        packet_b
            .validate()
            .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
        let matched = match settings.mode {
            Mode::Exact => packet_a == packet_b,
            Mode::Content => content_eq(&packet_a.root, &packet_b.root),
            Mode::Structure => structure_eq(&packet_a.root, &packet_b.root),
        };
        let routed = OutputValue::Data(Arc::clone(packet_a));
        let (matched_out, unmatched_out) = if matched {
            (routed, OutputValue::NoOutput)
        } else {
            (OutputValue::NoOutput, routed)
        };
        Ok([
            ("matched".into(), matched_out),
            ("unmatched".into(), unmatched_out),
        ]
        .into())
    }
}
