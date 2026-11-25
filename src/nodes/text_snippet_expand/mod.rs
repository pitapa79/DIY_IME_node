//! 未採用の実験ノード。契約はexamples/nodes/text_snippet_expand/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-snippet-expand";
const MAX_TEMPLATE_BYTES: usize = 4096;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const PORTS: [&str; 4] = ["a", "b", "c", "d"];
#[derive(Debug, Clone, Copy, Default)]
pub struct TextSnippetExpand;
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Literal(String),
    Placeholder(usize),
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    template: String,
}
fn parse_template(template: &str) -> Result<Vec<Token>, CoreError> {
    if template.len() > MAX_TEMPLATE_BYTES {
        return Err(CoreError::new(
            "invalid-config",
            "template exceeds 4096 UTF-8 bytes",
        ));
    }
    let chars: Vec<char> = template.chars().collect();
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '{' if chars.get(index + 1) == Some(&'{') => {
                literal.push('{');
                index += 2;
            }
            '{' => {
                let name = chars.get(index + 1).copied();
                let close = chars.get(index + 2).copied();
                match (name, close) {
                    (Some(name), Some('}')) if PORTS.iter().any(|port| port.starts_with(name)) => {
                        if !literal.is_empty() {
                            tokens.push(Token::Literal(std::mem::take(&mut literal)));
                        }
                        let port_index = PORTS
                            .iter()
                            .position(|port| port.starts_with(name))
                            .unwrap();
                        tokens.push(Token::Placeholder(port_index));
                        index += 3;
                    }
                    _ => {
                        return Err(CoreError::new(
                            "invalid-config",
                            "template has an undeclared or unterminated placeholder",
                        ));
                    }
                }
            }
            '}' if chars.get(index + 1) == Some(&'}') => {
                literal.push('}');
                index += 2;
            }
            '}' => {
                return Err(CoreError::new(
                    "invalid-config",
                    "template has an unmatched closing brace",
                ));
            }
            c => {
                literal.push(c);
                index += 1;
            }
        }
    }
    if !literal.is_empty() {
        tokens.push(Token::Literal(literal));
    }
    Ok(tokens)
}
fn settings(config: &Value) -> Result<(Settings, Vec<Token>), CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("template") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;
    let tokens = parse_template(&settings.template)?;
    Ok((settings, tokens))
}
impl Node for TextSnippetExpand {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Text)
        };
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            PORTS.map(|id| InputPort::optional(port(id))).to_vec(),
            vec![port("out")],
        );
        definition.default_config = json!({"template":"{a}"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["template"],"properties":{"template":{"type":"string","maxLength":4096}}});
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
        let (_, tokens) = settings(config)?;
        let mut values: [Option<String>; 4] = Default::default();
        for token in &tokens {
            let Token::Placeholder(port_index) = token else {
                continue;
            };
            if values[*port_index].is_some() {
                continue;
            }
            let id = PORTS[*port_index];
            // Only ports the template uses are consulted. Unused ports are
            // ignored in every state (absent, NoOutput, or Data).
            let packet = match inputs.get(id) {
                None => {
                    return Err(CoreError::new(
                        "missing-input",
                        "template placeholder requires input: a/b/c/d",
                    ));
                }
                Some(OutputValue::NoOutput) => {
                    return Ok([("out".into(), OutputValue::NoOutput)].into());
                }
                Some(OutputValue::Data(packet)) => packet,
            };
            packet
                .validate()
                .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
            let Candidate::Text { text, .. } = &packet.root else {
                return Err(CoreError::new("invalid-root", "expected Text"));
            };
            values[*port_index] = Some(text.clone());
        }
        let mut output = String::new();
        for token in &tokens {
            match token {
                Token::Literal(text) => output.push_str(text),
                Token::Placeholder(port_index) => {
                    output.push_str(values[*port_index].as_deref().unwrap_or_default())
                }
            }
            if output.len() > MAX_OUTPUT_BYTES {
                return Err(CoreError::new(
                    "output-limit",
                    "expanded text exceeds 1 MiB",
                ));
            }
        }
        Ok([(
            "out".into(),
            OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                text: output,
                meta: None,
            }))),
        )]
        .into())
    }
}
