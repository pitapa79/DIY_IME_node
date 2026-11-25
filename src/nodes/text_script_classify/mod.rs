//! 未採用の実験ノード。契約はexamples/nodes/text_script_classify/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-script-classify";
#[derive(Debug, Clone, Copy, PartialEq)]
enum Class {
    Hiragana,
    Katakana,
    Kanji,
    Latin,
    Digit,
    Other,
}
/// 結合文字・バリエーションセレクタは直前の文字のクラスを引き継ぐ。
fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x20D0..=0x20F0 | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0xE0100..=0xE01EF)
        || c == '\u{3099}'
        || c == '\u{309a}'
        || c == '\u{309b}'
        || c == '\u{309c}'
}
fn base_class(c: char) -> Class {
    match c as u32 {
        0x30A0 | 0x30FB => Class::Other,
        0x3040..=0x309F => Class::Hiragana,
        0x30A0..=0x30FF | 0xFF66..=0xFF9D => Class::Katakana,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0x3005 | 0x3006 => Class::Kanji,
        0x41..=0x5A | 0x61..=0x7A | 0xFF21..=0xFF3A | 0xFF41..=0xFF5A => Class::Latin,
        0x30..=0x39 | 0xFF10..=0xFF19 => Class::Digit,
        _ => Class::Other,
    }
}
fn classify(text: &str) -> &'static str {
    if text.is_empty() {
        return "empty";
    }
    let mut found: Vec<Class> = Vec::new();
    let mut previous: Option<Class> = None;
    for c in text.chars() {
        let class = if is_combining(c) {
            previous.unwrap_or(Class::Other)
        } else {
            base_class(c)
        };
        previous = Some(class);
        if class != Class::Other && !found.contains(&class) {
            found.push(class);
        }
    }
    match found.len() {
        0 => "neutral",
        1 => match found[0] {
            Class::Hiragana => "hiragana",
            Class::Katakana => "katakana",
            Class::Kanji => "kanji",
            Class::Latin => "latin",
            Class::Digit => "digit",
            Class::Other => unreachable!(),
        },
        _ => "mixed",
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct TextScriptClassify;
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
impl Node for TextScriptClassify {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Text)
        };
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(port("in"))],
            vec![port("out")],
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
                // The label is a new fact about the input: never inherit meta.
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: classify(text).to_string(),
                    meta: None,
                })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
