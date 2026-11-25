//! 未採用の実験ノード。契約はexamples/nodes/text_kanji_variants/README.md。
use crate::{
    Candidate, ChoiceOption, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-kanji-variants";
/// 基準字→異体字の明示対応表（#85の旧新字体表とは独立に持つ）。
const VARIANTS: &[(char, &[char])] = &[
    ('高', &['髙']),
    ('崎', &['﨑', '嵜']),
    ('島', &['嶋', '嶌']),
    ('凛', &['凜']),
    ('熙', &['煕']),
    ('遥', &['遙']),
    ('曾', &['曽']),
    ('餅', &['餠']),
    ('遡', &['溯']),
    ('麹', &['麴']),
];
#[derive(Debug, Clone, Copy, Default)]
pub struct TextKanjiVariants;
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
fn text_option(text: &str) -> ChoiceOption {
    ChoiceOption {
        value: Candidate::Text {
            text: text.into(),
            meta: None,
        },
        score: None,
    }
}
impl Node for TextKanjiVariants {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(
                Port::packet("in")
                    .with_language("ja")
                    .with_root(RootConstraint::Text),
            )],
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(RootConstraint::Choice),
            ],
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
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                let mut chars = text.chars();
                let single = match (chars.next(), chars.next()) {
                    (Some(ch), None) => Some(ch),
                    _ => None,
                };
                let mut options = vec![text_option(text)];
                if let Some(ch) = single
                    && let Some((_, variants)) = VARIANTS.iter().find(|(base, _)| *base == ch)
                {
                    for variant in *variants {
                        options.push(text_option(&variant.to_string()));
                    }
                }
                OutputValue::Data(Arc::new(Packet::new(Candidate::Choice { options })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
