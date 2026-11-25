//! 未採用の実験ノード。契約はexamples/nodes/flow_branch/README.md。
use crate::{
    Candidate,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
pub const KIND: &str = "experimental.flow-branch";
#[derive(Debug, Clone, Copy, Default)]
pub struct FlowBranch;
fn settings(config: &Value) -> Result<(), CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if !object.is_empty() {
        return Err(CoreError::new(
            "invalid-config",
            "this node declares no settings",
        ));
    }
    Ok(())
}
impl Node for FlowBranch {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![
                InputPort::required(Port::packet("data")),
                InputPort::required(Port::packet("selector").with_root(RootConstraint::Text)),
            ],
            vec![Port::packet("a"), Port::packet("b")],
        );
        definition.default_config = json!({});
        definition.config_schema = json!({"type":"object","additionalProperties":false});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        settings(config)
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        settings(config)?;
        let data = inputs
            .get("data")
            .ok_or_else(|| CoreError::new("missing-input", "required input: data"))?;
        let selector = inputs
            .get("selector")
            .ok_or_else(|| CoreError::new("missing-input", "required input: selector"))?;
        let (OutputValue::Data(data_packet), OutputValue::Data(selector_packet)) = (data, selector)
        else {
            return Ok([
                ("a".into(), OutputValue::NoOutput),
                ("b".into(), OutputValue::NoOutput),
            ]
            .into());
        };
        data_packet
            .validate()
            .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
        selector_packet
            .validate()
            .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
        let Candidate::Text { text, .. } = &selector_packet.root else {
            return Err(CoreError::new("invalid-root", "selector must be Text"));
        };
        let routed = OutputValue::Data(data_packet.clone());
        let (a, b) = match text.as_str() {
            "a" => (routed, OutputValue::NoOutput),
            "b" => (OutputValue::NoOutput, routed),
            _ => {
                return Err(CoreError::new(
                    "invalid-selector",
                    "selector must be a or b",
                ));
            }
        };
        Ok([("a".into(), a), ("b".into(), b)].into())
    }
}
