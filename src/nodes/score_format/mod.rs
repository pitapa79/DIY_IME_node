//! 未採用の実験ノード。契約はexamples/nodes/score_format/README.md。
use crate::{
    Candidate, Packet, TextMeta,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.score-format";
#[derive(Debug, Clone, Copy, Default)]
pub struct ScoreFormat;
struct Settings {
    decimals: usize,
    percent: bool,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 2 || !object.contains_key("decimals") || !object.contains_key("percent") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let decimals = object["decimals"]
        .as_u64()
        .filter(|value| *value <= 6)
        .ok_or_else(|| {
            CoreError::new(
                "invalid-config",
                "decimals must be an integer between 0 and 6",
            )
        })? as usize;
    let percent = object["percent"]
        .as_bool()
        .ok_or_else(|| CoreError::new("invalid-config", "percent must be a boolean"))?;
    Ok(Settings { decimals, percent })
}
fn format_score(score: Option<f64>, settings: &Settings) -> Result<String, CoreError> {
    let Some(score) = score else {
        return Ok(String::new());
    };
    if !score.is_finite() {
        return Err(CoreError::new(
            "non-finite-score",
            "score must be finite to format",
        ));
    }
    if settings.percent {
        Ok(format!(
            "{:.prec$}%",
            score * 100.0,
            prec = settings.decimals
        ))
    } else {
        Ok(format!("{:.prec$}", score, prec = settings.decimals))
    }
}
impl Node for ScoreFormat {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(
                Port::packet("in")
                    .with_language("ja")
                    .with_root(RootConstraint::Choice),
            )],
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(RootConstraint::Sequence),
            ],
        );
        definition.default_config = json!({"decimals":2,"percent":false});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["decimals","percent"],"properties":{"decimals":{"type":"integer","minimum":0,"maximum":6},"percent":{"type":"boolean"}}});
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
                let Candidate::Choice { options } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Choice"));
                };
                let mut items = Vec::with_capacity(options.len());
                for option in options {
                    let reading = match &option.value {
                        Candidate::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    };
                    items.push(Candidate::Text {
                        text: format_score(option.score, &settings)?,
                        meta: reading.map(|reading| TextMeta {
                            reading: Some(reading),
                            source_span: None,
                        }),
                    });
                }
                OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
