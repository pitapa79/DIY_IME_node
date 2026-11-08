#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, ChoiceOption, Packet, node, tree};

#[path = "../src/nodes/choice_build/mod.rs"]
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
use subject::{ChoiceBuild, KIND};

fn text(value: &str) -> Candidate {
    Candidate::Text {
        text: value.into(),
        meta: None,
    }
}
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "choice",
        input: None,
    }
}
fn data(root: Candidate) -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(root)))
}
fn option(root: Candidate, score: Option<f64>) -> ChoiceOption {
    ChoiceOption { value: root, score }
}
fn execute(inputs: &Inputs, config: &Value) -> Result<Outputs, CoreError> {
    ChoiceBuild.execute(context(), inputs, config)
}

#[test]
fn matches_all_independent_contract_cases() {
    let fixture: Value =
        serde_json::from_str(include_str!("../examples/nodes/choice_build/contract.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 18);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let mut inputs = Inputs::new();
        if let Some(values) = case.get("inputs") {
            for (port, value) in values.as_object().unwrap() {
                let value = if value["status"] == "no-output" {
                    OutputValue::NoOutput
                } else {
                    OutputValue::Data(Arc::new(
                        serde_json::from_value(value["packet"].clone()).unwrap(),
                    ))
                };
                inputs.insert(port.clone(), value);
            }
        }
        let result = execute(
            &inputs,
            case.get("config").unwrap_or(&fixture["defaultConfig"]),
        );
        match case["expected"]["status"].as_str().unwrap() {
            "data" => {
                let result = result.unwrap_or_else(|error| panic!("{id}: {error}"));
                let OutputValue::Data(packet) = &result["out"] else {
                    panic!("{id}: expected Data")
                };
                let expected: Packet =
                    serde_json::from_value(case["expected"]["packet"].clone()).unwrap();
                assert_eq!(packet.as_ref(), &expected, "{id}");
                packet.validate().unwrap();
            }
            "error" => assert_eq!(result.unwrap_err().code, case["expected"]["code"], "{id}"),
            other => panic!("unknown fixture status: {other}"),
        }
    }
}

#[test]
fn rejects_non_objects_null_scores_and_nonfinite_inputs() {
    for config in [
        json!(null),
        json!([]),
        json!(0),
        json!(false),
        json!(""),
        json!({}),
        json!({"options":null}),
        json!({"options":[],"unknown":0}),
    ] {
        assert_eq!(
            ChoiceBuild.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            execute(&Inputs::new(), &config).unwrap_err().code,
            "invalid-config"
        );
    }
    for score in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        // serde_json represents nonfinite floats as null; null scores are rejected too.
        let config = json!({"options": [{"value":{"type":"text","text":"x"},"score":score}]});
        assert_eq!(
            ChoiceBuild.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        let invalid = Candidate::Choice {
            options: vec![option(text("x"), Some(score))],
        };
        let inputs = [("a".into(), data(invalid))].into();
        assert_eq!(
            execute(&inputs, &json!({"options":[]})).unwrap_err().code,
            "invalid-packet"
        );
    }
}

#[test]
fn preserves_input_metadata_scores_nested_choices_and_reproducibility() {
    let root = Candidate::Choice {
        options: vec![
            option(
                Candidate::Text {
                    text: "𠮷👩‍💻e\u{301}".into(),
                    meta: Some(TextMeta {
                        reading: Some("よし".into()),
                        source_span: Some([2, 7]),
                    }),
                },
                Some(-3.5),
            ),
            option(Candidate::Sequence { items: vec![] }, None),
        ],
    };
    let packet = Arc::new(Packet::new(root));
    let before = packet.to_json().unwrap();
    let config = json!({"options":[{"value":{"type":"text","text":"seed"},"score":1.25}]});
    let before_config = config.clone();
    let inputs = [
        ("a".into(), OutputValue::Data(packet.clone())),
        ("b".into(), data(text(""))),
    ]
    .into();
    for _ in 0..3 {
        let result = execute(&inputs, &config).unwrap();
        let OutputValue::Data(output) = &result["out"] else {
            panic!("expected Data")
        };
        let Candidate::Choice { options } = &output.root else {
            panic!("expected Choice")
        };
        assert_eq!(options.len(), 3);
        assert_eq!(options[0], option(text("seed"), Some(1.25)));
        assert_eq!(options[1], option(packet.root.clone(), None));
        assert_eq!(options[2], option(text(""), None));
        assert_eq!(packet.to_json().unwrap(), before);
        assert_eq!(config, before_config);
    }
}

fn drop_packet_iteratively(packet: Packet) {
    let mut pending = vec![packet.root];
    while let Some(root) = pending.pop() {
        match root {
            Candidate::Sequence { items } => pending.extend(items),
            Candidate::Choice { options } => {
                pending.extend(options.into_iter().map(|option| option.value))
            }
            Candidate::Text { .. } => {}
        }
    }
}

#[test]
fn copies_deep_input_iteratively_without_flattening() {
    let depth = 4_000;
    let mut root = text("終");
    for _ in 0..depth {
        root = Candidate::Sequence { items: vec![root] };
    }
    let input = Arc::new(Packet::new(root));
    let inputs = [("a".into(), OutputValue::Data(input.clone()))].into();
    let mut result = execute(&inputs, &json!({"options":[]})).unwrap();
    let OutputValue::Data(output) = result.remove("out").unwrap() else {
        panic!("expected Data")
    };
    let Candidate::Choice { options } = &output.root else {
        panic!("expected Choice")
    };
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].score, None);
    let mut cursor = &options[0].value;
    for _ in 0..depth {
        let Candidate::Sequence { items } = cursor else {
            panic!("sequence structure lost")
        };
        assert_eq!(items.len(), 1);
        cursor = &items[0];
    }
    assert_eq!(cursor, &text("終"));
    drop(inputs);
    drop_packet_iteratively(Arc::try_unwrap(output).unwrap());
    drop_packet_iteratively(Arc::try_unwrap(input).unwrap());
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
            vec![Port::packet("out").with_language("ja")],
        )
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
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

#[test]
fn runs_in_its_own_registry_with_optional_sources_and_default_empty_choice() {
    for source_a in [data(text("a")), OutputValue::NoOutput] {
        let mut registry = Registry::new();
        registry.register(ChoiceBuild).unwrap();
        registry
            .register(Source {
                kind: "test.choice-a",
                value: source_a.clone(),
            })
            .unwrap();
        registry
            .register(Source {
                kind: "test.choice-b",
                value: data(text("b")),
            })
            .unwrap();
        let graph = Graph {
            version: 1,
            nodes: vec![
                instance("a", "test.choice-a", None),
                instance("b", "test.choice-b", None),
                instance(
                    "build",
                    KIND,
                    Some(json!({"options":[{"value":{"type":"text","text":"seed"},"score":2.0}]})),
                ),
                instance("empty", KIND, None),
            ],
            edges: ["a", "b"]
                .into_iter()
                .map(|port| Edge {
                    from: Endpoint::new(port, "out"),
                    to: Endpoint::new("build", port),
                })
                .collect(),
        };
        let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
        for _ in 0..2 {
            let report = engine.run().unwrap();
            assert!(report.succeeded());
            assert_eq!(report.nodes["build"].status, NodeStatus::Executed);
            assert_eq!(report.nodes["empty"].status, NodeStatus::Executed);
            assert_eq!(
                report.packet("empty", "out").unwrap().root,
                Candidate::Choice { options: vec![] }
            );
            let mut expected = vec![option(text("seed"), Some(2.0))];
            if matches!(source_a, OutputValue::Data(_)) {
                expected.push(option(text("a"), None));
            }
            expected.push(option(text("b"), None));
            assert_eq!(
                report.packet("build", "out").unwrap().root,
                Candidate::Choice { options: expected }
            );
        }
    }
}

#[test]
fn declares_optional_any_inputs_and_choice_output() {
    let definition = ChoiceBuild.definition();
    assert_eq!(definition.kind, KIND);
    assert_eq!(definition.version, 1);
    assert_eq!(definition.default_config, json!({"options":[]}));
    assert_eq!(definition.inputs.len(), 2);
    assert_eq!(definition.outputs.len(), 1);
    for (input, id) in definition.inputs.iter().zip(["a", "b"]) {
        assert_eq!(input.port.id, id);
        assert!(!input.required);
        assert_eq!(input.port.root, RootConstraint::Any);
        assert_eq!(input.port.language.as_deref(), Some("ja"));
    }
    assert_eq!(definition.outputs[0].id, "out");
    assert_eq!(definition.outputs[0].root, RootConstraint::Choice);
    assert_eq!(definition.outputs[0].language.as_deref(), Some("ja"));
}
