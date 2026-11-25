//! 未採用の実験ノード。あいまい検索辞書検索。契約はexamples/nodes/dictionary_fuzzy/README.md。
//!
//! prepareで明示Host資源を取得し、text.dictionary-exactと同じ全件検証を通過した
//! 不変の辞書を所有する。索引成果物v1は読みのハッシュ順でありあいまい検索の範囲を
//! 取り出せないため再利用しない。完全一致とは別責務であり、一致なし・空入力では
//! 原文を候補として返さず、空のChoiceを返す。
use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Candidate, ChoiceOption, Packet,
    node::{
        CoreError, InputPort, Node, NodeDefinition, OutputValue, Outputs, Port, PrepareResources,
        PreparedNode, RootConstraint, RunContext,
    },
};

use crate::nodes::dictionary_exact::{Entry, MAX_CODE_POINTS, MAX_FILE_BYTES, collect_entries};

pub const KIND: &str = "experimental.text-dictionary-fuzzy";
pub const MAX_LIMIT: usize = 1024;
pub const DEFAULT_LIMIT: usize = 64;
pub const MAX_DISTANCE: usize = 3;
pub const DEFAULT_MAX_DISTANCE: usize = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyDictionary {
    entries: BTreeMap<String, Vec<Entry>>,
}

impl FuzzyDictionary {
    /// text.dictionary-exactと同じ全件検証を通過した場合だけ辞書を返す。
    pub fn from_tsv_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        Ok(Self {
            entries: collect_entries(bytes)?,
        })
    }

    /// 入力と読みの編集距離（コードポイント単位のLevenshtein距離）が
    /// max_distance以下の候補を返す。順序は距離の昇順→読みの昇順→元の行順。
    /// 距離は順序でのみ表し、scoreはTSVの重みのままにする。
    pub fn lookup_packet(
        &self,
        input: &Packet,
        limit: usize,
        max_distance: usize,
    ) -> Result<Packet, CoreError> {
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(CoreError::new(
                "invalid-config",
                "limit must be an integer between 1 and 1024",
            ));
        }
        if max_distance > MAX_DISTANCE {
            return Err(CoreError::new(
                "invalid-config",
                "maxDistance must be an integer between 0 and 3",
            ));
        }
        input
            .validate()
            .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
        let Candidate::Text { text, .. } = &input.root else {
            return Err(CoreError::new("invalid-root", "fuzzy lookup requires Text"));
        };
        if text.chars().count() > MAX_CODE_POINTS {
            return Err(CoreError::new(
                "dictionary-input-limit",
                "input exceeds 4096 code points",
            ));
        }
        let mut options: Vec<ChoiceOption> = Vec::new();
        if !text.is_empty() {
            let input: Vec<char> = text.chars().collect();
            let mut matched: Vec<(usize, ChoiceOption)> = Vec::new();
            // 読みの昇順→同一読み内は元の行順で集め、距離で安定ソートする。
            for (reading, entries) in &self.entries {
                let reading_chars: Vec<char> = reading.chars().collect();
                let Some(distance) = levenshtein(&input, &reading_chars, max_distance) else {
                    continue;
                };
                for entry in entries {
                    matched.push((
                        distance,
                        ChoiceOption {
                            value: Candidate::Text {
                                text: entry.surface.clone(),
                                meta: None,
                            },
                            score: Some(entry.score),
                        },
                    ));
                }
            }
            matched.sort_by_key(|(distance, _)| *distance);
            options = matched
                .into_iter()
                .map(|(_, option)| option)
                .take(limit)
                .collect();
        }
        Ok(Packet::new(Candidate::Choice { options }))
    }

    /// Data / NoOutputへ適用する純粋なラッパー。準備済みNodeも同じ処理を使う。
    pub fn lookup_output(
        &self,
        input: &OutputValue,
        limit: usize,
        max_distance: usize,
    ) -> Result<OutputValue, CoreError> {
        match input {
            OutputValue::Data(packet) => self
                .lookup_packet(packet, limit, max_distance)
                .map(|packet| OutputValue::Data(Arc::new(packet))),
            OutputValue::NoOutput => Ok(OutputValue::NoOutput),
        }
    }
}

/// コードポイント単位のLevenshtein距離。`max` を超える場合は None。
/// 長さ差による打ち切りと、帯状DP＋行最小値による早期終了で絞る。
fn levenshtein(a: &[char], b: &[char], max: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let inf = max + 1;
    let n = a.len();
    let m = b.len();
    let mut prev: Vec<usize> = (0..=n).map(|i| if i <= max { i } else { inf }).collect();
    let mut cur = vec![inf; n + 1];
    for j in 1..=m {
        let lo = j.saturating_sub(max);
        let hi = (j + max).min(n);
        if lo == 0 {
            cur[0] = j;
        } else {
            cur[lo - 1] = inf;
        }
        if hi < n {
            cur[hi + 1] = inf;
        }
        for i in lo.max(1)..=hi {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[i] = (prev[i] + 1).min(cur[i - 1] + 1).min(prev[i - 1] + cost);
            if cur[i] > max {
                cur[i] = inf;
            }
        }
        std::mem::swap(&mut prev, &mut cur);
        if prev[lo..=hi].iter().all(|&value| value > max) {
            return None;
        }
    }
    let distance = prev[n];
    (distance <= max).then_some(distance)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DictionaryFuzzy;
struct PreparedDictionaryFuzzy {
    dictionary: FuzzyDictionary,
    limit: usize,
    max_distance: usize,
}

fn config_parts(config: &serde_json::Value) -> Result<(&str, usize, usize), CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 3
        || !object.contains_key("path")
        || !object.contains_key("limit")
        || !object.contains_key("maxDistance")
    {
        return Err(CoreError::new(
            "invalid-config",
            "expected only path, limit and maxDistance",
        ));
    }
    let path = object["path"]
        .as_str()
        .filter(|path| !path.is_empty() && path.len() <= 4096 && !path.contains('\0'))
        .ok_or_else(|| {
            CoreError::new(
                "invalid-config",
                "path must be a nonempty resource name up to 4096 UTF-8 bytes",
            )
        })?;
    let limit = object["limit"]
        .as_u64()
        .filter(|limit| (1..=MAX_LIMIT as u64).contains(limit))
        .ok_or_else(|| {
            CoreError::new(
                "invalid-config",
                "limit must be an integer between 1 and 1024",
            )
        })? as usize;
    let max_distance = object["maxDistance"]
        .as_u64()
        .filter(|distance| *distance <= MAX_DISTANCE as u64)
        .ok_or_else(|| {
            CoreError::new(
                "invalid-config",
                "maxDistance must be an integer between 0 and 3",
            )
        })? as usize;
    Ok((path, limit, max_distance))
}

impl Node for DictionaryFuzzy {
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
        definition.default_config =
            serde_json::json!({"path":"dictionary.tsv","limit":64,"maxDistance":1});
        definition.config_schema = serde_json::json!({"type":"object","required":["path","limit","maxDistance"],"additionalProperties":false,"properties":{"path":{"type":"string","minLength":1,"description":"明示Hostルート基準の相対名。区切りは /"},"limit":{"type":"integer","minimum":1,"maximum":1024},"maxDistance":{"type":"integer","minimum":0,"maximum":3}}});
        definition
    }
    fn validate_config(&self, config: &serde_json::Value) -> Result<(), CoreError> {
        config_parts(config).map(|_| ())
    }
    fn prepare(
        &self,
        config: &serde_json::Value,
        resources: &mut dyn PrepareResources,
    ) -> Result<Option<Arc<dyn PreparedNode>>, CoreError> {
        let (path, limit, max_distance) = config_parts(config)?;
        let bytes = resources.read(path, MAX_FILE_BYTES).map_err(|error| {
            let code = match error.code.as_str() {
                "resource-unavailable" | "resource-path" => "dictionary-path",
                "resource-limit" => "dictionary-limit",
                "resource-read" | "resource-busy" | "resource-deadline" | "resource-platform" => {
                    "dictionary-read"
                }
                _ => &error.code,
            };
            CoreError::new(code, error.message)
        })?;
        let dictionary = FuzzyDictionary::from_tsv_bytes(&bytes)?;
        Ok(Some(Arc::new(PreparedDictionaryFuzzy {
            dictionary,
            limit,
            max_distance,
        })))
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        _: &crate::node::Inputs,
        config: &serde_json::Value,
    ) -> Result<Outputs, CoreError> {
        config_parts(config)?;
        Err(CoreError::new(
            "dictionary-not-prepared",
            "prepare the dictionary with explicit Host resources before execution",
        ))
    }
}
impl PreparedNode for PreparedDictionaryFuzzy {
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &crate::node::Inputs,
    ) -> Result<Outputs, CoreError> {
        let input = inputs
            .get("in")
            .ok_or_else(|| CoreError::new("missing-input", "required input in was not provided"))?;
        Ok([(
            "out".into(),
            self.dictionary
                .lookup_output(input, self.limit, self.max_distance)?,
        )]
        .into())
    }
}
