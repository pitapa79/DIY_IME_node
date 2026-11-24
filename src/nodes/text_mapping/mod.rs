//! 未採用の実験ノード。契約はexamples/nodes/text_mapping/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};
pub const KIND: &str = "experimental.text-mapping";
const MAX_ENTRIES: usize = 256;
const MAX_FROM_POINTS: usize = 64;
const MAX_TO_POINTS: usize = 64;
#[derive(Debug, Clone, Copy, Default)]
pub struct TextMapping;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    from: String,
    to: String,
}
#[derive(serde::Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Unknown {
    Preserve,
    Error,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    entries: Vec<Entry>,
    unknown: Unknown,
}
fn invalid(message: &str) -> CoreError {
    CoreError::new("invalid-config", message)
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| invalid("expected an object"))?;
    const KEYS: &[&str] = &["entries", "unknown"];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(
            "expected every declared setting and no unknown keys",
        ));
    }
    let settings: Settings =
        serde_json::from_value(config.clone()).map_err(|e| invalid(&e.to_string()))?;
    if settings.entries.len() > MAX_ENTRIES {
        return Err(invalid("entries exceeds 256"));
    }
    let mut seen = BTreeSet::new();
    for entry in &settings.entries {
        let from_points = entry.from.chars().count();
        if from_points == 0 || from_points > MAX_FROM_POINTS {
            return Err(invalid("from must contain 1 to 64 code points"));
        }
        if entry.to.chars().count() > MAX_TO_POINTS {
            return Err(invalid("to must contain at most 64 code points"));
        }
        if !seen.insert(entry.from.as_str()) {
            return Err(invalid("duplicate from values are not allowed"));
        }
    }
    Ok(settings)
}
fn transform(text: &str, settings: &Settings) -> Result<String, CoreError> {
    // Longest keys first so the longest match wins at every position.
    let mut entries: Vec<&Entry> = settings.entries.iter().collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.from.chars().count()));
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let matched = entries
            .iter()
            .find(|entry| rest.starts_with(entry.from.as_str()));
        match matched {
            Some(entry) => {
                output.push_str(&entry.to);
                rest = &rest[entry.from.len()..];
            }
            None => match settings.unknown {
                Unknown::Preserve => {
                    let ch = rest.chars().next().expect("rest is not empty");
                    output.push(ch);
                    rest = &rest[ch.len_utf8()..];
                }
                Unknown::Error => {
                    return Err(CoreError::new(
                        "unmapped-input",
                        "input contains text with no mapping entry",
                    ));
                }
            },
        }
    }
    Ok(output)
}
impl Node for TextMapping {
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
        definition.default_config = json!({"entries":[],"unknown":"preserve"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["entries","unknown"],"properties":{"entries":{"type":"array","maxItems":MAX_ENTRIES,"items":{"type":"object","additionalProperties":false,"required":["from","to"],"properties":{"from":{"type":"string"},"to":{"type":"string"}}}},"unknown":{"type":"string","enum":["preserve","error"]}}});
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
                let converted = transform(text, &settings)?;
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
