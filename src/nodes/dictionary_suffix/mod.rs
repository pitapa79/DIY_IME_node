//! 未採用の実験ノード。後方一致辞書検索。契約はexamples/nodes/dictionary_suffix/README.md。
//!
//! prepareで明示Host資源を取得し、text.dictionary-exactと同じ全件検証を通過した
//! 不変の辞書を所有する。索引成果物v1は読みのハッシュ順であり後方一致の範囲を
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

pub const KIND: &str = "experimental.text-dictionary-suffix";
pub const MAX_LIMIT: usize = 1024;
pub const DEFAULT_LIMIT: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub struct SuffixDictionary {
    entries: BTreeMap<String, Vec<Entry>>,
}

impl SuffixDictionary {
    /// text.dictionary-exactと同じ全件検証を通過した場合だけ辞書を返す。
    pub fn from_tsv_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        Ok(Self {
            entries: collect_entries(bytes)?,
        })
    }

    /// 入力を接尾辞として読みを検索する。順序は読みの昇順→同一読み内は元の行順。
    /// 完全一致する読みも含み、総候補数がlimitに達したらその順序のまま打ち切る。
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
                "suffix lookup requires Text",
            ));
        };
        if text.chars().count() > MAX_CODE_POINTS {
            return Err(CoreError::new(
                "dictionary-input-limit",
                "input exceeds 4096 code points",
            ));
        }
        let mut options: Vec<ChoiceOption> = Vec::new();
        if !text.is_empty() {
            // 読みの昇順→同一読み内は元の行順。接尾辞一致は範囲で絞れないため
            // 全件走査する（辞書は検証時点で件数上限を持つ）。
            'readings: for (reading, entries) in &self.entries {
                if !reading.ends_with(text.as_str()) {
                    continue;
                }
                for entry in entries {
                    if options.len() == limit {
                        break 'readings;
                    }
                    options.push(ChoiceOption {
                        value: Candidate::Text {
                            text: entry.surface.clone(),
                            meta: None,
                        },
                        score: Some(entry.score),
                    });
                }
            }
        }
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
pub struct DictionarySuffix;
struct PreparedDictionarySuffix {
    dictionary: SuffixDictionary,
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

impl Node for DictionarySuffix {
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
        let dictionary = SuffixDictionary::from_tsv_bytes(&bytes)?;
        Ok(Some(Arc::new(PreparedDictionarySuffix {
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
impl PreparedNode for PreparedDictionarySuffix {
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
