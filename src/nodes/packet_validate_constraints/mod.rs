//! 未採用の実験ノード。契約はexamples/nodes/packet_validate_constraints/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, RunContext,
    },
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub const KIND: &str = "experimental.packet-validate-constraints";
#[derive(Debug, Clone, Default)]
pub struct PacketValidateConstraints;
struct Settings {
    max_candidates: Option<usize>,
    max_text_length: Option<usize>,
    allowed_roots: BTreeSet<&'static str>,
}
fn optional_limit(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<usize>, CoreError> {
    match &object[key] {
        Value::Null => Ok(None),
        value => value
            .as_u64()
            .map(|limit| Some(limit as usize))
            .ok_or_else(|| {
                CoreError::new(
                    "invalid-config",
                    format!("{key} must be null or a non-negative integer"),
                )
            }),
    }
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 3
        || !object.contains_key("maxCandidates")
        || !object.contains_key("maxTextLength")
        || !object.contains_key("allowedRoots")
    {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let mut allowed_roots = BTreeSet::new();
    let roots = object["allowedRoots"]
        .as_array()
        .ok_or_else(|| CoreError::new("invalid-config", "allowedRoots must be an array"))?;
    for root in roots {
        let name = match root.as_str() {
            Some("text") => "text",
            Some("sequence") => "sequence",
            Some("choice") => "choice",
            _ => {
                return Err(CoreError::new(
                    "invalid-config",
                    "allowedRoots entries must be text, sequence or choice",
                ));
            }
        };
        if !allowed_roots.insert(name) {
            return Err(CoreError::new(
                "invalid-config",
                "allowedRoots must not contain duplicates",
            ));
        }
    }
    Ok(Settings {
        max_candidates: optional_limit(object, "maxCandidates")?,
        max_text_length: optional_limit(object, "maxTextLength")?,
        allowed_roots,
    })
}
fn satisfies(packet: &Packet, settings: &Settings) -> bool {
    let root_name = match &packet.root {
        Candidate::Text { .. } => "text",
        Candidate::Sequence { .. } => "sequence",
        Candidate::Choice { .. } => "choice",
    };
    if !settings.allowed_roots.is_empty() && !settings.allowed_roots.contains(root_name) {
        return false;
    }
    // 初版は根だけを検査する（候補や項目の中までは降りない）。
    match &packet.root {
        Candidate::Text { text, .. } => settings
            .max_text_length
            .is_none_or(|limit| text.chars().count() <= limit),
        Candidate::Choice { options } => settings
            .max_candidates
            .is_none_or(|limit| options.len() <= limit),
        Candidate::Sequence { .. } => true,
    }
}
impl Node for PacketValidateConstraints {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(Port::packet("in"))],
            vec![Port::packet("valid"), Port::packet("invalid")],
        );
        definition.default_config =
            json!({"maxCandidates":null,"maxTextLength":null,"allowedRoots":[]});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["maxCandidates","maxTextLength","allowedRoots"],"properties":{"maxCandidates":{"type":["integer","null"],"minimum":0},"maxTextLength":{"type":["integer","null"],"minimum":0},"allowedRoots":{"type":"array","items":{"enum":["text","sequence","choice"]},"uniqueItems":true}}});
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
        let (valid, invalid) = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                if satisfies(packet, &settings) {
                    (OutputValue::Data(packet.clone()), OutputValue::NoOutput)
                } else {
                    (OutputValue::NoOutput, OutputValue::Data(packet.clone()))
                }
            }
            Some(OutputValue::NoOutput) => (OutputValue::NoOutput, OutputValue::NoOutput),
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("valid".into(), valid), ("invalid".into(), invalid)].into())
    }
}
