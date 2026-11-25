//! 未採用の実験ノード。契約はexamples/nodes/text_script_split/README.md。
use crate::{
    Candidate, Packet, TextMeta,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-script-split";
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
fn split(text: &str) -> Vec<(String, [usize; 2])> {
    let mut runs: Vec<(Class, String, usize)> = Vec::new();
    let mut previous: Option<Class> = None;
    for (index, c) in text.chars().enumerate() {
        let class = if is_combining(c) {
            previous.unwrap_or(Class::Other)
        } else {
            base_class(c)
        };
        previous = Some(class);
        match runs.last_mut() {
            Some((last_class, run_text, _)) if *last_class == class => run_text.push(c),
            _ => runs.push((class, c.to_string(), index)),
        }
    }
    runs.into_iter()
        .map(|(_, run_text, start)| {
            let end = start + run_text.chars().count();
            (run_text, [start, end])
        })
        .collect()
}
#[derive(Debug, Clone, Copy, Default)]
pub struct TextScriptSplit;
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
impl Node for TextScriptSplit {
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
                    .with_root(RootConstraint::Sequence),
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
                let items = split(text)
                    .into_iter()
                    .map(|(run_text, span)| Candidate::Text {
                        text: run_text,
                        meta: Some(TextMeta {
                            reading: None,
                            source_span: Some(span),
                        }),
                    })
                    .collect();
                OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
