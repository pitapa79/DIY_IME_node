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
pub const KIND: &str = "experimental.score-normalize";
#[derive(Debug, Clone, Copy, Default)]
pub struct ScoreNormalize;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    method: Method,
    unscored: f64,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Method {
    Softmax,
    Minmax,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected object"))?;
    const KEYS: &[&str] = &["method", "unscored"];
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
    if options.is_empty() {
        return Ok(Some(Candidate::Choice { options: vec![] }));
    }
    let values: Vec<_> = options
        .iter()
        .map(|o| o.score.unwrap_or(settings.unscored))
        .collect();
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let normalized: Vec<f64> = match settings.method {
        Method::Softmax => {
            let exp: Vec<_> = values.iter().map(|v| (v - max).exp()).collect();
            let sum: f64 = exp.iter().sum();
            exp.iter().map(|v| v / sum).collect()
        }
        Method::Minmax => {
            if min == max {
                vec![0.0; values.len()]
            } else {
                let range = max - min;
                if range.is_finite() {
                    values.iter().map(|value| (value - min) / range).collect()
                } else {
                    let scale = min.abs().max(max.abs());
                    let denominator = max / scale - min / scale;
                    values
                        .iter()
                        .map(|value| (value / scale - min / scale) / denominator)
                        .collect()
                }
            }
        }
    };
    Ok(Some(Candidate::Choice {
        options: options
            .iter()
            .zip(normalized)
            .map(|(o, score)| {
                let mut o = copy_option(o);
                o.score = Some(score);
                o
            })
            .collect(),
    }))
}
impl Node for ScoreNormalize {
    fn definition(&self) -> NodeDefinition {
        let input = Port::packet("in")
            .with_language("ja")
            .with_root(RootConstraint::Choice);
        let output = Port::packet("out")
            .with_language("ja")
            .with_root(RootConstraint::Choice);
        let mut definition =
            NodeDefinition::new(KIND, 1, vec![InputPort::required(input)], vec![output]);
        definition.default_config = json!({"method":"softmax","unscored":0});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["method","unscored"],"properties":{"method":{"type":"string"},"unscored":{"type":"number"}}});
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
