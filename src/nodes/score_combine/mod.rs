//! 未採用の実験ノード。契約はexamples/nodes/score_combine/README.md。
use crate::{
    Candidate, ChoiceOption, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.score-combine";
#[derive(Debug, Clone, Copy, Default)]
pub struct ScoreCombine;
#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Missing {
    Zero,
    Intersect,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
    weight_a: f64,
    weight_b: f64,
    missing: Missing,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    const KEYS: &[&str] = &["weightA", "weightB", "missing"];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;
    if !settings.weight_a.is_finite() || !settings.weight_b.is_finite() {
        return Err(CoreError::new(
            "invalid-config",
            "weights must be finite numbers",
        ));
    }
    Ok(settings)
}
/// 候補（meta を含む完全な値）ごとに score を合計する。None は 0 として数える。
/// 並びは最初の出現順を保つ。
fn aggregate(options: &[ChoiceOption]) -> Vec<(Candidate, f64)> {
    let mut totals: Vec<(Candidate, f64)> = Vec::new();
    for option in options {
        let score = option.score.unwrap_or(0.0);
        if let Some((_, total)) = totals.iter_mut().find(|(value, _)| *value == option.value) {
            *total += score;
        } else {
            totals.push((option.value.clone(), score));
        }
    }
    totals
}
fn combine(
    a: &[ChoiceOption],
    b: &[ChoiceOption],
    settings: &Settings,
) -> Result<Vec<ChoiceOption>, CoreError> {
    let totals_a = aggregate(a);
    let totals_b = aggregate(b);
    let mut combined: Vec<ChoiceOption> = Vec::new();
    for (value, sum_a) in &totals_a {
        let from_b = totals_b
            .iter()
            .find(|(other, _)| other == value)
            .map(|(_, sum_b)| *sum_b);
        let total = match (settings.missing, from_b) {
            (Missing::Intersect, None) => continue,
            (_, from_b) => settings.weight_a * sum_a + settings.weight_b * from_b.unwrap_or(0.0),
        };
        if !total.is_finite() {
            return Err(CoreError::new(
                "non-finite-score",
                "combined score is not finite",
            ));
        }
        combined.push(ChoiceOption {
            value: value.clone(),
            score: Some(total),
        });
    }
    if matches!(settings.missing, Missing::Zero) {
        for (value, sum_b) in &totals_b {
            if totals_a.iter().any(|(other, _)| other == value) {
                continue;
            }
            let total = settings.weight_b * sum_b;
            if !total.is_finite() {
                return Err(CoreError::new(
                    "non-finite-score",
                    "combined score is not finite",
                ));
            }
            combined.push(ChoiceOption {
                value: value.clone(),
                score: Some(total),
            });
        }
    }
    Ok(combined)
}
impl Node for ScoreCombine {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Choice)
        };
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![
                InputPort::required(port("a")),
                InputPort::required(port("b")),
            ],
            vec![port("out")],
        );
        definition.default_config = json!({"weightA":1.0,"weightB":1.0,"missing":"zero"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["weightA","weightB","missing"],"properties":{"weightA":{"type":"number"},"weightB":{"type":"number"},"missing":{"type":"string","enum":["zero","intersect"]}}});
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
        let a = inputs
            .get("a")
            .ok_or_else(|| CoreError::new("missing-input", "required input: a"))?;
        let b = inputs
            .get("b")
            .ok_or_else(|| CoreError::new("missing-input", "required input: b"))?;
        let (OutputValue::Data(packet_a), OutputValue::Data(packet_b)) = (a, b) else {
            return Ok([("out".into(), OutputValue::NoOutput)].into());
        };
        packet_a
            .validate()
            .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
        packet_b
            .validate()
            .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
        let Candidate::Choice { options: options_a } = &packet_a.root else {
            return Err(CoreError::new("invalid-root", "expected Choice"));
        };
        let Candidate::Choice { options: options_b } = &packet_b.root else {
            return Err(CoreError::new("invalid-root", "expected Choice"));
        };
        let options = combine(options_a, options_b, &settings)?;
        Ok([(
            "out".into(),
            OutputValue::Data(Arc::new(Packet::new(Candidate::Choice { options }))),
        )]
        .into())
    }
}
