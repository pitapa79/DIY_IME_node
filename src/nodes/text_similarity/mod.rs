//! 未採用の実験ノード。契約はexamples/nodes/text_similarity/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-similarity";
pub const MAX_CODE_POINTS: usize = 4096;
#[derive(Debug, Clone, Copy, Default)]
pub struct TextSimilarity;
fn settings(config: &Value) -> Result<(), CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if !object.is_empty() {
        return Err(CoreError::new(
            "invalid-config",
            "this node declares no settings",
        ));
    }
    Ok(())
}
/// コードポイント単位のLevenshtein距離（2行DP）。
fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
fn similarity(a: &str, b: &str) -> String {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let max = a_chars.len().max(b_chars.len());
    if max == 0 {
        return "1".to_string();
    }
    let distance = levenshtein(&a_chars, &b_chars);
    let value = 1.0 - distance as f64 / max as f64;
    let formatted = format!("{value:.6}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}
fn text_of(packet: &crate::Packet) -> Result<&str, CoreError> {
    packet
        .validate()
        .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
    let Candidate::Text { text, .. } = &packet.root else {
        return Err(CoreError::new("invalid-root", "expected Text"));
    };
    if text.chars().count() > MAX_CODE_POINTS {
        return Err(CoreError::new(
            "input-limit",
            "input exceeds 4096 code points",
        ));
    }
    Ok(text)
}
impl Node for TextSimilarity {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Text)
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
        definition.default_config = json!({});
        definition.config_schema = json!({"type":"object","additionalProperties":false});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        settings(config)
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        settings(config)?;
        let a = inputs
            .get("a")
            .ok_or_else(|| CoreError::new("missing-input", "required input: a"))?;
        let b = inputs
            .get("b")
            .ok_or_else(|| CoreError::new("missing-input", "required input: b"))?;
        let (OutputValue::Data(packet_a), OutputValue::Data(packet_b)) = (a, b) else {
            return Ok([("out".into(), OutputValue::NoOutput)].into());
        };
        let text_a = text_of(packet_a)?;
        let text_b = text_of(packet_b)?;
        Ok([(
            "out".into(),
            OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                text: similarity(text_a, text_b),
                meta: None,
            }))),
        )]
        .into())
    }
}
