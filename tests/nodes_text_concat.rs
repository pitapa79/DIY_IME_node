#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};

#[path = "../src/nodes/text_concat/mod.rs"]
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
use subject::{KIND, TextConcat};

fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "concat",
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
fn run(a: OutputValue, b: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    TextConcat.execute(
        context(),
        &[("a".into(), a), ("b".into(), b)].into(),
        config,
    )
}
fn assert_packet(value: &OutputValue, expected: &Packet) {
    let OutputValue::Data(actual) = value else {
        panic!("expected Data, got {value:?}")
    };
    assert_eq!(actual.as_ref(), expected);
}

#[test]
fn fixture_examples_match_the_node() {
    let fixture: Value =
        serde_json::from_str(include_str!("../examples/nodes/text_concat/contract.json")).unwrap();
    assert_eq!(fixture["kind"], KIND);
    for case in fixture["cases"].as_array().unwrap() {
        let config = case.get("config").unwrap_or(&fixture["defaultConfig"]);
        let output = run(
            data(packet(case["a"].as_str().unwrap())),
            data(packet(case["b"].as_str().unwrap())),
            config,
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
        assert_packet(&output["out"], &packet(case["expected"].as_str().unwrap()));
    }
}

#[test]
fn declaration_has_required_text_ports_and_empty_separator_default() {
    let definition = TextConcat.definition();
    assert_eq!(definition.kind, KIND);
    assert_eq!(definition.version, 1);
    assert_eq!(definition.inputs.len(), 2);
    for (input, id) in definition.inputs.iter().zip(["a", "b"]) {
        assert_eq!(input.port.id, id);
        assert!(input.required);
        assert_eq!(input.port.root, RootConstraint::Text);
        assert_eq!(input.port.language.as_deref(), Some("ja"));
    }
    assert_eq!(definition.outputs.len(), 1);
    assert_eq!(definition.outputs[0].id, "out");
    assert_eq!(definition.outputs[0].root, RootConstraint::Text);
    assert_eq!(definition.outputs[0].language.as_deref(), Some("ja"));
    assert_eq!(definition.default_config, json!({"separator":""}));
    TextConcat
        .validate_config(&definition.default_config)
        .unwrap();
}

#[test]
fn output_always_drops_metadata_and_preserves_both_inputs() {
    for (left, right) in [("猫", "犬"), ("", "")] {
        let original_a = Arc::new(Packet::new(Candidate::Text {
            text: left.into(),
            meta: Some(TextMeta {
                reading: Some("ねこ".into()),
                source_span: Some([0, left.chars().count()]),
            }),
        }));
        let original_b = Arc::new(Packet::new(Candidate::Text {
            text: right.into(),
            meta: Some(TextMeta {
                reading: Some("いぬ".into()),
                source_span: Some([4, 4 + right.chars().count()]),
            }),
        }));
        let before_a = original_a.as_ref().clone();
        let before_b = original_b.as_ref().clone();
        let output = run(
            OutputValue::Data(Arc::clone(&original_a)),
            OutputValue::Data(Arc::clone(&original_b)),
            &json!({"separator":""}),
        )
        .unwrap();
        assert_packet(&output["out"], &packet(&format!("{left}{right}")));
        assert_eq!(original_a.as_ref(), &before_a);
        assert_eq!(original_b.as_ref(), &before_b);
    }
}

#[test]
fn no_output_on_either_input_is_not_empty_text() {
    let config = json!({"separator":","});
    for (a, b) in [
        (OutputValue::NoOutput, data(packet("b"))),
        (data(packet("a")), OutputValue::NoOutput),
        (OutputValue::NoOutput, OutputValue::NoOutput),
    ] {
        assert_eq!(run(a, b, &config).unwrap()["out"], OutputValue::NoOutput);
    }
    assert_packet(
        &run(data(packet("")), data(packet("")), &config).unwrap()["out"],
        &packet(","),
    );
}

#[test]
fn missing_input_is_reported_even_when_another_is_no_output() {
    let config = TextConcat.definition().default_config;
    for inputs in [
        Inputs::new(),
        [("a".into(), OutputValue::NoOutput)].into(),
        [("b".into(), OutputValue::NoOutput)].into(),
    ] {
        assert_eq!(
            TextConcat
                .execute(context(), &inputs, &config)
                .unwrap_err()
                .code,
            "missing-input"
        );
    }
}

#[test]
fn rejects_invalid_config_before_processing_inputs() {
    for config in [
        Value::Null,
        json!([]),
        json!(""),
        json!({}),
        json!({"separator":null}),
        json!({"separator":1}),
        json!({"separator":false}),
        json!({"separator":[]}),
        json!({"separator":"" ,"extra":true}),
    ] {
        assert_eq!(
            TextConcat.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            run(OutputValue::NoOutput, OutputValue::NoOutput, &config)
                .unwrap_err()
                .code,
            "invalid-config"
        );
    }
}

#[test]
fn rejects_invalid_packets_and_wrong_roots_on_either_input() {
    let config = TextConcat.definition().default_config;
    for input in ["a", "b"] {
        let mut invalid_version = packet("a");
        invalid_version.version = 999;
        let invalid_meta = Packet::new(Candidate::Text {
            text: "a".into(),
            meta: Some(TextMeta {
                reading: None,
                source_span: Some([4, 2]),
            }),
        });
        for invalid in [invalid_version, invalid_meta] {
            let mut inputs: Inputs = [
                ("a".into(), data(packet("a"))),
                ("b".into(), data(packet("b"))),
            ]
            .into();
            inputs.insert(input.into(), data(invalid));
            assert_eq!(
                TextConcat
                    .execute(context(), &inputs, &config)
                    .unwrap_err()
                    .code,
                "invalid-packet"
            );
        }
        for root in [
            Candidate::Sequence { items: vec![] },
            Candidate::Choice { options: vec![] },
        ] {
            let mut inputs: Inputs = [
                ("a".into(), data(packet("a"))),
                ("b".into(), data(packet("b"))),
            ]
            .into();
            inputs.insert(input.into(), data(Packet::new(root)));
            assert_eq!(
                TextConcat
                    .execute(context(), &inputs, &config)
                    .unwrap_err()
                    .code,
                "invalid-root"
            );
        }
    }
}

#[test]
fn no_output_does_not_hide_an_invalid_supplied_data_packet() {
    let mut invalid = packet("x");
    invalid.version = 999;
    for (a, b) in [
        (OutputValue::NoOutput, data(invalid.clone())),
        (data(invalid), OutputValue::NoOutput),
    ] {
        assert_eq!(
            run(a, b, &json!({"separator":""})).unwrap_err().code,
            "invalid-packet"
        );
    }
}

#[test]
fn no_implicit_runtime_input_length_limit_is_added() {
    let left = "猫".repeat(5000);
    let right = "🙂".repeat(5000);
    let output = run(
        data(packet(&left)),
        data(packet(&right)),
        &json!({"separator":"-"}),
    )
    .unwrap();
    assert_packet(&output["out"], &packet(&format!("{left}-{right}")));
}

struct Source {
    kind: &'static str,
    value: OutputValue,
}
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            self.kind,
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
        Ok([("out".into(), self.value.clone())].into())
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
fn graph(default_config: Option<Value>) -> Graph {
    let concat_nodes = ["default", "dash", "unicode"];
    let nodes = vec![
        instance("a", "test.concat-a", None),
        instance("b", "test.concat-b", None),
        instance("default", KIND, default_config),
        instance("dash", KIND, Some(json!({"separator":"-"}))),
        instance("unicode", KIND, Some(json!({"separator":"🙂"}))),
    ];
    let mut edges = Vec::new();
    for concat in concat_nodes {
        for input in ["a", "b"] {
            edges.push(Edge {
                from: Endpoint::new(input, "out"),
                to: Endpoint::new(concat, input),
            });
        }
    }
    Graph {
        version: 1,
        nodes,
        edges,
    }
}
fn registry(a: OutputValue, b: OutputValue) -> Registry {
    let mut registry = Registry::default();
    registry
        .register(Source {
            kind: "test.concat-a",
            value: a,
        })
        .unwrap();
    registry
        .register(Source {
            kind: "test.concat-b",
            value: b,
        })
        .unwrap();
    registry.register(TextConcat).unwrap();
    registry
}

#[test]
fn engine_uses_defaults_and_keeps_configured_instances_independent() {
    let registry = registry(data(packet("猫")), data(packet("犬")));
    let mut engine = Engine::new(ExecutionPlan::compile(&graph(None), &registry).unwrap());
    for _ in 0..3 {
        let report = engine.run().unwrap();
        assert!(report.succeeded());
        for (id, expected) in [
            ("default", "猫犬"),
            ("dash", "猫-犬"),
            ("unicode", "猫🙂犬"),
        ] {
            assert_eq!(
                report.packet(id, "out").unwrap().as_ref(),
                &packet(expected)
            );
        }
    }
}

#[test]
fn engine_skips_required_no_output_on_either_input() {
    for (a, b) in [
        (OutputValue::NoOutput, data(packet("b"))),
        (data(packet("a")), OutputValue::NoOutput),
        (OutputValue::NoOutput, OutputValue::NoOutput),
    ] {
        let registry = registry(a, b);
        let mut engine = Engine::new(ExecutionPlan::compile(&graph(None), &registry).unwrap());
        let report = engine.run().unwrap();
        for id in ["default", "dash", "unicode"] {
            assert_eq!(report.nodes[id].status, NodeStatus::Skipped);
            assert!(report.packet(id, "out").is_none());
        }
    }
}

#[test]
fn prepare_rejects_explicit_missing_setting_and_unconnected_input() {
    let registry = registry(data(packet("a")), data(packet("b")));
    let error = ExecutionPlan::compile(&graph(Some(json!({}))), &registry)
        .err()
        .unwrap();
    assert_eq!(error.code, "invalid-config");
    let mut missing = graph(None);
    missing
        .edges
        .retain(|edge| edge.to.node != "default" || edge.to.port != "b");
    let error = ExecutionPlan::compile(&missing, &registry).err().unwrap();
    assert_eq!(error.code, "missing-input");
}
