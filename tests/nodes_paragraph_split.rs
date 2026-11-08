#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
#[path = "../src/nodes/paragraph_split/mod.rs"]
mod paragraph_split;

use crate::test_host::{
    TextMeta,
    graph::{ExecutionPlan, Graph},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RunContext,
    },
    runtime::{Engine, ResolvedValue},
};
use paragraph_split::{KIND, ParagraphSplit, split};
use serde_json::{Value, json};
use std::sync::Arc;

struct Source(OutputValue, &'static str);
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.source",
            1,
            vec![],
            vec![
                Port::packet("out")
                    .with_language(self.1)
                    .with_root(node::RootConstraint::Text),
            ],
        )
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([("out".into(), self.0.clone())].into())
    }
}

fn graph(config: Option<Value>) -> Graph {
    let mut left = json!({"id":"left","kind":KIND,"version":1});
    if let Some(config) = config {
        left["config"] = config;
    }
    Graph::from_json(&json!({
        "version":1,
        "nodes":[{"id":"source","kind":"test.source","version":1},left,{"id":"right","kind":KIND,"version":1}],
        "edges":[
            {"from":{"node":"source","port":"out"},"to":{"node":"left","port":"in"}},
            {"from":{"node":"source","port":"out"},"to":{"node":"right","port":"in"}}
        ]
    }).to_string()).unwrap()
}

fn registry(value: OutputValue, language: &'static str) -> Registry {
    let mut registry = Registry::new();
    registry.register(Source(value, language)).unwrap();
    registry.register(ParagraphSplit).unwrap();
    registry
}

fn text_chunks(packet: &Packet) -> Vec<&str> {
    let Candidate::Sequence { items } = &packet.root else {
        panic!("expected Sequence");
    };
    items
        .iter()
        .map(|item| {
            let Candidate::Text { text, meta } = item else {
                panic!("expected Text");
            };
            assert!(meta.is_none());
            text.as_str()
        })
        .collect()
}

#[test]
fn standalone_contract_uses_real_engine_preserves_input_and_isolates_instances_and_runs() {
    let contract: Value = serde_json::from_str(include_str!(
        "../examples/nodes/paragraph_split/contract.json"
    ))
    .unwrap();
    assert_eq!(contract["kind"], KIND);
    assert_eq!(contract["version"], 1);
    for case in contract["cases"].as_array().unwrap() {
        let input = Arc::new(Packet::new(Candidate::Text {
            text: case["input"].as_str().unwrap().into(),
            meta: Some(TextMeta {
                reading: Some("元のmeta".into()),
                source_span: Some([0, 1]),
            }),
        }));
        let before = (*input).clone();
        let registry = registry(OutputValue::Data(input.clone()), "ja");
        let mut engine = Engine::new(
            ExecutionPlan::compile(&graph(case.get("config").cloned()), &registry).unwrap(),
        );
        for _ in 0..2 {
            let report = engine.run().unwrap();
            for id in ["left", "right"] {
                let packet = report.packet(id, "out").unwrap();
                packet.validate().unwrap();
                let chunks = text_chunks(packet);
                assert_eq!(json!(chunks), case["expected"], "case {case}");
                assert_eq!(chunks.concat(), case["input"].as_str().unwrap());
            }
            assert!(Arc::ptr_eq(report.packet("source", "out").unwrap(), &input));
            assert_eq!(*input, before);
        }
    }
}

#[test]
fn exhaustive_short_unicode_and_mixed_newlines_preserve_bytes_and_split_only_after_separator_runs()
{
    let alphabet = ['a', '\r', '\n', ' ', '😀', '\u{2029}'];
    for length in 0..=5 {
        for mut code in 0..alphabet.len().pow(length) {
            let mut input = String::new();
            for _ in 0..length {
                input.push(alphabet[code % alphabet.len()]);
                code /= alphabet.len();
            }
            let chunks = split(&input);
            assert_eq!(chunks.concat(), input);
            assert!(!chunks.is_empty());
            if !input.is_empty() {
                assert!(chunks.iter().all(|chunk| !chunk.is_empty()));
            }
            for (index, chunk) in chunks.iter().enumerate() {
                // Independent normalized-line view verifies separator placement,
                // while the original-byte assertion above forbids normalization.
                let lines = chunk.replace("\r\n", "\n").replace('\r', "\n");
                if index + 1 < chunks.len() {
                    assert!(lines.ends_with("\n\n"));
                    assert!(!chunks[index + 1].starts_with(['\r', '\n']));
                }
                let body = lines.trim_end_matches('\n');
                assert!(!body.contains("\n\n"));
            }
        }
    }
}

fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "split",
        input: None,
    }
}

#[test]
fn no_output_stays_distinct_from_empty_text_for_engine_and_direct_calls() {
    let registry = registry(OutputValue::NoOutput, "ja");
    let report = Engine::new(ExecutionPlan::compile(&graph(None), &registry).unwrap())
        .run()
        .unwrap();
    for id in ["left", "right"] {
        assert!(matches!(
            report.nodes[id].outputs["out"],
            ResolvedValue::NoOutput
        ));
    }
    let inputs = [("in".into(), OutputValue::NoOutput)].into();
    assert!(matches!(
        ParagraphSplit
            .execute(context(), &inputs, &json!({}))
            .unwrap()["out"],
        OutputValue::NoOutput
    ));
    assert_eq!(split(""), vec![""]);
}

#[test]
fn invalid_config_is_rejected_before_execution_and_language_mismatch_before_running() {
    for config in [
        Value::Null,
        json!([]),
        json!(false),
        json!(1),
        json!(""),
        json!({"extra":true}),
    ] {
        assert_eq!(
            ParagraphSplit.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            ParagraphSplit
                .execute(context(), &Inputs::new(), &config)
                .unwrap_err()
                .code,
            "invalid-config"
        );
        if !config.is_null() {
            let registry = registry(OutputValue::NoOutput, "ja");
            assert!(ExecutionPlan::compile(&graph(Some(config)), &registry).is_err());
        }
    }
    // Graph JSON rejects explicit null before configuration validation.
    assert!(Graph::from_json(&json!({"version":1,"nodes":[{"id":"bad","kind":KIND,"version":1,"config":null}],"edges":[]}).to_string()).is_err());
    let registry = registry(OutputValue::NoOutput, "en");
    assert!(ExecutionPlan::compile(&graph(None), &registry).is_err());
}

#[test]
fn missing_input_invalid_packet_and_wrong_root_have_distinct_errors() {
    assert_eq!(
        ParagraphSplit
            .execute(context(), &Inputs::new(), &json!({}))
            .unwrap_err()
            .code,
        "missing-input"
    );
    for root in [
        Candidate::Sequence { items: vec![] },
        Candidate::Choice { options: vec![] },
    ] {
        let packet = Packet::new(root);
        let inputs = [("in".into(), OutputValue::Data(Arc::new(packet.clone())))].into();
        assert_eq!(
            ParagraphSplit
                .execute(context(), &inputs, &json!({}))
                .unwrap_err()
                .code,
            "invalid-root"
        );
        let mut invalid = packet;
        invalid.version = 2;
        let inputs = [("in".into(), OutputValue::Data(Arc::new(invalid)))].into();
        assert_eq!(
            ParagraphSplit
                .execute(context(), &inputs, &json!({}))
                .unwrap_err()
                .code,
            "invalid-packet"
        );
    }
}
