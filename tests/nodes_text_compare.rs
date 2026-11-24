#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
#[path = "../src/nodes/text_compare/mod.rs"]
mod subject;
use crate::test_host::{
    TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextCompare;
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "test",
        input: None,
    }
}
fn packet(text: &str) -> Packet {
    Packet::new(Candidate::Text {
        text: text.into(),
        meta: None,
    })
}
fn data(text: &str) -> OutputValue {
    OutputValue::Data(Arc::new(packet(text)))
}
fn run(a: OutputValue, b: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    TextCompare.execute(
        context(),
        &[("a".into(), a), ("b".into(), b)].into(),
        config,
    )
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../examples/nodes/text_compare/contract.json")).unwrap()
}
#[test]
fn fixture_examples_match_the_node() {
    let fixture = fixture();
    assert_eq!(fixture["kind"], subject::KIND);
    for case in fixture["cases"].as_array().unwrap() {
        let config = case.get("config").unwrap_or(&fixture["defaultConfig"]);
        let meta = TextMeta {
            reading: Some("subject".into()),
            source_span: Some([0, case["a"].as_str().unwrap().chars().count()]),
        };
        let a_packet = Arc::new(Packet::new(Candidate::Text {
            text: case["a"].as_str().unwrap().into(),
            meta: Some(meta),
        }));
        let outputs = run(
            OutputValue::Data(Arc::clone(&a_packet)),
            data(case["b"].as_str().unwrap()),
            config,
        )
        .unwrap();
        let expected_port = case["expected"].as_str().unwrap();
        let other_port = if expected_port == "matched" {
            "unmatched"
        } else {
            "matched"
        };
        assert_eq!(
            outputs[expected_port],
            OutputValue::Data(Arc::clone(&a_packet)),
            "case={}",
            case["id"]
        );
        assert_eq!(
            outputs[other_port],
            OutputValue::NoOutput,
            "case={}",
            case["id"]
        );
    }
}
#[test]
fn declaration_has_two_required_inputs_two_outputs_and_exact_default() {
    let definition = TextCompare.definition();
    assert_eq!(definition.kind, subject::KIND);
    assert_eq!(definition.default_config, json!({ "mode": "exact" }));
    let inputs: Vec<_> = definition
        .inputs
        .iter()
        .map(|p| p.port.id.as_str())
        .collect();
    assert_eq!(inputs, ["a", "b"]);
    let outputs: Vec<_> = definition.outputs.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(outputs, ["matched", "unmatched"]);
}
#[test]
fn no_output_on_either_input_gives_no_output_on_both_outputs() {
    let config = TextCompare.definition().default_config;
    for (a, b) in [
        (OutputValue::NoOutput, data("x")),
        (data("x"), OutputValue::NoOutput),
        (OutputValue::NoOutput, OutputValue::NoOutput),
    ] {
        let outputs = run(a, b, &config).unwrap();
        assert_eq!(outputs["matched"], OutputValue::NoOutput);
        assert_eq!(outputs["unmatched"], OutputValue::NoOutput);
    }
}
#[test]
fn missing_input_is_reported_even_when_another_is_no_output() {
    let config = TextCompare.definition().default_config;
    for inputs in [
        Inputs::new(),
        [("a".into(), OutputValue::NoOutput)].into(),
        [("b".into(), data("x"))].into(),
    ] {
        assert_eq!(
            TextCompare
                .execute(context(), &inputs, &config)
                .unwrap_err()
                .code,
            "missing-input"
        );
    }
}
#[test]
fn invalid_config_is_rejected_before_processing_inputs() {
    assert_eq!(
        TextCompare
            .execute(context(), &Inputs::new(), &json!({ "mode": "equals" }))
            .unwrap_err()
            .code,
        "invalid-config"
    );
    let config = TextCompare.definition().default_config;
    let mut unknown = config.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("unexpected".into(), json!(true));
    for invalid in [Value::Null, json!({}), json!([]), unknown] {
        assert_eq!(
            TextCompare.validate_config(&invalid).unwrap_err().code,
            "invalid-config"
        );
    }
    for key in config.as_object().unwrap().keys() {
        let mut partial = config.clone();
        partial.as_object_mut().unwrap().remove(key);
        assert_eq!(
            TextCompare.validate_config(&partial).unwrap_err().code,
            "invalid-config"
        );
    }
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            TextCompare.validate_config(invalid).unwrap_err().code,
            "invalid-config"
        );
    }
}
#[test]
fn invalid_packets_and_wrong_roots_are_rejected_on_either_input() {
    let config = TextCompare.definition().default_config;
    let bad_version = OutputValue::Data(Arc::new(Packet {
        version: 99,
        root: Candidate::Text {
            text: "a".into(),
            meta: None,
        },
    }));
    let wrong_root =
        OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items: vec![] })));
    for (a, b, code) in [
        (bad_version.clone(), data("x"), "invalid-packet"),
        (data("x"), bad_version, "invalid-packet"),
        (wrong_root.clone(), data("x"), "invalid-root"),
        (data("x"), wrong_root, "invalid-root"),
        // NoOutput on one side must not hide an invalid Data packet on the other.
        (
            OutputValue::NoOutput,
            OutputValue::Data(Arc::new(Packet {
                version: 99,
                root: Candidate::Text {
                    text: "a".into(),
                    meta: None,
                },
            })),
            "invalid-packet",
        ),
    ] {
        assert_eq!(run(a, b, &config).unwrap_err().code, code);
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
fn registry_and_engine_route_the_subject_packet() {
    let mut registry = Registry::new();
    registry.register(Source("日本語".into())).unwrap();
    registry.register(TextCompare).unwrap();
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
                config: Some(json!({ "mode": "prefix" })),
            },
        ],
        edges: vec![
            Edge {
                from: Endpoint::new("source", "out"),
                to: Endpoint::new("node", "a"),
            },
            Edge {
                from: Endpoint::new("source", "out"),
                to: Endpoint::new("node", "b"),
            },
        ],
    };
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    assert!(report.succeeded());
    assert_eq!(
        report.packet("node", "matched").unwrap().root,
        Candidate::Text {
            text: "日本語".into(),
            meta: None
        }
    );
    assert!(report.packet("node", "unmatched").is_none());
}
