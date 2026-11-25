//! 未採用の実験ノード。契約はexamples/nodes/text_symbol_notation/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-symbol-notation";
#[derive(Debug, Clone, Copy, Default)]
pub struct TextSymbolNotation;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    preset: String,
}
fn settings(config: &Value) -> Result<bool, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("preset") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;
    match settings.preset.as_str() {
        "japanese" => Ok(true),
        "ascii" => Ok(false),
        _ => Err(CoreError::new(
            "invalid-config",
            "preset must be japanese or ascii",
        )),
    }
}
fn to_japanese(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut double_open = true;
    let mut single_open = true;
    for ch in text.chars() {
        match ch {
            '(' => out.push('（'),
            ')' => out.push('）'),
            '[' => out.push('［'),
            ']' => out.push('］'),
            '{' => out.push('｛'),
            '}' => out.push('｝'),
            '"' => {
                out.push(if double_open { '「' } else { '」' });
                double_open = !double_open;
            }
            '\'' => {
                out.push(if single_open { '『' } else { '』' });
                single_open = !single_open;
            }
            _ => out.push(ch),
        }
    }
    out
}
fn to_ascii(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '（' => '(',
            '）' => ')',
            '［' => '[',
            '］' => ']',
            '｛' => '{',
            '｝' => '}',
            '「' | '」' => '"',
            '『' | '』' => '\'',
            _ => ch,
        })
        .collect()
}
impl Node for TextSymbolNotation {
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
        definition.default_config = json!({"preset":"japanese"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["preset"],"properties":{"preset":{"enum":["japanese","ascii"]}}});
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
        let japanese = settings(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                let converted = if japanese {
                    to_japanese(text)
                } else {
                    to_ascii(text)
                };
                if converted == *text {
                    OutputValue::Data(packet.clone())
                } else {
                    OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                        text: converted,
                        meta: None,
                    })))
                }
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
