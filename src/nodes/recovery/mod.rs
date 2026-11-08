//! Synchronous, opt-in recovery. No implicit RuntimeInput or delayed selection.
use crate::{
    error_input::{
        ErrorAwareExecution, ErrorAwareInputs, ErrorAwareNode, ErrorAwareValue as V, RecoveryClaim,
    },
    node::{CoreError, InputPort, NodeDefinition, Port, RunContext},
};
use serde_json::{Value, json};

pub const KIND: &str = "text.recover-original";
pub struct RecoverOriginal;

fn fallback_on_no_output(config: &Value) -> Result<bool, CoreError> {
    config
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("onNoOutput"))
        .and_then(Value::as_bool)
        .ok_or_else(|| CoreError::new("invalid-config", "expected {onNoOutput: boolean}"))
}
fn output(value: V) -> ErrorAwareExecution {
    ErrorAwareExecution::new([("out".into(), value)].into())
}
impl ErrorAwareNode for RecoverOriginal {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            ["primary", "original"]
                .into_iter()
                .map(|id| InputPort::optional(Port::packet(id).with_language("ja")))
                .collect(),
            vec![Port::packet("out").with_language("ja")],
        );
        definition.default_config = json!({"onNoOutput":false});
        definition.config_schema = json!({"type":"object","properties":{"onNoOutput":{"type":"boolean"}},"required":["onNoOutput"],"additionalProperties":false});
        definition
    }
    fn error_inputs(&self) -> Vec<String> {
        vec!["primary".into(), "original".into()]
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        fallback_on_no_output(config).map(|_| ())
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &ErrorAwareInputs,
        config: &Value,
    ) -> Result<ErrorAwareExecution, CoreError> {
        let on_no_output = fallback_on_no_output(config)?;
        for id in ["primary", "original"] {
            if let Some(V::Data(packet)) = inputs.get(id) {
                packet
                    .validate()
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
            }
        }
        let absent = V::NoOutput;
        let primary = inputs.get("primary").unwrap_or(&absent);
        let original = inputs.get("original").unwrap_or(&absent);
        if let V::Data(packet) = primary {
            let mut result = output(V::Data(packet.clone()));
            result
                .dependencies
                .insert("out".into(), vec!["primary".into()]);
            return Ok(result);
        }
        if matches!(primary, V::NoOutput) && !on_no_output {
            return Ok(output(V::NoOutput));
        }
        match (primary, original) {
            (_, V::Data(packet)) => {
                let mut result = output(V::Data(packet.clone()));
                result
                    .dependencies
                    .insert("out".into(), vec!["original".into()]);
                if let V::Error(errors) = primary {
                    result.recoveries.push(RecoveryClaim {
                        input: "primary".into(),
                        output: "out".into(),
                        causes: errors
                            .causes()
                            .iter()
                            .map(|cause| cause.id.clone())
                            .collect(),
                    });
                }
                Ok(result)
            }
            (V::Error(primary), V::Error(original)) => {
                Ok(output(V::Error(primary.merge(original))))
            }
            (V::Error(errors), _) | (_, V::Error(errors)) => Ok(output(V::Error(errors.clone()))),
            _ => Ok(output(V::NoOutput)),
        }
    }
}
