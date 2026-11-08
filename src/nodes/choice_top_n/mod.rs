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
pub const KIND: &str = "experimental.choice-top-n";
#[derive(Debug, Clone, Copy, Default)]
pub struct ChoiceTopN;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    count: usize,
    unscored: f64,
}

fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected object"))?;
    const KEYS: &[&str] = &["count", "unscored"];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(CoreError::new(
            "invalid-config",
            "expected complete settings and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;
    if !settings.unscored.is_finite() {
        return Err(CoreError::new("invalid-config", "unscored must be finite"));
    }
    Ok(settings)
}
fn copy_candidate(root: &Candidate) -> Candidate {
    crate::tree::map_text(root, |text, meta| Candidate::Text {
        text: text.to_owned(),
        meta: meta.cloned(),
    })
}
fn copy_option(option: &crate::ChoiceOption) -> crate::ChoiceOption {
    crate::ChoiceOption {
        value: copy_candidate(&option.value),
        score: option.score,
    }
}

fn transform(root: &Candidate, settings: &Settings) -> Result<Option<Candidate>, CoreError> {
    let _ = settings;
    let Candidate::Choice { options } = root else {
        return Err(CoreError::new("invalid-root", "expected Choice"));
    };
    let mut sorted: Vec<_> = options.iter().collect();
    sorted.sort_by(|a, b| {
        b.score
            .unwrap_or(settings.unscored)
            .partial_cmp(&a.score.unwrap_or(settings.unscored))
            .unwrap()
    });
    Ok(Some(Candidate::Choice {
        options: sorted
            .into_iter()
            .take(settings.count)
            .map(copy_option)
            .collect(),
    }))
}
impl Node for ChoiceTopN {
    fn definition(&self) -> NodeDefinition {
        let input = Port::packet("in")
            .with_language("ja")
            .with_root(RootConstraint::Choice);
        let output = Port::packet("out")
            .with_language("ja")
            .with_root(RootConstraint::Choice);
        let mut definition =
            NodeDefinition::new(KIND, 1, vec![InputPort::required(input)], vec![output]);
        definition.default_config = json!({"count":5,"unscored":0});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["count","unscored"],"properties":{"count":{"type":"integer"},"unscored":{"type":"number"}}});
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
