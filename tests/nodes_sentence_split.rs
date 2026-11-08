#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
#[path = "../src/nodes/sentence_split/mod.rs"]
mod sentence_split;
use crate::test_host::{
    TextMeta,
    graph::{ExecutionPlan, Graph},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RunContext,
    },
    runtime::{Engine, ResolvedValue},
};
use sentence_split::{KIND, SentenceSplit, split};
use serde_json::{Value, json};
use std::sync::Arc;
struct Source(OutputValue);
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.source",
            1,
            vec![],
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(node::RootConstraint::Text),
            ],
        )
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([("out".into(), self.0.clone())].into())
    }
}
fn run(value: OutputValue, config: Option<Value>) -> crate::test_host::runtime::RunReport {
    let mut registry = Registry::new();
    registry.register(Source(value)).unwrap();
    registry.register(SentenceSplit).unwrap();
    let mut node = json!({"id":"split","kind":KIND,"version":1});
    if let Some(config) = config {
        node["config"] = config;
    }
    let graph=Graph::from_json(&json!({"version":1,"nodes":[{"id":"source","kind":"test.source","version":1},node],"edges":[{"from":{"node":"source","port":"out"},"to":{"node":"split","port":"in"}}]}).to_string()).unwrap();
    Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap())
        .run()
        .unwrap()
}
#[test]
fn standalone_contract_runs_the_actual_node_in_its_own_registry() {
    let contract: Value = serde_json::from_str(include_str!(
        "../examples/nodes/sentence_split/contract.json"
    ))
    .unwrap();
    for case in contract["cases"].as_array().unwrap() {
        let input = Arc::new(Packet::new(Candidate::Text {
            text: case["input"].as_str().unwrap().into(),
            meta: Some(TextMeta {
                reading: Some("元のmeta".into()),
                source_span: Some([0, 1]),
            }),
        }));
        let before = (*input).clone();
        let report = run(
            OutputValue::Data(input.clone()),
            case.get("config").cloned(),
        );
        let Candidate::Sequence { items } = &report.packet("split", "out").unwrap().root else {
            panic!("expected Sequence");
        };
        let actual: Vec<&str> = items
            .iter()
            .map(|item| {
                let Candidate::Text { text, meta } = item else {
                    panic!("expected Text");
                };
                assert!(meta.is_none());
                text.as_str()
            })
            .collect();
        assert_eq!(json!(actual), case["expected"]);
        assert_eq!(actual.concat(), case["input"].as_str().unwrap());
        assert_eq!(*input, before);
        assert!(Arc::ptr_eq(report.packet("source", "out").unwrap(), &input));
    }
}
#[test]
fn exhaustive_short_unicode_strings_preserve_bytes_and_never_add_empty_tail_chunks() {
    let alphabet = ['a', '。', '!', '😀', '\n'];
    for length in 0..=4 {
        for mut code in 0..5usize.pow(length) {
            let mut input = String::new();
            for _ in 0..length {
                input.push(alphabet[code % 5]);
                code /= 5;
            }
            for terminators in ["。！？!?", "", "😀\n", "a。"] {
                let chunks = split(&input, terminators);
                assert_eq!(chunks.concat(), input);
                assert!(!chunks.is_empty());
                if !input.is_empty() {
                    assert!(chunks.iter().all(|chunk| !chunk.is_empty()));
                }
            }
        }
    }
}
#[test]
fn no_output_is_preserved_and_default_config_does_not_leak_between_instances() {
    let report = run(OutputValue::NoOutput, None);
    assert!(matches!(
        report.nodes["split"].outputs["out"],
        ResolvedValue::NoOutput
    ));
    assert_eq!(split("a。b", ""), vec!["a。b"]);
    assert_eq!(split("a。b", "。！？!?"), vec!["a。", "b"]);
}
#[test]
fn invalid_config_input_root_and_packet_are_distinct_errors() {
    for config in [
        Value::Null,
        json!({}),
        json!([]),
        json!({"terminators":true}),
        json!({"terminators":"。","extra":1}),
    ] {
        assert_eq!(
            SentenceSplit.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
    }
    let context = RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "split",
        input: None,
    };
    let config = SentenceSplit.definition().default_config;
    assert_eq!(
        SentenceSplit
            .execute(context, &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    let packet = Packet::new(Candidate::Sequence { items: vec![] });
    let inputs = [("in".into(), OutputValue::Data(Arc::new(packet.clone())))].into();
    assert_eq!(
        SentenceSplit
            .execute(context, &inputs, &config)
            .unwrap_err()
            .code,
        "invalid-root"
    );
    let mut packet = packet;
    packet.version = 2;
    let inputs = [("in".into(), OutputValue::Data(Arc::new(packet)))].into();
    assert_eq!(
        SentenceSplit
            .execute(context, &inputs, &config)
            .unwrap_err()
            .code,
        "invalid-packet"
    );
}
