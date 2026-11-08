//! 独立した未採用の候補木操作ノード。契約は専用examplesを参照。
use crate::node::{
    CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.packet-passthrough";
#[derive(Debug, Clone, Copy, Default)]
pub struct PacketPassthrough;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {}

fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected object"))?;
    const KEYS: &[&str] = &[];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(CoreError::new(
            "invalid-config",
            "expected complete settings and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;

    Ok(settings)
}

impl Node for PacketPassthrough {
    fn definition(&self) -> NodeDefinition {
        let input = Port::packet("in")
            .with_language("ja")
            .with_root(RootConstraint::Any);
        let output = Port::packet("out")
            .with_language("ja")
            .with_root(RootConstraint::Any);
        let mut definition =
            NodeDefinition::new(KIND, 1, vec![InputPort::required(input)], vec![output]);
        definition.default_config = json!({});
        definition.config_schema =
            json!({"type":"object","additionalProperties":false,"required":[],"properties":{}});
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
        let _ = settings;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                OutputValue::Data(Arc::clone(packet))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
