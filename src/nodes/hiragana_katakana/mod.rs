//! ひらがなを対応する全角カタカナへ変換する独立実験Node。
use std::sync::Arc;

use serde_json::Value;

use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};

pub const KIND: &str = "experimental.hiragana-katakana";

/// 正規化を行わず、Unicodeに対応するカタカナがある範囲だけを変換する。
#[derive(Debug, Clone, Copy, Default)]
pub struct HiraganaKatakana;

impl Node for HiraganaKatakana {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Text)
        };
        NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(port("in"))],
            vec![port("out")],
        )
    }

    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        self.validate_config(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
                let Candidate::Text { text, meta } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                let converted: String = text
                    .chars()
                    .map(|character| match character {
                        '\u{3041}'..='\u{3096}' | '\u{309d}' | '\u{309e}' => {
                            char::from_u32(character as u32 + 0x60)
                                .expect("mapped katakana range contains only Unicode scalar values")
                        }
                        other => other,
                    })
                    .collect();
                let output_meta = if converted == *text {
                    meta.clone()
                } else {
                    None
                };
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: converted,
                    meta: output_meta,
                })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
