//! ノード固有の小型TSV辞書と完全一致検索。
//!
//! Nodeはprepareで明示Host資源を取得し、計画が検査済み辞書を所有する。共通辞書形式ではない。
//! 検査済みの辞書は呼出元のバイト列や過去の検索から独立した不変データとして所有する。
use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Candidate, ChoiceOption, Packet,
    node::{CoreError, OutputValue},
};

/// このNode固有の上限。本体共通の辞書制約ではない。
pub const MAX_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_LINE_BYTES: usize = 64 * 1024;
pub const MAX_ROWS: usize = 10_000;
pub const MAX_CANDIDATES: usize = 64;
pub const MAX_CODE_POINTS: usize = 4096;
const HEADER: &str = "reading\tsurface\tweight";

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    surface: String,
    score: f64,
    line: usize,
}

/// 全行の検査に成功した場合だけ作成できる、不変のノード固有辞書。
#[derive(Debug, Clone, PartialEq)]
pub struct ExactDictionary {
    entries: BTreeMap<String, Vec<Entry>>,
}

fn format_error(line: usize, column: &str, message: &str) -> CoreError {
    CoreError::new(
        "dictionary-format",
        format!("line {line}, {column}: {message}"),
    )
}

fn limit_error(line: usize, limit: &str, maximum: usize) -> CoreError {
    CoreError::new(
        "dictionary-limit",
        format!("line {line}: {limit} exceeds {maximum}"),
    )
}

fn field(value: &str, line: usize, column: &str) -> Result<(), CoreError> {
    if value.is_empty() || value.contains('\0') {
        return Err(format_error(
            line,
            column,
            "expected nonempty text without NUL",
        ));
    }
    if value.chars().count() > MAX_CODE_POINTS {
        return Err(limit_error(line, column, MAX_CODE_POINTS));
    }
    Ok(())
}

/// JSON数値の字句だけを許可し、その後f64としての有限性を検査する。
fn weight(value: &str, line: usize) -> Result<f64, CoreError> {
    let bytes = value.as_bytes();
    let mut position = usize::from(bytes.first() == Some(&b'-'));
    match bytes.get(position) {
        Some(b'0') => position += 1,
        Some(b'1'..=b'9') => {
            while bytes.get(position).is_some_and(u8::is_ascii_digit) {
                position += 1;
            }
        }
        _ => return Err(format_error(line, "weight", "expected a JSON number")),
    }
    if bytes.get(position) == Some(&b'.') {
        position += 1;
        let start = position;
        while bytes.get(position).is_some_and(u8::is_ascii_digit) {
            position += 1;
        }
        if position == start {
            return Err(format_error(line, "weight", "fraction requires digits"));
        }
    }
    if matches!(bytes.get(position), Some(b'e' | b'E')) {
        position += 1;
        if matches!(bytes.get(position), Some(b'+' | b'-')) {
            position += 1;
        }
        let start = position;
        while bytes.get(position).is_some_and(u8::is_ascii_digit) {
            position += 1;
        }
        if position == start {
            return Err(format_error(line, "weight", "exponent requires digits"));
        }
    }
    if position != bytes.len() {
        return Err(format_error(
            line,
            "weight",
            "expected exactly one JSON number",
        ));
    }
    let score = value
        .parse::<f64>()
        .map_err(|_| format_error(line, "weight", "number is not representable as f64"))?;
    if !score.is_finite() {
        return Err(format_error(line, "weight", "number must be finite"));
    }
    Ok(score)
}

impl ExactDictionary {
    /// UTF-8/BOM、改行、全行、重複、上限を検査し、成功時だけ辞書を返す。
    /// 呼出元のバイト列を借用し続けず、入力から独立した所有データを作る。
    pub fn from_tsv_bytes(bytes: &[u8]) -> Result<Self, CoreError> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(limit_error(0, "file bytes", MAX_FILE_BYTES));
        }
        let text = std::str::from_utf8(bytes).map_err(|error| {
            CoreError::new(
                "dictionary-encoding",
                format!("invalid UTF-8 at byte {}", error.valid_up_to()),
            )
        })?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut entries: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        let mut saw_header = false;
        for (index, segment) in text.split_inclusive('\n').enumerate() {
            let line_number = index + 1;
            let line = match segment.strip_suffix('\n') {
                Some(line) => line.strip_suffix('\r').unwrap_or(line),
                None => segment,
            };
            if line.len() > MAX_LINE_BYTES {
                return Err(limit_error(line_number, "line bytes", MAX_LINE_BYTES));
            }
            if line.contains('\r') {
                return Err(format_error(
                    line_number,
                    "line",
                    "bare CR is not supported",
                ));
            }
            if index == 0 {
                if line != HEADER {
                    return Err(format_error(
                        1,
                        "header",
                        "expected reading, surface, weight",
                    ));
                }
                saw_header = true;
                continue;
            }
            if index > MAX_ROWS {
                return Err(limit_error(line_number, "data rows", MAX_ROWS));
            }
            let mut columns = line.split('\t');
            let (Some(reading), Some(surface), Some(raw_weight), None) = (
                columns.next(),
                columns.next(),
                columns.next(),
                columns.next(),
            ) else {
                return Err(format_error(
                    line_number,
                    "line",
                    "expected exactly three columns",
                ));
            };
            field(reading, line_number, "reading")?;
            field(surface, line_number, "surface")?;
            let score = weight(raw_weight, line_number)?;
            let candidates = entries.entry(reading.to_owned()).or_default();
            if let Some(first) = candidates.iter().find(|entry| entry.surface == surface) {
                return Err(format_error(
                    line_number,
                    "reading/surface",
                    &format!("duplicate pair, first occurrence at line {}", first.line),
                ));
            }
            if candidates.len() == MAX_CANDIDATES {
                return Err(limit_error(
                    line_number,
                    "candidates per reading",
                    MAX_CANDIDATES,
                ));
            }
            candidates.push(Entry {
                surface: surface.into(),
                score,
                line: line_number,
            });
        }
        if !saw_header {
            return Err(format_error(1, "header", "dictionary is empty"));
        }
        Ok(Self { entries })
    }

    /// Textの全文を完全一致で検索する。候補は行順、未知語・空入力はscoreなしの原文1候補。
    /// 正規化・部分一致・選択は行わず、出力Textのmetaはすべて省略する。
    pub fn lookup_packet(&self, input: &Packet) -> Result<Packet, CoreError> {
        input
            .validate()
            .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
        let Candidate::Text { text, .. } = &input.root else {
            return Err(CoreError::new("invalid-root", "exact lookup requires Text"));
        };
        if text.chars().count() > MAX_CODE_POINTS {
            return Err(CoreError::new(
                "dictionary-input-limit",
                "input exceeds 4096 code points",
            ));
        }
        let option = |text: String, score| ChoiceOption {
            value: Candidate::Text { text, meta: None },
            score,
        };
        let options = match self.entries.get(text) {
            Some(entries) => entries
                .iter()
                .map(|entry| option(entry.surface.clone(), Some(entry.score)))
                .collect(),
            None => vec![option(text.clone(), None)],
        };
        Ok(Packet::new(Candidate::Choice { options }))
    }

    /// Data / NoOutputへ適用する純粋なラッパー。準備済みNodeも同じ処理を使う。
    pub fn lookup_output(&self, input: &OutputValue) -> Result<OutputValue, CoreError> {
        match input {
            OutputValue::Data(packet) => self
                .lookup_packet(packet)
                .map(|packet| OutputValue::Data(Arc::new(packet))),
            OutputValue::NoOutput => Ok(OutputValue::NoOutput),
        }
    }
}

/// 初版の全文完全一致辞書ノード。設定pathは明示Hostルート基準。
pub const KIND: &str = "text.dictionary-exact";
#[derive(Debug, Clone, Copy, Default)]
pub struct DictionaryExact;
struct PreparedDictionary(ExactDictionary);
fn path_config(config: &serde_json::Value) -> Result<&str, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("path") {
        return Err(CoreError::new("invalid-config", "expected only path"));
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
    Ok(path)
}
impl crate::node::Node for DictionaryExact {
    fn definition(&self) -> crate::node::NodeDefinition {
        use crate::node::{InputPort, NodeDefinition, Port, RootConstraint};
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
        definition.default_config = serde_json::json!({"path":"dictionary.tsv"});
        definition.config_schema = serde_json::json!({"type":"object","required":["path"],"additionalProperties":false,"properties":{"path":{"type":"string","minLength":1,"description":"明示Hostルート基準の相対名。区切りは /"}}});
        definition
    }
    fn validate_config(&self, config: &serde_json::Value) -> Result<(), CoreError> {
        path_config(config).map(|_| ())
    }
    fn prepare(
        &self,
        config: &serde_json::Value,
        resources: &mut dyn crate::node::PrepareResources,
    ) -> Result<Option<Arc<dyn crate::node::PreparedNode>>, CoreError> {
        let path = path_config(config)?;
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
        // Check length again at the Node boundary even for a custom Rust provider.
        let dictionary = ExactDictionary::from_tsv_bytes(&bytes)?;
        Ok(Some(Arc::new(PreparedDictionary(dictionary))))
    }
    fn execute(
        &self,
        _: crate::node::RunContext<'_>,
        _: &crate::node::Inputs,
        config: &serde_json::Value,
    ) -> Result<crate::node::Outputs, CoreError> {
        path_config(config)?;
        Err(CoreError::new(
            "dictionary-not-prepared",
            "prepare the dictionary with explicit Host resources before execution",
        ))
    }
}
impl crate::node::PreparedNode for PreparedDictionary {
    fn execute(
        &self,
        _: crate::node::RunContext<'_>,
        inputs: &crate::node::Inputs,
    ) -> Result<crate::node::Outputs, CoreError> {
        let input = inputs
            .get("in")
            .ok_or_else(|| CoreError::new("missing-input", "required input in was not provided"))?;
        Ok([("out".into(), self.0.lookup_output(input)?)].into())
    }
}

#[cfg(feature = "dictionary-artifact")]
pub mod artifact;
