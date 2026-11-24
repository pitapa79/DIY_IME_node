#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, TextMeta, node};
use crate::test_host::{
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::text_regex_search as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextRegexSearch;
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
    let outputs = TextRegexSearch.execute(context(), &[("in".into(), input)].into(), config)?;
    Ok(outputs["out"].clone())
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_regex_search/contract.json"
    ))
    .unwrap()
}
fn expected_packet(case: &Value) -> Packet {
    let items = case["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| Candidate::Text {
            text: item["text"].as_str().unwrap().into(),
            meta: Some(TextMeta {
                reading: None,
                source_span: Some([
                    item["span"][0].as_u64().unwrap() as usize,
                    item["span"][1].as_u64().unwrap() as usize,
                ]),
            }),
        })
        .collect();
    Packet::new(Candidate::Sequence { items })
}
#[test]
fn fixture_cases_match_with_spans() {
    let fixture = fixture();
    assert_eq!(fixture["kind"], subject::KIND);
    for case in fixture["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        // An input meta must not leak into the result items.
        let packet = Arc::new(Packet::new(Candidate::Text {
            text: input.into(),
            meta: Some(TextMeta {
                reading: Some("source".into()),
                source_span: Some([0, input.chars().count()]),
            }),
        }));
        let result = run(OutputValue::Data(packet), &case["config"]).unwrap();
        assert_eq!(
            result,
            OutputValue::Data(Arc::new(expected_packet(case))),
            "case={}",
            case["id"]
        );
    }
}
#[test]
fn definition_and_failures_remain_distinct() {
    let node = TextRegexSearch;
    let definition = node.definition();
    assert_eq!(definition.kind, subject::KIND);
    assert_eq!(definition.outputs[0].root, RootConstraint::Sequence);
    assert_eq!(
        definition.default_config,
        json!({ "pattern": "", "limit": 64 })
    );
    let config = definition.default_config.clone();
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap(),
        OutputValue::NoOutput
    );
    assert_eq!(
        node.execute(context(), &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    assert_eq!(
        run(
            OutputValue::Data(Arc::new(Packet::new(Candidate::Choice { options: vec![] }))),
            &config
        )
        .unwrap_err()
        .code,
        "invalid-root"
    );
    assert_eq!(
        node.validate_config(&json!({ "pattern": "(", "limit": 64 }))
            .unwrap_err()
            .code,
        "invalid-pattern"
    );
    assert_eq!(
        run(data("abc"), &json!({ "pattern": "(", "limit": 64 }))
            .unwrap_err()
            .code,
        "invalid-pattern"
    );
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            node.validate_config(invalid).unwrap_err().code,
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
    let fixtures = fixture();
    let case = &fixtures["cases"][0];
    let mut registry = Registry::new();
    registry
        .register(Source(case["input"].as_str().unwrap().into()))
        .unwrap();
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
                config: Some(case["config"].clone()),
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
        report.packet("node", "out").unwrap().as_ref(),
        &expected_packet(case)
    );
}
