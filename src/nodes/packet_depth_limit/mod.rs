//! 独立した未採用の候補木操作ノード。契約は専用examplesを参照。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.packet-depth-limit";
#[derive(Debug, Clone, Copy, Default)]
pub struct PacketDepthLimit;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    #[serde(rename = "maxDepth")]
    max_depth: usize,
    exceeded: Exceeded,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Exceeded {
    Error,
    NoOutput,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected object"))?;
    const KEYS: &[&str] = &["maxDepth", "exceeded"];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(CoreError::new(
            "invalid-config",
            "expected complete settings and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;
    if settings.max_depth == 0 {
        return Err(CoreError::new(
            "invalid-config",
            "maxDepth must be positive",
        ));
    }
    Ok(settings)
}
fn copy_candidate(root: &Candidate) -> Candidate {
    crate::tree::map_text(root, |text, meta| Candidate::Text {
        text: text.to_owned(),
        meta: meta.cloned(),
    })
}

fn transform(root: &Candidate, settings: &Settings) -> Result<Option<Candidate>, CoreError> {
    let _ = settings;
    let mut pending = vec![(root, 1_usize)];
    while let Some((candidate, depth)) = pending.pop() {
        if depth > settings.max_depth {
            return match settings.exceeded {
                Exceeded::Error => Err(CoreError::new("depth-limit", "configured depth exceeded")),
                Exceeded::NoOutput => Ok(None),
            };
        }
        let next = depth
            .checked_add(1)
            .ok_or_else(|| CoreError::new("depth-limit", "depth overflow"))?;
        match candidate {
            Candidate::Text { .. } => {}
            Candidate::Sequence { items } => {
                pending.extend(items.iter().map(|child| (child, next)))
            }
            Candidate::Choice { options } => {
                pending.extend(options.iter().map(|o| (&o.value, next)))
            }
        }
    }
    Ok(Some(copy_candidate(root)))
}
impl Node for PacketDepthLimit {
    fn definition(&self) -> NodeDefinition {
        let input = Port::packet("in")
            .with_language("ja")
            .with_root(RootConstraint::Any);
        let output = Port::packet("out")
            .with_language("ja")
            .with_root(RootConstraint::Any);
        let mut definition =
            NodeDefinition::new(KIND, 1, vec![InputPort::required(input)], vec![output]);
        definition.default_config = json!({"maxDepth":32,"exceeded":"error"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["maxDepth","exceeded"],"properties":{"maxDepth":{"type":"integer"},"exceeded":{"type":"string"}}});
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
                match transform(&packet.root, &settings)? {
                    Some(root) => OutputValue::Data(Arc::new(Packet::new(root))),
                    None => OutputValue::NoOutput,
                }
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
