//! 独立した未採用の候補木操作ノード。契約は専用examplesを参照。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.packet-rebuild";
#[derive(Debug, Clone, Copy, Default)]
pub struct PacketRebuild;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    mode: Mode,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    WrapSequence,
    WrapChoice,
    UnwrapSingle,
}
fn settings(config: &Value) -> Result<Settings, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected object"))?;
    const KEYS: &[&str] = &["mode"];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(CoreError::new(
            "invalid-config",
            "expected complete settings and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;

    Ok(settings)
}
fn copy_candidate(root: &Candidate) -> Candidate {
    crate::tree::map_text(root, |text, meta| Candidate::Text {
        text: text.to_owned(),
        meta: meta.cloned(),
    })
}

fn transform(root: &Candidate, settings: &Settings) -> Result<Option<Candidate>, CoreError> {
    let _ = settings;
    Ok(Some(match settings.mode {
        Mode::WrapSequence => Candidate::Sequence {
            items: vec![copy_candidate(root)],
        },
        Mode::WrapChoice => Candidate::Choice {
            options: vec![crate::ChoiceOption {
                value: copy_candidate(root),
                score: None,
            }],
        },
        Mode::UnwrapSingle => match root {
            Candidate::Sequence { items } if items.len() == 1 => copy_candidate(&items[0]),
            Candidate::Choice { options } if options.len() == 1 => {
                copy_candidate(&options[0].value)
            }
            _ => copy_candidate(root),
        },
    }))
}
impl Node for PacketRebuild {
    fn definition(&self) -> NodeDefinition {
        let input = Port::packet("in")
            .with_language("ja")
            .with_root(RootConstraint::Any);
        let output = Port::packet("out")
            .with_language("ja")
            .with_root(RootConstraint::Any);
        let mut definition =
            NodeDefinition::new(KIND, 1, vec![InputPort::required(input)], vec![output]);
        definition.default_config = json!({"mode":"wrap-sequence"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["mode"],"properties":{"mode":{"type":"string"}}});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        settings(config).map(|_| ())
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let settings = settings(config)?;

        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                match transform(&packet.root, &settings)? {
                    Some(root) => OutputValue::Data(Arc::new(Packet::new(root))),
                    None => OutputValue::NoOutput,
                }
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}
