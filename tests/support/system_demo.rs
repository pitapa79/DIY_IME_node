#![allow(dead_code)]
//! 初期 E2E ノードと旧デモの互換実装。標準ノードの最終 API ではありません。
use std::sync::Arc;

use serde_json::{Value, json};

use diy_ime2_nodes::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext,
    },
    tree::map_text,
};

pub(super) fn config_keys<'a>(
    config: &'a Value,
    keys: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(CoreError::new(
            "invalid-config",
            format!("expected exactly these settings: {keys:?}"),
        ));
    }
    Ok(object)
}

pub(super) fn output(value: OutputValue) -> Outputs {
    [("out".into(), value)].into()
}
fn unary(kind: &str, root: RootConstraint) -> NodeDefinition {
    NodeDefinition::new(
        kind,
        1,
        vec![InputPort::required(Port::packet("in"))],
        vec![Port::packet("out").with_root(root)],
    )
}

pub struct PacketSource;
impl Node for PacketSource {
    fn definition(&self) -> NodeDefinition {
        let mut definition =
            NodeDefinition::new("demo.packet", 1, vec![], vec![Port::packet("out")]);
        definition.default_config =
            json!({"packet":{"version":1,"root":{"type":"text","text":""}}});
        definition.config_schema = json!({"type":"object","required":["packet"],"additionalProperties":false,"properties":{"packet":{"type":"object"}}});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        let object = config_keys(config, &["packet"])?;
        Packet::from_json(&object["packet"].to_string())
            .map_err(|error| CoreError::new("invalid-config", error.to_string()))?;
        Ok(())
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, config: &Value) -> Result<Outputs, CoreError> {
        let packet = Packet::from_json(&config["packet"].to_string())
            .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
        Ok(output(OutputValue::Data(Arc::new(packet))))
    }
}
