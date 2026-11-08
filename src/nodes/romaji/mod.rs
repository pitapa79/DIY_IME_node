//! ローマ字→ひらがな変換ノード（`text.romaji-to-hiragana@1`）。
//! 契約: docs/node-contracts-v0.1.md / tests/contracts/romaji.json
//!
//! 変換の核は [`convert`] の純粋関数。RunContextの実行時入力からモードを読み、
//! 接続された入力ポートの全文を変換する。実行時入力がなければfinalizeを使う。
mod convert;

use std::sync::Arc;

use serde_json::{Value, json};

pub use convert::{Config, Converted, Mode, Override, Unknown, convert};

use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputMode, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};

pub const KIND: &str = "text.romaji-to-hiragana";

/// `text.romaji-to-hiragana@1`
pub struct RomajiToHiragana;

fn text_packet(text: String) -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(Candidate::Text { text, meta: None })))
}

impl Node for RomajiToHiragana {
    fn definition(&self) -> NodeDefinition {
        let port = |id: &str, language: &str| {
            Port::packet(id)
                .with_language(language)
                .with_root(RootConstraint::Text)
        };
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(port("in", "ja-Latn"))],
            vec![port("out", "ja"), port("pending", "ja-Latn")],
        );
        definition.default_config = json!({"overrides": [], "unknown": "preserve"});
        definition.config_schema = json!({
            "type": "object",
            "required": ["overrides", "unknown"],
            "additionalProperties": false,
            "properties": {
                "unknown": {"enum": ["preserve", "error"]},
                "overrides": {
                    "type": "array",
                    "maxItems": 256,
                    "items": {
                        "type": "object",
                        "required": ["from", "to"],
                        "additionalProperties": false,
                        "properties": {
                            "from": {"type": "string", "pattern": "^[a-z']{1,8}$"},
                            "to": {"type": "string", "minLength": 1, "maxLength": 16}
                        }
                    }
                }
            }
        });
        definition
    }

    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        Config::from_value(config).map(|_| ())
    }

    fn execute(
        &self,
        context: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let config = Config::from_value(config)?;
        let packet = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => packet,
            Some(OutputValue::NoOutput) => {
                return Ok([
                    ("out".to_string(), OutputValue::NoOutput),
                    ("pending".to_string(), OutputValue::NoOutput),
                ]
                .into());
            }
            None => return Err(CoreError::new("missing-input", "in is required")),
        };
        packet
            .validate()
            .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
        let Candidate::Text { text, .. } = &packet.root else {
            return Err(CoreError::new(
                "invalid-root",
                "this node requires a Text root",
            ));
        };
        let mode = match context.input.map(|input| input.mode) {
            Some(InputMode::Compose) => Mode::Compose,
            Some(InputMode::Finalize) | None => Mode::Finalize,
        };
        let converted = convert(text, mode, &config)?;
        Ok([
            ("out".to_string(), text_packet(converted.text)),
            ("pending".to_string(), text_packet(converted.pending)),
        ]
        .into())
    }
}
