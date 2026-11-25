//! 未採用の実験ノード。契約はexamples/nodes/flow_input_select/README.md。
use crate::node::{
    CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.flow-input-select";
#[derive(Debug, Clone, Copy, Default)]
pub struct FlowInputSelect;
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum Priority {
    AFirst,
    BFirst,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    priority: Priority,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("priority") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))
}
impl Node for FlowInputSelect {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![
                InputPort::optional(Port::packet("a")),
                InputPort::optional(Port::packet("b")),
            ],
            vec![Port::packet("out")],
        );
        definition.default_config = json!({"priority":"a-first"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["priority"],"properties":{"priority":{"type":"string","enum":["a-first","b-first"]}}});
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
        let order: [&str; 2] = match settings.priority {
            Priority::AFirst => ["a", "b"],
            Priority::BFirst => ["b", "a"],
        };
        for id in order {
            if let Some(OutputValue::Data(packet)) = inputs.get(id) {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                return Ok([("out".into(), OutputValue::Data(Arc::clone(packet)))].into());
            }
        }
        Ok([("out".into(), OutputValue::NoOutput)].into())
    }
}
