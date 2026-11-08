#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};

#[path = "../src/nodes/text_replace/mod.rs"]
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
use subject::{KIND, TextReplace};

fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "replace",
        input: None,
    }
}
fn packet(text: &str) -> Packet {
    Packet::new(Candidate::Text {
        text: text.into(),
        meta: None,
    })
}
fn data(packet: Packet) -> OutputValue {
    OutputValue::Data(Arc::new(packet))
}
fn run(value: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    TextReplace.execute(context(), &[("in".into(), value)].into(), config)
}
fn assert_packet(value: &OutputValue, expected: &Packet) {
    let OutputValue::Data(actual) = value else {
        panic!("expected Data")
    };
    assert_eq!(actual.as_ref(), expected);
}

#[test]
fn fixture_examples_match() {
    let fixture: Value =
        serde_json::from_str(include_str!("../examples/nodes/text_replace/contract.json")).unwrap();
    assert_eq!(fixture["kind"], KIND);
    for case in fixture["cases"].as_array().unwrap() {
        let config = case.get("config").unwrap_or(&fixture["defaultConfig"]);
        let output = run(data(packet(case["input"].as_str().unwrap())), config).unwrap();
        assert_packet(&output["out"], &packet(case["expected"].as_str().unwrap()));
    }
}

#[test]
fn changed_text_drops_metadata_without_mutating_input() {
    let original = Arc::new(Packet::new(Candidate::Text {
        text: "猫ねこ".into(),
        meta: Some(TextMeta {
            reading: Some("ねこねこ".into()),
            source_span: Some([2, 5]),
        }),
    }));
    let before = original.as_ref().clone();
    let output = run(
        OutputValue::Data(Arc::clone(&original)),
        &json!({"from":"猫","to":"犬"}),
    )
    .unwrap();
    assert_packet(&output["out"], &packet("犬ねこ"));
    assert_eq!(original.as_ref(), &before);
}

#[test]
fn unchanged_text_retains_metadata_and_shares_packet() {
    for config in [json!({"from":"x","to":"y"}), json!({"from":"猫","to":"猫"})] {
        let original = Arc::new(Packet::new(Candidate::Text {
            text: "猫".into(),
            meta: Some(TextMeta {
                reading: Some("ねこ".into()),
                source_span: Some([0, 1]),
            }),
        }));
        let output = run(OutputValue::Data(Arc::clone(&original)), &config).unwrap();
        let OutputValue::Data(actual) = &output["out"] else {
            panic!("expected Data")
        };
        assert!(Arc::ptr_eq(&original, actual));
    }
}

#[test]
fn no_output_and_missing_input_are_distinct() {
    let config = TextReplace.definition().default_config;
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap()["out"],
        OutputValue::NoOutput
    );
    assert_eq!(
        TextReplace
            .execute(context(), &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
}

#[test]
fn rejects_invalid_configuration_even_without_data() {
    for config in [
        Value::Null,
        json!([]),
        json!("text"),
        json!({}),
        json!({"from":"a"}),
        json!({"to":"b"}),
        json!({"from":"","to":""}),
        json!({"from":null,"to":"b"}),
        json!({"from":"a","to":null}),
        json!({"from":1,"to":"b"}),
        json!({"from":"a","to":false}),
        json!({"from":"a","to":"b","extra":1}),
    ] {
        assert_eq!(
            TextReplace.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            run(OutputValue::NoOutput, &config).unwrap_err().code,
            "invalid-config"
        );
    }
}

#[test]
fn rejects_invalid_packet_and_non_text_roots() {
    let config = TextReplace.definition().default_config;
    let mut invalid = packet("a");
    invalid.version = 999;
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
    let invalid_meta = Packet::new(Candidate::Text {
        text: "a".into(),
        meta: Some(TextMeta {
            reading: None,
            source_span: Some([4, 2]),
        }),
    });
    assert_eq!(
        run(data(invalid_meta), &config).unwrap_err().code,
        "invalid-packet"
    );
}

struct Source(OutputValue);
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.replace-source",
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
fn node_instance(id: &str, kind: &str, config: Option<Value>) -> NodeInstance {
    NodeInstance {
        id: id.into(),
        kind: kind.into(),
        version: 1,
        config,
    }
}
fn graph(config_a: Option<Value>, config_b: Option<Value>) -> Graph {
    Graph {
        version: 1,
        nodes: vec![
            node_instance("source", "test.replace-source", None),
            node_instance("a", KIND, config_a),
            node_instance("b", KIND, config_b),
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
fn engine_uses_defaults_and_keeps_instances_independent() {
    let mut registry = Registry::default();
    registry
        .register(Source(data(packet("置換前🙂置換前"))))
        .unwrap();
    registry.register(TextReplace).unwrap();
    let graph = graph(None, Some(json!({"from":"置換前","to":"別"})));
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    for _ in 0..2 {
        let report = engine.run().unwrap();
        assert!(report.succeeded());
        assert_eq!(
            report.packet("a", "out").unwrap().as_ref(),
            &packet("置換後🙂置換後")
        );
        assert_eq!(
            report.packet("b", "out").unwrap().as_ref(),
            &packet("別🙂別")
        );
    }
}
#[test]
fn engine_skips_required_no_output_and_rejects_partial_config() {
    let mut registry = Registry::default();
    registry.register(Source(OutputValue::NoOutput)).unwrap();
    registry.register(TextReplace).unwrap();
    let mut engine = Engine::new(ExecutionPlan::compile(&graph(None, None), &registry).unwrap());
    let report = engine.run().unwrap();
    assert_eq!(report.nodes["a"].status, NodeStatus::Skipped);
    assert!(report.packet("a", "out").is_none());
    let error = ExecutionPlan::compile(&graph(Some(json!({"from":"a"})), None), &registry)
        .err()
        .unwrap();
    assert_eq!(error.code, "invalid-config");
}
