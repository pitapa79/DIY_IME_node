//! 未採用の実験ノード。辞書逆引き。契約はexamples/nodes/dictionary_reverse/README.md。
//!
//! prepareで明示Host資源を取得し、text.dictionary-exactと同じ全件検証を通過した
//! 不変の辞書を所有する。索引成果物v1は読みのハッシュ順であり表記からは引けない
//! ため再利用しない。完全一致・前方一致とは別責務であり、一致なし・空入力では
//! 原文を候補として返さず、空のChoiceを返す。
use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Candidate, ChoiceOption, Packet,
    node::{
        CoreError, InputPort, Node, NodeDefinition, OutputValue, Outputs, Port, PrepareResources,
        PreparedNode, RootConstraint, RunContext,
    },
};

use crate::nodes::dictionary_exact::{MAX_CODE_POINTS, MAX_FILE_BYTES, collect_entries};

pub const KIND: &str = "experimental.text-dictionary-reverse";
pub const MAX_LIMIT: usize = 1024;
pub const DEFAULT_LIMIT: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub struct ReverseDictionary {
    /// 表記 -> (読み, 重み)。並びは元の行順で、重みでは並べ替えない。
    by_surface: BTreeMap<String, Vec<(String, f64)>>,
}

impl ReverseDictionary {
    /// text.dictionary-exactと同じ全件検証を通過した場合だけ辞書を返す。
    pub fn from_tsv_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        let by_reading = collect_entries(bytes)?;
        let mut rows: Vec<(usize, String, String, f64)> = Vec::new();
        for (reading, entries) in by_reading {
            for entry in entries {
                rows.push((entry.line, entry.surface, reading.clone(), entry.score));
            }
        }
        rows.sort_by_key(|(line, ..)| *line);
        let mut by_surface: BTreeMap<String, Vec<(String, f64)>> = BTreeMap::new();
        for (_, surface, reading, score) in rows {
            by_surface
                .entry(surface)
                .or_default()
                .push((reading, score));
        }
        Ok(Self { by_surface })
    }

    /// 表記を完全一致で引き、読み候補を元の行順で返す。
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
                "reverse lookup requires Text",
            ));
        };
        if text.chars().count() > MAX_CODE_POINTS {
            return Err(CoreError::new(
                "dictionary-input-limit",
                "input exceeds 4096 code points",
            ));
        }
        let mut options: Vec<ChoiceOption> = Vec::new();
        if !text.is_empty()
            && let Some(readings) = self.by_surface.get(text.as_str())
        {
            for (reading, score) in readings.iter().take(limit) {
                options.push(ChoiceOption {
                    value: Candidate::Text {
                        text: reading.clone(),
                        meta: None,
                    },
                    score: Some(*score),
                });
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
pub struct DictionaryReverse;
struct PreparedDictionaryReverse {
    dictionary: ReverseDictionary,
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

impl Node for DictionaryReverse {
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
        let dictionary = ReverseDictionary::from_tsv_bytes(&bytes)?;
        Ok(Some(Arc::new(PreparedDictionaryReverse {
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
impl PreparedNode for PreparedDictionaryReverse {
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
