//! 未採用の実験ノード。契約はexamples/nodes/packet_test_data/README.md。
use crate::{
    Candidate, ChoiceOption, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.packet-test-data";
const ALPHABET: &[char] = &[
    'あ', 'い', 'う', 'え', 'お', 'か', 'き', 'く', 'け', 'こ', 'さ', 'し', 'す', 'せ', 'そ', 'た',
    'ち', 'つ', 'て', 'と',
];
#[derive(Debug, Clone, Copy, Default)]
pub struct PacketTestData;
struct Settings {
    shape: &'static str,
    size: usize,
    seed: u64,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 3
        || !object.contains_key("shape")
        || !object.contains_key("size")
        || !object.contains_key("seed")
    {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let shape = match object["shape"].as_str() {
        Some("text") => "text",
        Some("sequence") => "sequence",
        Some("choice") => "choice",
        _ => {
            return Err(CoreError::new(
                "invalid-config",
                "shape must be text, sequence or choice",
            ));
        }
    };
    let size = object["size"]
        .as_u64()
        .filter(|size| *size <= 1024)
        .ok_or_else(|| {
            CoreError::new(
                "invalid-config",
                "size must be an integer between 0 and 1024",
            )
        })? as usize;
    let seed = object["seed"].as_u64().ok_or_else(|| {
        CoreError::new("invalid-config", "seed must be an unsigned 64-bit integer")
    })?;
    Ok(Settings { shape, size, seed })
}
/// SplitMix64。決定的なテストデータ生成専用で、乱数源は外部へ公開しない。
struct Rng(u64);
impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn pick(&mut self, count: usize) -> usize {
        let n = count as u64;
        loop {
            let x = self.next_u64();
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
    fn short_text(&mut self) -> String {
        (0..3)
            .map(|_| ALPHABET[self.pick(ALPHABET.len())])
            .collect()
    }
}
fn generate(settings: &Settings) -> Packet {
    let mut rng = Rng(settings.seed);
    let root = match settings.shape {
        "text" => Candidate::Text {
            text: (0..settings.size)
                .map(|_| ALPHABET[rng.pick(ALPHABET.len())])
                .collect(),
            meta: None,
        },
        "sequence" => Candidate::Sequence {
            items: (0..settings.size)
                .map(|_| Candidate::Text {
                    text: rng.short_text(),
                    meta: None,
                })
                .collect(),
        },
        _ => Candidate::Choice {
            options: (0..settings.size)
                .map(|_| {
                    let text = rng.short_text();
                    let score = (rng.next_u64() % 1000) as f64 / 1000.0;
                    ChoiceOption {
                        value: Candidate::Text { text, meta: None },
                        score: Some(score),
                    }
                })
                .collect(),
        },
    };
    Packet::new(root)
}
impl Node for PacketTestData {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            // 内容は使わないが、実行のきっかけとして明示の入力を必須とする。
            vec![InputPort::required(Port::packet("trigger"))],
            vec![Port::packet("out")],
        );
        definition.default_config = json!({"shape":"text","size":16,"seed":0});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["shape","size","seed"],"properties":{"shape":{"enum":["text","sequence","choice"]},"size":{"type":"integer","minimum":0,"maximum":1024},"seed":{"type":"integer","minimum":0}}});
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
        let output = match inputs.get("trigger") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                OutputValue::Data(Arc::new(generate(&settings)))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: trigger")),
        };
        Ok([("out".into(), output)].into())
    }
}
