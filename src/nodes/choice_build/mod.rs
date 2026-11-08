//! 設定と任意入力からChoiceを作る独立実験Node。
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    Candidate, ChoiceOption, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
    tree::map_text,
};

pub const KIND: &str = "experimental.choice-build";

#[derive(Debug, Clone, Copy, Default)]
pub struct ChoiceBuild;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    options: Vec<ChoiceOption>,
}

fn configured_options(config: &Value) -> Result<Vec<ChoiceOption>, CoreError> {
    let config = Config::deserialize(config)
        .map_err(|error| CoreError::new("invalid-config", error.to_string()))?;
    let packet = Packet::new(Candidate::Choice {
        options: config.options,
    });
    packet
        .validate()
        .map_err(|error| CoreError::new("invalid-config", error.to_string()))?;
    let Candidate::Choice { options } = packet.root else {
        unreachable!("constructed Choice")
    };
    Ok(options)
}

impl Node for ChoiceBuild {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![
                InputPort::optional(Port::packet("a").with_language("ja")),
                InputPort::optional(Port::packet("b").with_language("ja")),
            ],
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(RootConstraint::Choice),
            ],
        );
        definition.default_config = json!({"options": []});
        definition.config_schema = json!({
            "type": "object", "required": ["options"], "additionalProperties": false,
            "properties": {"options": {"type": "array", "items": {
                "type": "object", "required": ["value"], "additionalProperties": false,
                "properties": {"value": {"type": "object"}, "score": {"type": "number"}}
            }}}
        });
        definition
    }

    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        configured_options(config).map(|_| ())
    }

    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let mut options = configured_options(config)?;
        for id in ["a", "b"] {
            let Some(OutputValue::Data(packet)) = inputs.get(id) else {
                continue;
            };
            packet
                .validate()
                .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
            let value = map_text(&packet.root, |text, meta| Candidate::Text {
                text: text.to_owned(),
                meta: meta.cloned(),
            });
            options.push(ChoiceOption { value, score: None });
        }
        Ok([(
            "out".into(),
            OutputValue::Data(Arc::new(Packet::new(Candidate::Choice { options }))),
        )]
        .into())
    }
}
