//! 未採用の実験ノード。契約はexamples/nodes/choice_random_select/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.choice-random-select";
#[derive(Debug, Clone, Copy, Default)]
pub struct ChoiceRandomSelect;
fn settings(config: &Value) -> Result<u64, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("seed") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    object["seed"]
        .as_u64()
        .ok_or_else(|| CoreError::new("invalid-config", "seed must be an unsigned 64-bit integer"))
}
/// SplitMix64。OS乱数・時刻・内部状態は使わず、seedだけで決定的に選ぶ。
fn next_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}
/// Lemire法による一様選択（剰余の偏りを避ける）。
fn pick(seed: u64, count: usize) -> usize {
    debug_assert!(count > 0);
    let n = count as u64;
    let mut state = seed;
    loop {
        let x = next_u64(&mut state);
        let m = (x as u128) * (n as u128);
        let low = m as u64;
        if low < n {
            let threshold = n.wrapping_neg() % n;
            if low < threshold {
                continue;
            }
        }
        return (m >> 64) as usize;
    }
}
impl Node for ChoiceRandomSelect {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(
                Port::packet("in")
                    .with_language("ja")
                    .with_root(RootConstraint::Choice),
            )],
            // 選ばれた候補そのものが根になるため、出力に根制約は付けない。
            vec![Port::packet("out")],
        );
        definition.default_config = json!({"seed":0});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["seed"],"properties":{"seed":{"type":"integer","minimum":0}}});
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
        let seed = settings(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Choice { options } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Choice"));
                };
                if options.is_empty() {
                    OutputValue::NoOutput
                } else {
                    let chosen = &options[pick(seed, options.len())];
                    OutputValue::Data(Arc::new(Packet::new(chosen.value.clone())))
                }
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
