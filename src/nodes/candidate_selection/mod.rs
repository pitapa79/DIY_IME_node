//! 候補選択ノード `text.select-candidates@1`。
//! 契約: docs/node-contracts-v0.1.md / tests/contracts/candidate_selection.json
//!
//! 各 Choice の中だけで最大スコアの選択肢を選び、Sequence の順序・入れ子と
//! Text の文字列・メタデータをそのまま保持します。文字列の結合・スコアの合算・
//! 正規化・辞書の参照は行いません。ノードは状態を持たず、同じ入力・設定からは
//! 常に同じ出力を返します。
use std::sync::Arc;

use serde_json::{Value, json};

use crate::{
    Candidate, ChoiceOption, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};

/// ノードの kind。
pub const KIND: &str = "text.select-candidates";
/// ポートの言語ラベル。アルゴリズム自体は言語に依存しません。
const LANGUAGE: &str = "ja";
/// 設定キー。この1キーだけを許可します。
const UNSCORED_KEY: &str = "unscored";

/// 候補木から、Choice ごとにスコアに従って1つの選択肢を選ぶノード。
#[derive(Debug, Clone, Copy, Default)]
pub struct SelectCandidates;

impl Node for SelectCandidates {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(
                Port::packet("in")
                    .with_language(LANGUAGE)
                    .with_root(RootConstraint::Any),
            )],
            vec![
                Port::packet("out")
                    .with_language(LANGUAGE)
                    .with_root(RootConstraint::Any),
            ],
        );
        definition.default_config = json!({ UNSCORED_KEY: 0.0 });
        definition.config_schema = json!({
            "type": "object",
            "required": [UNSCORED_KEY],
            "additionalProperties": false,
            "properties": { UNSCORED_KEY: { "type": "number" } }
        });
        definition
    }

    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        unscored_score(config).map(|_| ())
    }

    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let unscored = unscored_score(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
                match select(&packet.root, unscored) {
                    Some(root) => OutputValue::Data(Arc::new(Packet::new(root))),
                    None => OutputValue::NoOutput,
                }
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => {
                return Err(CoreError::new(
                    "missing-input",
                    "required input `in` was not provided",
                ));
            }
        };
        Ok([("out".to_owned(), output)].into())
    }
}

/// 設定を検査し、スコア省略時の評価値を返す。`{"unscored": <有限数>}` だけを許可する。
fn unscored_score(config: &Value) -> Result<f64, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key(UNSCORED_KEY) {
        return Err(CoreError::new(
            "invalid-config",
            format!("expected exactly this setting: {UNSCORED_KEY}"),
        ));
    }
    object[UNSCORED_KEY]
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| CoreError::new("invalid-config", "unscored must be a finite number"))
}

/// 同一 Choice 内で最大スコアの選択肢を返す。同点は先頭、省略は `unscored`。空なら `None`。
fn best_option(options: &[ChoiceOption], unscored: f64) -> Option<&ChoiceOption> {
    let mut best: Option<(&ChoiceOption, f64)> = None;
    for option in options {
        let score = option.score.unwrap_or(unscored);
        // 厳密な大小比較だけで置き換えるため、同点（0.0 と -0.0 を含む）は先の選択肢が残る。
        if best.is_none_or(|(_, current)| score > current) {
            best = Some((option, score));
        }
    }
    best.map(|(option, _)| option)
}

/// 選択経路上のすべての Choice を解決した新しい候補木を返す。
/// 選択経路に空の Choice があれば `None`（NoOutput）。入力は変更しない。
/// 深い入れ子でもスタックを消費しないよう、再帰を使わずに走査する。
fn select(root: &Candidate, unscored: f64) -> Option<Candidate> {
    enum Frame<'a> {
        Visit(&'a Candidate),
        Sequence(usize),
    }
    let mut frames = vec![Frame::Visit(root)];
    let mut values: Vec<Candidate> = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Visit(Candidate::Text { text, meta }) => values.push(Candidate::Text {
                text: text.clone(),
                meta: meta.clone(),
            }),
            Frame::Visit(Candidate::Sequence { items }) => {
                frames.push(Frame::Sequence(items.len()));
                frames.extend(items.iter().rev().map(Frame::Visit));
            }
            // 選んだ枝だけを訪問する。選ばれなかった枝は走査も複製もしない。
            Frame::Visit(Candidate::Choice { options }) => {
                frames.push(Frame::Visit(&best_option(options, unscored)?.value));
            }
            Frame::Sequence(count) => {
                let items = values.split_off(values.len() - count);
                values.push(Candidate::Sequence { items });
            }
        }
    }
    values.pop()
}
