//! Experimental, bounded guest wire. This is not the Core/Session management API.
use crate::{
    Packet,
    node::{
        CoreError, InputMode, Inputs, Node, NodeDefinition, OutputValue, Outputs, PrepareResources,
        RunContext, RuntimeInput,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

pub mod service;

pub const API_VERSION: u32 = 1;
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Request {
    pub api_version: u32,
    pub node: NodeIdentity,
    pub operation: Operation,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NodeIdentity {
    pub kind: String,
    pub version: u32,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "method", rename_all = "camelCase", deny_unknown_fields)]
pub enum Operation {
    Describe {},
    Validate {
        config: Value,
    },
    Resume {
        config: Value,
        #[serde(deserialize_with = "present_json")]
        continuation: Value,
        response: service::HttpResponse,
        #[serde(default)]
        resources: BTreeMap<String, Vec<u8>>,
    },
    Prepare {
        config: Value,
        resources: BTreeMap<String, Vec<u8>>,
    },
    Execute {
        config: Value,
        inputs: BTreeMap<String, WireValue>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<InputMode>,
        #[serde(default)]
        resources: BTreeMap<String, Vec<u8>>,
    },
}
fn present_json<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
    Value::deserialize(deserializer)
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", deny_unknown_fields)]
pub enum WireValue {
    Data { packet: Packet },
    NoOutput {},
}

struct Snapshots(BTreeMap<String, Vec<u8>>);
impl PrepareResources for Snapshots {
    fn read(&mut self, reference: &str, max_bytes: usize) -> Result<Arc<[u8]>, CoreError> {
        let bytes = self.0.get(reference).ok_or_else(|| {
            CoreError::new("resource-unavailable", "snapshot was not supplied by Host")
        })?;
        if bytes.len() > max_bytes {
            return Err(CoreError::new(
                "resource-limit",
                "snapshot exceeds Node limit",
            ));
        }
        Ok(Arc::from(bytes.as_slice()))
    }
}

fn make_node(kind: &str, version: u32) -> Result<Box<dyn Node>, CoreError> {
    match (kind, version) {
        ("demo.branch", 1) => Ok(Box::new(crate::compat::demo::Branch)),
        ("demo.identity", 1) => Ok(Box::new(crate::compat::demo::Identity)),
        ("demo.select-text", 1) => Ok(Box::new(crate::compat::demo::SelectText)),
        ("demo.uppercase", 1) => Ok(Box::new(crate::compat::demo::Uppercase)),
        ("e2e.kana-kanji", 1) => Ok(Box::new(crate::compat::initial_e2e::KanaKanjiCandidates)),
        ("e2e.romaji-to-hiragana", 1) => Ok(Box::new(crate::compat::initial_e2e::RomajiToHiragana)),
        ("e2e.segment", 1) => Ok(Box::new(crate::compat::initial_e2e::Segment)),
        ("e2e.select-candidates", 1) => Ok(Box::new(crate::compat::initial_e2e::SelectCandidates)),
        ("e2e.stringify", 1) => Ok(Box::new(crate::compat::initial_e2e::Stringify)),
        ("experimental.ascii-width", 1) => Ok(Box::new(crate::nodes::ascii_width::AsciiWidth)),
        ("experimental.character-filter", 1) => {
            Ok(Box::new(crate::nodes::character_filter::CharacterFilter))
        }
        ("experimental.choice-build", 1) => Ok(Box::new(crate::nodes::choice_build::ChoiceBuild)),
        ("experimental.choice-deduplicate", 1) => Ok(Box::new(
            crate::nodes::choice_deduplicate::ChoiceDeduplicate,
        )),
        ("experimental.choice-extract", 1) => {
            Ok(Box::new(crate::nodes::choice_extract::ChoiceExtract))
        }
        ("experimental.choice-limit", 1) => Ok(Box::new(crate::nodes::choice_limit::ChoiceLimit)),
        ("experimental.choice-merge", 1) => Ok(Box::new(crate::nodes::choice_merge::ChoiceMerge)),
        ("experimental.choice-remove", 1) => {
            Ok(Box::new(crate::nodes::choice_remove::ChoiceRemove))
        }
        ("experimental.choice-sort", 1) => Ok(Box::new(crate::nodes::choice_sort::ChoiceSort)),
        ("experimental.choice-threshold", 1) => {
            Ok(Box::new(crate::nodes::choice_threshold::ChoiceThreshold))
        }
        ("experimental.choice-top-n", 1) => Ok(Box::new(crate::nodes::choice_top_n::ChoiceTopN)),
        ("experimental.hiragana-katakana", 1) => {
            Ok(Box::new(crate::nodes::hiragana_katakana::HiraganaKatakana))
        }
        ("experimental.japanese-punctuation", 1) => Ok(Box::new(
            crate::nodes::japanese_punctuation::JapanesePunctuation,
        )),
        ("experimental.json-escape", 1) => Ok(Box::new(crate::nodes::json_escape::JsonEscape)),
        ("experimental.katakana-hiragana", 1) => {
            Ok(Box::new(crate::nodes::katakana_hiragana::KatakanaHiragana))
        }
        ("experimental.line-endings", 1) => Ok(Box::new(crate::nodes::line_endings::LineEndings)),
        ("experimental.packet-depth-limit", 1) => {
            Ok(Box::new(crate::nodes::packet_depth_limit::PacketDepthLimit))
        }
        ("experimental.packet-passthrough", 1) => Ok(Box::new(
            crate::nodes::packet_passthrough::PacketPassthrough,
        )),
        ("experimental.packet-rebuild", 1) => {
            Ok(Box::new(crate::nodes::packet_rebuild::PacketRebuild))
        }
        ("experimental.score-add", 1) => Ok(Box::new(crate::nodes::score_add::ScoreAdd)),
        ("experimental.score-multiply", 1) => {
            Ok(Box::new(crate::nodes::score_multiply::ScoreMultiply))
        }
        ("experimental.score-normalize", 1) => {
            Ok(Box::new(crate::nodes::score_normalize::ScoreNormalize))
        }
        ("experimental.score-set", 1) => Ok(Box::new(crate::nodes::score_set::ScoreSet)),
        ("experimental.sentence-split", 1) => {
            Ok(Box::new(crate::nodes::sentence_split::SentenceSplit))
        }
        ("experimental.sequence-build", 1) => {
            Ok(Box::new(crate::nodes::sequence_build::SequenceBuild))
        }
        ("experimental.sequence-extract", 1) => {
            Ok(Box::new(crate::nodes::sequence_extract::SequenceExtract))
        }
        ("experimental.text-case", 1) => Ok(Box::new(crate::nodes::text_case::TextCase)),
        ("experimental.text-concat", 1) => Ok(Box::new(crate::nodes::text_concat::TextConcat)),
        ("experimental.text-extract", 1) => Ok(Box::new(crate::nodes::text_extract::TextExtract)),
        ("experimental.text-prefix", 1) => Ok(Box::new(crate::nodes::text_prefix::TextPrefix)),
        ("experimental.text-replace", 1) => Ok(Box::new(crate::nodes::text_replace::TextReplace)),
        ("experimental.text-split", 1) => Ok(Box::new(crate::nodes::text_split::TextSplit)),
        ("experimental.text-suffix", 1) => Ok(Box::new(crate::nodes::text_suffix::TextSuffix)),
        ("experimental.text-trim", 1) => Ok(Box::new(crate::nodes::text_trim::TextTrim)),
        ("experimental.whitespace-collapse", 1) => Ok(Box::new(
            crate::nodes::whitespace_collapse::WhitespaceCollapse,
        )),
        ("text.dictionary-exact", 1) => {
            Ok(Box::new(crate::nodes::dictionary_exact::DictionaryExact))
        }
        ("text.romaji-to-hiragana", 1) => Ok(Box::new(crate::nodes::romaji::RomajiToHiragana)),
        ("text.select-candidates", 1) => Ok(Box::new(
            crate::nodes::candidate_selection::SelectCandidates,
        )),
        ("text.stringify", 1) => Ok(Box::new(crate::nodes::stringify::Stringify)),
        _ => Err(CoreError::new(
            "unknown-kind",
            "guest Node kind/version is not registered",
        )),
    }
}
fn inputs(
    definition: &NodeDefinition,
    input: BTreeMap<String, WireValue>,
) -> Result<Inputs, CoreError> {
    let mut result = Inputs::new();
    for (name, value) in input {
        let port = definition
            .inputs
            .iter()
            .find(|p| p.port.id == name)
            .ok_or_else(|| CoreError::new("guest-input", "undeclared input port"))?;
        let value = match value {
            WireValue::NoOutput {} => OutputValue::NoOutput,
            WireValue::Data { packet } => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                if !port.port.root.accepts(&packet.root) {
                    return Err(CoreError::new(
                        "invalid-root",
                        "input root does not match port",
                    ));
                }
                OutputValue::Data(Arc::new(packet))
            }
        };
        result.insert(name, value);
    }
    if definition
        .inputs
        .iter()
        .any(|p| p.required && !result.contains_key(&p.port.id))
    {
        return Err(CoreError::new(
            "missing-input",
            "required input port is missing",
        ));
    }
    Ok(result)
}
fn outputs(definition: &NodeDefinition, outputs: Outputs) -> Result<Value, CoreError> {
    if outputs.len() != definition.outputs.len()
        || outputs
            .keys()
            .any(|k| !definition.outputs.iter().any(|p| p.id == *k))
    {
        return Err(CoreError::new(
            "guest-output",
            "Node did not return exactly its declared outputs",
        ));
    }
    let mut result = BTreeMap::new();
    for (name, value) in outputs {
        let port = definition.outputs.iter().find(|p| p.id == name).unwrap();
        let value = match value {
            OutputValue::NoOutput => WireValue::NoOutput {},
            OutputValue::Data(packet) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                if !port.root.accepts(&packet.root) {
                    return Err(CoreError::new(
                        "invalid-root",
                        "output root does not match port",
                    ));
                }
                WireValue::Data {
                    packet: (*packet).clone(),
                }
            }
        };
        result.insert(name, value);
    }
    serde_json::to_value(result)
        .map_err(|_| CoreError::new("guest-json", "output serialization failed"))
}
fn dispatch(request: Request) -> Result<Value, CoreError> {
    if request.api_version != API_VERSION {
        return Err(CoreError::new("guest-version", "unsupported guest API"));
    }
    let node = make_node(&request.node.kind, request.node.version)?;
    let definition = node.definition();
    match request.operation {
        Operation::Resume { .. } => Err(CoreError::new(
            "guest-operation",
            "pure Node cannot resume a service operation",
        )),
        Operation::Describe {} => serde_json::to_value(definition)
            .map_err(|_| CoreError::new("guest-json", "definition serialization failed")),
        Operation::Validate { config } => {
            node.validate_config(&config)?;
            Ok(json!({"valid":true}))
        }
        Operation::Prepare { config, resources } => {
            node.validate_config(&config)?;
            let prepared = node.prepare(&config, &mut Snapshots(resources))?;
            Ok(json!({"valid":true,"prepared":prepared.is_some()}))
        }
        Operation::Execute {
            config,
            inputs: supplied,
            mode,
            resources,
        } => {
            node.validate_config(&config)?;
            let inputs = inputs(&definition, supplied)?;
            let prepared = node.prepare(&config, &mut Snapshots(resources))?;
            if definition.inputs.iter().any(|p| {
                p.required && matches!(inputs.get(&p.port.id), Some(OutputValue::NoOutput))
            }) {
                return outputs(
                    &definition,
                    definition
                        .outputs
                        .iter()
                        .map(|p| (p.id.clone(), OutputValue::NoOutput))
                        .collect(),
                );
            }
            // These local diagnostic identifiers confer no Host authority. Only mode is projected.
            let mode = mode.map(|mode| RuntimeInput {
                revision: 1,
                text: String::new(),
                mode,
            });
            let context = RunContext {
                run_id: 1,
                plan_version: 1,
                node_id: "guest",
                input: mode.as_ref(),
            };
            let result = match prepared {
                Some(prepared) => prepared.execute(context, &inputs)?,
                None => node.execute(context, &inputs, &config)?,
            };
            outputs(&definition, result)
        }
    }
}
fn failure(error: CoreError) -> Vec<u8> {
    serde_json::to_vec(&json!({"apiVersion":API_VERSION,"ok":false,"error":error})).unwrap()
}
/// Decode one bounded request; errors never echo raw request bytes.
pub fn handle(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return failure(CoreError::new("guest-limit", "guest message exceeds limit"));
    }
    let result = serde_json::from_slice::<Request>(bytes)
        .map_err(|_| CoreError::new("guest-json", "invalid guest request"))
        .and_then(dispatch);
    match result {
        Ok(data) => {
            match serde_json::to_vec(&json!({"apiVersion":API_VERSION,"ok":true,"data":data})) {
                Ok(bytes) if bytes.len() <= MAX_MESSAGE_BYTES => bytes,
                _ => failure(CoreError::new(
                    "guest-limit",
                    "guest response exceeds limit",
                )),
            }
        }
        Err(error) => failure(error),
    }
}

#[cfg(all(target_arch = "wasm32", not(feature = "custom-guest")))]
mod abi {
    use super::*;
    use std::sync::Mutex;
    static REQUEST: Mutex<Option<Box<[u8]>>> = Mutex::new(None);
    static RESPONSE: Mutex<Option<Box<[u8]>>> = Mutex::new(None);
    #[unsafe(no_mangle)]
    pub extern "C" fn diyime_api_version() -> u32 {
        API_VERSION
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn diyime_alloc(length: u32) -> u32 {
        let length = length as usize;
        if length == 0 || length > MAX_MESSAGE_BYTES {
            return 0;
        }
        let bytes = vec![0; length].into_boxed_slice();
        let pointer = bytes.as_ptr() as u32;
        *REQUEST.lock().unwrap() = Some(bytes);
        pointer
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn diyime_call(pointer: u32, length: u32) -> u64 {
        let bytes = {
            let mut pending = REQUEST.lock().unwrap();
            if !pending
                .as_ref()
                .is_some_and(|b| b.as_ptr() as u32 == pointer && b.len() == length as usize)
            {
                return 0;
            }
            pending.take().unwrap()
        };
        let result = handle(&bytes).into_boxed_slice();
        let descriptor = ((result.len() as u64) << 32) | result.as_ptr() as u64;
        *RESPONSE.lock().unwrap() = Some(result);
        descriptor
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn diyime_release_reply() {
        *RESPONSE.lock().unwrap() = None;
    }
}

#[cfg(feature = "dictionary-artifact")]
pub(crate) fn artifact_inputs(
    definition: &NodeDefinition,
    values: BTreeMap<String, WireValue>,
) -> Result<Inputs, CoreError> {
    inputs(definition, values)
}
#[cfg(feature = "dictionary-artifact")]
pub(crate) fn artifact_outputs(
    definition: &NodeDefinition,
    values: Outputs,
) -> Result<Value, CoreError> {
    outputs(definition, values)
}
