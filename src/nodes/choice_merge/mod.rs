//! Choiceの最上位候補を順序どおりに統合する独立実験Node。
use std::sync::Arc;

use serde_json::Value;

use crate::{
    Candidate, ChoiceOption, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
    tree::map_text,
};

pub const KIND: &str = "experimental.choice-merge";

#[derive(Debug, Clone, Copy, Default)]
pub struct ChoiceMerge;

impl Node for ChoiceMerge {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Choice)
        };
        NodeDefinition::new(
            KIND,
            1,
            vec![
                InputPort::optional(port("a")),
                InputPort::optional(port("b")),
            ],
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
        let mut options = Vec::new();
        for id in ["a", "b"] {
            let Some(OutputValue::Data(packet)) = inputs.get(id) else {
                continue;
            };
            packet
                .validate()
                .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
            let Candidate::Choice { options: source } = &packet.root else {
                return Err(CoreError::new("invalid-root", "expected Choice"));
            };
            for option in source {
                options.push(ChoiceOption {
                    value: map_text(&option.value, |text, meta| Candidate::Text {
                        text: text.to_owned(),
                        meta: meta.cloned(),
                    }),
                    score: option.score,
                });
            }
        }
        Ok([(
            "out".into(),
            OutputValue::Data(Arc::new(Packet::new(Candidate::Choice { options }))),
        )]
        .into())
    }
}
