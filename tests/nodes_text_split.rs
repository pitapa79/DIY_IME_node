#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};

#[path = "../src/nodes/text_split/mod.rs"]
mod subject;

use std::sync::Arc;

use crate::test_host::{
    TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext,
    },
    runtime::{Engine, NodeStatus},
};
use serde_json::{Value, json};
use subject::{KIND, TextSplit};

fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "split",
        input: None,
    }
}
fn text(text: &str) -> Candidate {
    Candidate::Text {
        text: text.into(),
        meta: None,
    }
}
fn data(packet: Packet) -> OutputValue {
    OutputValue::Data(Arc::new(packet))
}
fn run(value: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    TextSplit.execute(context(), &[("in".into(), value)].into(), config)
}
fn expected(parts: &[&str]) -> Packet {
    Packet::new(Candidate::Sequence {
        items: parts.iter().map(|part| text(part)).collect(),
    })
}
fn assert_packet(value: &OutputValue, expected: &Packet) {
    let OutputValue::Data(actual) = value else {
        panic!("expected Data")
    };
    assert_eq!(actual.as_ref(), expected);
}

#[test]
fn fixture_examples_match_node() {
    let fixture: Value =
        serde_json::from_str(include_str!("../examples/nodes/text_split/contract.json")).unwrap();
    assert_eq!(fixture["kind"], KIND);
    for case in fixture["cases"].as_array().unwrap() {
        let config = case.get("config").unwrap_or(&fixture["defaultConfig"]);
        let output = run(
            data(Packet::new(text(case["input"].as_str().unwrap()))),
            config,
        )
        .unwrap();
        let parts: Vec<_> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|part| part.as_str().unwrap())
            .collect();
        assert_packet(&output["out"], &expected(&parts));
    }
}

#[test]
fn split_drops_unreliable_metadata_and_preserves_input() {
    let original = Arc::new(Packet::new(Candidate::Text {
        text: "猫,犬".into(),
        meta: Some(TextMeta {
            reading: Some("ねこ,いぬ".into()),
            source_span: Some([2, 5]),
        }),
    }));
    let before = original.as_ref().clone();
    let output = run(
        OutputValue::Data(Arc::clone(&original)),
        &json!({"separator":","}),
    )
    .unwrap();
    assert_packet(&output["out"], &expected(&["猫", "犬"]));
    assert_eq!(original.as_ref(), &before);
}

#[test]
fn no_match_still_returns_sequence_without_metadata() {
    let input = Packet::new(Candidate::Text {
        text: "猫".into(),
        meta: Some(TextMeta {
            reading: Some("ねこ".into()),
            source_span: None,
        }),
    });
    assert_packet(
        &run(data(input), &json!({"separator":","})).unwrap()["out"],
        &expected(&["猫"]),
    );
}

#[test]
fn no_output_and_missing_input_are_distinct() {
    let config = TextSplit.definition().default_config;
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap()["out"],
        OutputValue::NoOutput
    );
    assert_eq!(
        TextSplit
            .execute(context(), &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
}

#[test]
fn invalid_settings_are_rejected_before_input_processing() {
    for config in [
        Value::Null,
        json!([]),
        json!(","),
        json!({}),
        json!({"separator":""}),
        json!({"separator":null}),
        json!({"separator":1}),
        json!({"separator":false}),
        json!({"separator":",","extra":true}),
    ] {
        assert_eq!(
            TextSplit.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            run(OutputValue::NoOutput, &config).unwrap_err().code,
            "invalid-config"
        );
    }
}

#[test]
fn invalid_packet_and_wrong_root_are_rejected() {
    let config = TextSplit.definition().default_config;
    let mut invalid = Packet::new(text("a"));
    invalid.version = 0;
    assert_eq!(
        run(data(invalid), &config).unwrap_err().code,
        "invalid-packet"
    );
    for root in [
        Candidate::Sequence { items: vec![] },
        Candidate::Choice { options: vec![] },
    ] {
        assert_eq!(
            run(data(Packet::new(root)), &config).unwrap_err().code,
            "invalid-root"
        );
    }
}

struct Source(OutputValue);
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.split-source",
            1,
            vec![],
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(RootConstraint::Text),
            ],
        )
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, config: &Value) -> Result<Outputs, CoreError> {
        self.validate_config(config)?;
        Ok([("out".into(), self.0.clone())].into())
    }
}
fn instance(id: &str, kind: &str, config: Option<Value>) -> NodeInstance {
    NodeInstance {
        id: id.into(),
        kind: kind.into(),
        version: 1,
        config,
    }
}
fn graph(config: Option<Value>) -> Graph {
    Graph {
        version: 1,
        nodes: vec![
            instance("source", "test.split-source", None),
            instance("a", KIND, config),
            instance("b", KIND, Some(json!({"separator":"犬"}))),
        ],
        edges: ["a", "b"]
            .into_iter()
            .map(|id| Edge {
                from: Endpoint::new("source", "out"),
                to: Endpoint::new(id, "in"),
            })
            .collect(),
    }
}
#[test]
fn engine_applies_default_config_and_separates_instances() {
    let mut registry = Registry::default();
    registry
        .register(Source(data(Packet::new(text("猫,犬,,🙂,")))))
        .unwrap();
    registry.register(TextSplit).unwrap();
    let mut engine = Engine::new(ExecutionPlan::compile(&graph(None), &registry).unwrap());
    for _ in 0..2 {
        let report = engine.run().unwrap();
        assert!(report.succeeded());
        assert_eq!(
            report.packet("a", "out").unwrap().as_ref(),
            &expected(&["猫", "犬", "", "🙂", ""])
        );
        assert_eq!(
            report.packet("b", "out").unwrap().as_ref(),
            &expected(&["猫,", ",,🙂,"])
        );
    }
}
#[test]
fn engine_skips_no_output_and_rejects_empty_separator_at_prepare() {
    let mut registry = Registry::default();
    registry.register(Source(OutputValue::NoOutput)).unwrap();
    registry.register(TextSplit).unwrap();
    let mut engine = Engine::new(ExecutionPlan::compile(&graph(None), &registry).unwrap());
    let report = engine.run().unwrap();
    assert_eq!(report.nodes["a"].status, NodeStatus::Skipped);
    assert!(report.packet("a", "out").is_none());
    let error = ExecutionPlan::compile(&graph(Some(json!({"separator":""}))), &registry)
        .err()
        .unwrap();
    assert_eq!(error.code, "invalid-config");
}
