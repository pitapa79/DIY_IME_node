//! 未採用の実験ノード。契約はexamples/nodes/text_regex_replace/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-regex-replace";
const MAX_PATTERN_BYTES: usize = 1024;
const MAX_REPLACEMENT_BYTES: usize = 1024;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const COMPILED_SIZE_LIMIT: usize = 1024 * 1024;
#[derive(Debug, Clone, Copy, Default)]
pub struct TextRegexReplace;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    pattern: String,
    replacement: String,
    mode: Mode,
}
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Mode {
    All,
    First,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    const KEYS: &[&str] = &["pattern", "replacement", "mode"];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;
    if settings.pattern.len() > MAX_PATTERN_BYTES {
        return Err(CoreError::new(
            "invalid-config",
            "pattern exceeds 1024 UTF-8 bytes",
        ));
    }
    if settings.replacement.len() > MAX_REPLACEMENT_BYTES {
        return Err(CoreError::new(
            "invalid-config",
            "replacement exceeds 1024 UTF-8 bytes",
        ));
    }
    Ok(settings)
}
fn compile(settings: &Settings) -> Result<Regex, CoreError> {
    regex::RegexBuilder::new(&settings.pattern)
        .size_limit(COMPILED_SIZE_LIMIT)
        .dfa_size_limit(COMPILED_SIZE_LIMIT)
        .build()
        .map_err(|e| CoreError::new("invalid-pattern", e.to_string()))
}
fn transform(text: &str, settings: &Settings, regex: &Regex) -> Result<String, CoreError> {
    let converted = match settings.mode {
        Mode::All => regex.replace_all(text, settings.replacement.as_str()),
        Mode::First => regex.replace(text, settings.replacement.as_str()),
    };
    if converted.len() > MAX_OUTPUT_BYTES {
        return Err(CoreError::new(
            "output-limit",
            "replacement result exceeds 1048576 bytes",
        ));
    }
    Ok(converted.into_owned())
}
impl Node for TextRegexReplace {
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
        definition.default_config = json!({"pattern":"","replacement":"","mode":"all"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["pattern","replacement","mode"],"properties":{"pattern":{"type":"string"},"replacement":{"type":"string"},"mode":{"type":"string","enum":["all","first"]}}});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        let settings = settings(config)?;
        compile(&settings).map(|_| ())
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let settings = settings(config)?;
        let regex = compile(&settings)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                let converted = transform(text, &settings, &regex)?;
                if converted == *text {
                    OutputValue::Data(Arc::clone(packet))
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
