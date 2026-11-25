//! 未採用の実験ノード。辞書による読み文字列の分かち書き候補。契約は
//! examples/nodes/dictionary_segment/README.md。
//!
//! prepareで明示Host資源を取得し、text.dictionary-exactと同じ全件検証を通過した
//! 不変の辞書を所有する。入力の読み文字列を辞書の読みで覆う分割候補を、未知文字数→
//! 分割数→辞書式順序で列挙する。表層は候補に含めない（分割位置だけを返す）。
//! 索引成果物v1は読みのハッシュ順であり分割には使えないため再利用しない。
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap},
    sync::Arc,
};

use crate::{
    Candidate, ChoiceOption, Packet,
    node::{
        CoreError, InputPort, Node, NodeDefinition, OutputValue, Outputs, Port, PrepareResources,
        PreparedNode, RootConstraint, RunContext,
    },
};

use crate::nodes::dictionary_exact::{Entry, MAX_CODE_POINTS, MAX_FILE_BYTES, collect_entries};

pub const KIND: &str = "experimental.text-dictionary-segment";
pub const MAX_LIMIT: usize = 1024;
pub const DEFAULT_LIMIT: usize = 64;
/// 最良優先探索の展開上限。決定的に打ち切り、得られた候補だけを返す。
const MAX_EXPANSIONS: usize = 100_000;
const MAX_EDGE_CHECKS: usize = 250_000;
const MAX_STATE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct SegmentDictionary {
    entries: BTreeMap<String, Vec<Entry>>,
}

#[derive(Eq, PartialEq)]
struct State {
    unknown: usize,
    segments: Vec<String>,
    position: usize,
}
impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeapは最大値を取り出すため、順位キーを反転して最良を先頭にする。
        other
            .unknown
            .cmp(&self.unknown)
            .then_with(|| other.segments.len().cmp(&self.segments.len()))
            .then_with(|| other.segments.cmp(&self.segments))
    }
}
impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl SegmentDictionary {
    /// text.dictionary-exactと同じ全件検証を通過した場合だけ辞書を返す。
    pub fn from_tsv_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        Ok(Self {
            entries: collect_entries(bytes)?,
        })
    }

    /// 分割候補を順位順に最大limit件返す。各候補は読み部分文字列の列。
    pub fn segment(&self, text: &str, limit: usize) -> Vec<Vec<String>> {
        let chars: Vec<char> = text.chars().collect();
        let mut results: Vec<Vec<String>> = Vec::new();
        if chars.is_empty() {
            return results;
        }
        if !self.entries.keys().any(|reading| text.contains(reading)) {
            return vec![vec![text.to_owned()]];
        }
        let mut heap = BinaryHeap::new();
        heap.push(State {
            unknown: 0,
            segments: Vec::new(),
            position: 0,
        });
        let mut expansions = 0;
        let mut edge_checks = 0;
        let mut state_bytes = 0;
        'search: while let Some(state) = heap.pop() {
            if state.position == chars.len() {
                results.push(state.segments);
                if results.len() >= limit {
                    break;
                }
                continue;
            }
            expansions += 1;
            if expansions > MAX_EXPANSIONS {
                break;
            }
            let last_unknown = state
                .segments
                .last()
                .is_some_and(|last| !self.entries.contains_key(last.as_str()));
            let remaining = chars.len() - state.position;
            let mut piece = String::new();
            for len in 1..=remaining {
                if edge_checks == MAX_EDGE_CHECKS {
                    break 'search;
                }
                edge_checks += 1;
                piece.push(chars[state.position + len - 1]);
                let known = self.entries.contains_key(piece.as_str());
                // 未知区間は連続させない（連続する未知文字は1区間にまとめる）。
                // 既知になる長さまでは、未知でも走査だけは続ける。
                if known || !last_unknown {
                    // Charge owned paths even after they leave the heap.
                    state_bytes += 2 * std::mem::size_of::<State>()
                        + (state.segments.len() + 1) * std::mem::size_of::<String>()
                        + (state.position + len) * 4;
                    if state_bytes > MAX_STATE_BYTES {
                        break 'search;
                    }
                    heap.push(State {
                        unknown: state.unknown + if known { 0 } else { len },
                        segments: state
                            .segments
                            .iter()
                            .cloned()
                            .chain(std::iter::once(piece.clone()))
                            .collect(),
                        position: state.position + len,
                    });
                }
            }
        }
        results
    }

    pub fn lookup_packet(&self, input: &Packet, limit: usize) -> Result<Packet, CoreError> {
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(CoreError::new(
                "invalid-config",
                "limit must be an integer between 1 and 1024",
            ));
        }
        input
            .validate()
            .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
        let Candidate::Text { text, .. } = &input.root else {
            return Err(CoreError::new(
                "invalid-root",
                "segment lookup requires Text",
            ));
        };
        if text.chars().count() > MAX_CODE_POINTS {
            return Err(CoreError::new(
                "dictionary-input-limit",
                "input exceeds 4096 code points",
            ));
        }
        let options = self
            .segment(text, limit)
            .into_iter()
            .map(|segments| ChoiceOption {
                value: Candidate::Sequence {
                    items: segments
                        .into_iter()
                        .map(|segment| Candidate::Text {
                            text: segment,
                            meta: None,
                        })
                        .collect(),
                },
                score: None,
            })
            .collect();
        Ok(Packet::new(Candidate::Choice { options }))
    }

    /// Data / NoOutputへ適用する純粋なラッパー。準備済みNodeも同じ処理を使う。
    pub fn lookup_output(
        &self,
        input: &OutputValue,
        limit: usize,
    ) -> Result<OutputValue, CoreError> {
        match input {
            OutputValue::Data(packet) => self
                .lookup_packet(packet, limit)
                .map(|packet| OutputValue::Data(Arc::new(packet))),
            OutputValue::NoOutput => Ok(OutputValue::NoOutput),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DictionarySegment;
struct PreparedDictionarySegment {
    dictionary: SegmentDictionary,
    limit: usize,
}

fn config_parts(config: &serde_json::Value) -> Result<(&str, usize), CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 2 || !object.contains_key("path") || !object.contains_key("limit") {
        return Err(CoreError::new(
            "invalid-config",
            "expected only path and limit",
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
    Ok((path, limit))
}

impl Node for DictionarySegment {
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
        definition.default_config = serde_json::json!({"path":"dictionary.tsv","limit":64});
        definition.config_schema = serde_json::json!({"type":"object","required":["path","limit"],"additionalProperties":false,"properties":{"path":{"type":"string","minLength":1,"description":"明示Hostルート基準の相対名。区切りは /"},"limit":{"type":"integer","minimum":1,"maximum":1024}}});
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
        let (path, limit) = config_parts(config)?;
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
        let dictionary = SegmentDictionary::from_tsv_bytes(&bytes)?;
        Ok(Some(Arc::new(PreparedDictionarySegment {
            dictionary,
            limit,
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
impl PreparedNode for PreparedDictionarySegment {
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
            self.dictionary.lookup_output(input, self.limit)?,
        )]
        .into())
    }
}
