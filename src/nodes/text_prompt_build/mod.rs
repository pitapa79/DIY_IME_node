//! 未採用の実験ノード。契約はexamples/nodes/text_prompt_build/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-prompt-build";
const MAX_INSTRUCTION_BYTES: usize = 4096;
const MAX_FENCE_BYTES: usize = 256;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
#[derive(Debug, Clone, Copy, Default)]
pub struct TextPromptBuild;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    instruction: String,
    before: String,
    after: String,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    const KEYS: &[&str] = &["instruction", "before", "after"];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;
    if settings.instruction.len() > MAX_INSTRUCTION_BYTES {
        return Err(CoreError::new(
            "invalid-config",
            "instruction exceeds 4096 UTF-8 bytes",
        ));
    }
    if settings.before.len() > MAX_FENCE_BYTES || settings.after.len() > MAX_FENCE_BYTES {
        return Err(CoreError::new(
            "invalid-config",
            "before/after exceed 256 UTF-8 bytes",
        ));
    }
    Ok(settings)
}
impl Node for TextPromptBuild {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Text)
        };
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(port("in"))],
            vec![port("out")],
        );
        definition.default_config = json!({"instruction":"","before":"\n\n","after":""});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["instruction","before","after"],"properties":{"instruction":{"type":"string","maxLength":4096},"before":{"type":"string","maxLength":256},"after":{"type":"string","maxLength":256}}});
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
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                let mut built = String::with_capacity(
                    settings.instruction.len()
                        + settings.before.len()
                        + text.len()
                        + settings.after.len(),
                );
                built.push_str(&settings.instruction);
                built.push_str(&settings.before);
                built.push_str(text);
                built.push_str(&settings.after);
                if built.len() > MAX_OUTPUT_BYTES {
                    return Err(CoreError::new("output-limit", "built text exceeds 1 MiB"));
                }
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: built,
                    meta: None,
                })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
