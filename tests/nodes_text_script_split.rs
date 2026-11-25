#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::text_script_split as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextScriptSplit;
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "test",
        input: None,
    }
}
fn data(text: &str) -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
        text: text.into(),
        meta: None,
    })))
}
fn run(input: OutputValue, config: &Value) -> Result<OutputValue, CoreError> {
    let outputs = TextScriptSplit.execute(context(), &[("in".into(), input)].into(), config)?;
    Ok(outputs["out"].clone())
}
fn items(input: &str) -> Vec<(String, Option<[usize; 2]>)> {
    let OutputValue::Data(packet) = run(data(input), &json!({})).unwrap() else {
        panic!("Data required")
    };
    let Candidate::Sequence { items } = &packet.root else {
        panic!("Sequence required")
    };
    items
        .iter()
        .map(|item| {
            let Candidate::Text { text, meta } = item else {
                panic!("Text items required")
            };
            assert!(meta.as_ref().is_none_or(|m| m.reading.is_none()));
            (text.clone(), meta.as_ref().and_then(|m| m.source_span))
        })
        .collect()
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_script_split/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_values_and_spans_are_correct() {
    for case in fixture()["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let result = items(input);
        let expected: Vec<(String, Option<[usize; 2]>)> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                let span = item["span"].as_array().unwrap();
                (
                    item["text"].as_str().unwrap().to_string(),
                    Some([
                        span[0].as_u64().unwrap() as usize,
                        span[1].as_u64().unwrap() as usize,
                    ]),
                )
            })
            .collect();
        assert_eq!(result, expected, "input={input:?}");
        // Concatenating the runs reproduces the input exactly.
        let joined: String = result.iter().map(|(text, _)| text.as_str()).collect();
        assert_eq!(joined, input);
    }
}
#[test]
fn combining_marks_never_split_a_run() {
    let input = "か\u{3099}く";
    assert_eq!(items(input), vec![(input.to_string(), Some([0, 3]))]);
    // A leading combining mark forms its own "other" run.
    let leading = "\u{3099}か";
    assert_eq!(
        items(leading),
        vec![
            ("\u{3099}".to_string(), Some([0, 1])),
            ("か".to_string(), Some([1, 2])),
        ]
    );
}
#[test]
fn katakana_block_punctuation_forms_an_other_run() {
    assert_eq!(
        items("カ・ナ"),
        vec![
            ("カ".to_string(), Some([0, 1])),
            ("・".to_string(), Some([1, 2])),
            ("ナ".to_string(), Some([2, 3])),
        ]
    );
}
#[test]
fn failures_remain_distinct() {
    let config = TextScriptSplit.definition().default_config;
    assert_eq!(config, json!({}));
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap(),
        OutputValue::NoOutput
    );
    assert_eq!(
        TextScriptSplit
            .execute(context(), &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    assert_eq!(
        run(
            OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items: vec![] }))),
            &config
        )
        .unwrap_err()
        .code,
        "invalid-root"
    );
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            TextScriptSplit.validate_config(invalid).unwrap_err().code,
            "invalid-config",
            "{invalid}"
        );
    }
}
struct Source(String);
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.source",
            1,
            vec![],
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(RootConstraint::Text),
            ],
        )
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([("out".into(), data(&self.0))].into())
    }
}
#[test]
fn registry_and_engine_use_the_actual_node() {
    let mut registry = Registry::new();
    registry.register(Source("abc123".into())).unwrap();
    diy_ime2_nodes::opt_in::register(&mut registry).unwrap();
    let graph = Graph {
        version: 1,
        nodes: vec![
            NodeInstance {
                id: "source".into(),
                kind: "test.source".into(),
                version: 1,
                config: None,
            },
            NodeInstance {
                id: "node".into(),
                kind: subject::KIND.into(),
                version: 1,
                config: Some(json!({})),
            },
        ],
        edges: vec![Edge {
            from: Endpoint::new("source", "out"),
            to: Endpoint::new("node", "in"),
        }],
    };
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    assert!(report.succeeded());
    assert_eq!(
        report.packet("node", "out").unwrap().root,
        Candidate::Sequence {
            items: vec![
                Candidate::Text {
                    text: "abc".into(),
                    meta: Some(TextMeta {
                        reading: None,
                        source_span: Some([0, 3]),
                    }),
                },
                Candidate::Text {
                    text: "123".into(),
                    meta: Some(TextMeta {
                        reading: None,
                        source_span: Some([3, 6]),
                    }),
                },
            ],
        }
    );
}
