//! 未採用の実験ノード。契約はexamples/nodes/text_regex_search/README.md。
use crate::{
    Candidate, Packet, TextMeta,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-regex-search";
const MAX_PATTERN_BYTES: usize = 1024;
const COMPILED_SIZE_LIMIT: usize = 1024 * 1024;
pub const MAX_LIMIT: usize = 1024;
const DEFAULT_LIMIT: usize = 64;
#[derive(Debug, Clone, Copy, Default)]
pub struct TextRegexSearch;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    pattern: String,
    limit: usize,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    const KEYS: &[&str] = &["pattern", "limit"];
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
    if !(1..=MAX_LIMIT).contains(&settings.limit) {
        return Err(CoreError::new(
            "invalid-config",
            "limit must be an integer from 1 to 1024",
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
fn search(text: &str, regex: &Regex, limit: usize) -> Packet {
    let mut items = Vec::new();
    let mut counted_bytes = 0usize;
    let mut counted_points = 0usize;
    for matched in regex.find_iter(text).take(limit) {
        counted_points += text[counted_bytes..matched.start()].chars().count();
        let start = counted_points;
        counted_points += matched.as_str().chars().count();
        counted_bytes = matched.end();
        items.push(Candidate::Text {
            text: matched.as_str().into(),
            meta: Some(TextMeta {
                reading: None,
                source_span: Some([start, counted_points]),
            }),
        });
    }
    Packet::new(Candidate::Sequence { items })
}
impl Node for TextRegexSearch {
    fn definition(&self) -> NodeDefinition {
        let input = Port::packet("in")
            .with_language("ja")
            .with_root(RootConstraint::Text);
        let output = Port::packet("out")
            .with_language("ja")
            .with_root(RootConstraint::Sequence);
        let mut definition =
            NodeDefinition::new(KIND, 1, vec![InputPort::required(input)], vec![output]);
        definition.default_config = json!({"pattern":"","limit":DEFAULT_LIMIT});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["pattern","limit"],"properties":{"pattern":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":MAX_LIMIT}}});
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
                OutputValue::Data(Arc::new(search(text, &regex, settings.limit)))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
