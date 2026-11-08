//! 最上位Choiceの重複を、最初の候補・スコアを残して削除する独立試作。
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

pub const KIND: &str = "experimental.choice-deduplicate";

/// 下位Choiceのスコアを含むCandidate構造を、再帰呼出しなしで比較する。
fn same_candidate(left: &Candidate, right: &Candidate) -> bool {
    let mut pending = vec![(left, right)];
    while let Some((left, right)) = pending.pop() {
        match (left, right) {
            (Candidate::Text { text: a, meta: am }, Candidate::Text { text: b, meta: bm })
                if a == b && am == bm => {}
            (Candidate::Sequence { items: a }, Candidate::Sequence { items: b })
                if a.len() == b.len() =>
            {
                pending.extend(a.iter().zip(b).rev());
            }
            (Candidate::Choice { options: a }, Candidate::Choice { options: b })
                if a.len() == b.len() =>
            {
                for (a, b) in a.iter().zip(b).rev() {
                    if a.score != b.score {
                        return false;
                    }
                    pending.push((&a.value, &b.value));
                }
            }
            _ => return false,
        }
    }
    true
}

/// トップのscoreを比較から除外し、valueが同じなら最初のoptionを保持する。
/// 下位候補の重複・順序・スコア・Textメタデータは変更しない。
pub fn deduplicate(packet: &Packet) -> Result<Packet, CoreError> {
    packet
        .validate()
        .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
    let Candidate::Choice { options } = &packet.root else {
        return Err(CoreError::new("invalid-root", "expected Choice"));
    };
    let mut retained: Vec<&ChoiceOption> = Vec::new();
    for option in options {
        if !retained
            .iter()
            .any(|previous| same_candidate(&previous.value, &option.value))
        {
            retained.push(option);
        }
    }
    let options = retained
        .into_iter()
        .map(|option| ChoiceOption {
            value: map_text(&option.value, |text, meta| Candidate::Text {
                text: text.to_owned(),
                meta: meta.cloned(),
            }),
            score: option.score,
        })
        .collect();
    Ok(Packet::new(Candidate::Choice { options }))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChoiceDeduplicate;

impl Node for ChoiceDeduplicate {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
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
                    .with_root(RootConstraint::Choice),
            ],
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
            Some(OutputValue::Data(packet)) => OutputValue::Data(Arc::new(deduplicate(packet)?)),
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
