#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, ChoiceOption, Packet, node, tree};

#[path = "../src/nodes/choice_merge/mod.rs"]
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
use subject::{ChoiceMerge, KIND};

fn text(value: &str) -> Candidate {
    Candidate::Text {
        text: value.into(),
        meta: None,
    }
}
fn option(value: Candidate, score: Option<f64>) -> ChoiceOption {
    ChoiceOption { value, score }
}
fn choice(options: Vec<ChoiceOption>) -> Candidate {
    Candidate::Choice { options }
}
fn data(root: Candidate) -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(root)))
}
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "merge",
        input: None,
    }
}
fn execute(inputs: &Inputs, config: &Value) -> Result<Outputs, CoreError> {
    ChoiceMerge.execute(context(), inputs, config)
}

#[test]
fn matches_all_independent_contract_cases() {
    let fixture: Value =
        serde_json::from_str(include_str!("../examples/nodes/choice_merge/contract.json")).unwrap();
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
fn rejects_invalid_configs_nonfinite_scores_and_invalid_packets_before_roots() {
    for config in [
        json!(null),
        json!([]),
        json!(0),
        json!(false),
        json!(""),
        json!({"sort":false}),
        json!({"options":[]}),
    ] {
        assert_eq!(
            ChoiceMerge.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            execute(&Inputs::new(), &config).unwrap_err().code,
            "invalid-config"
        );
    }
    for score in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let inputs = [(
            "a".into(),
            data(choice(vec![option(text("bad"), Some(score))])),
        )]
        .into();
        assert_eq!(
            execute(&inputs, &json!({})).unwrap_err().code,
            "invalid-packet"
        );
    }
    let inputs = [(
        "b".into(),
        data(Candidate::Text {
            text: "not-choice".into(),
            meta: Some(TextMeta {
                reading: None,
                source_span: Some([3, 1]),
            }),
        }),
    )]
    .into();
    assert_eq!(
        execute(&inputs, &json!({})).unwrap_err().code,
        "invalid-packet"
    );
}

#[test]
fn preserves_duplicates_ties_metadata_nested_candidates_and_input_immutability() {
    let duplicate = option(text("same"), Some(-2.0));
    let nested = option(
        choice(vec![option(
            Candidate::Text {
                text: "𠮷👩‍💻e\u{301}".into(),
                meta: Some(TextMeta {
                    reading: Some("よし".into()),
                    source_span: Some([2, 7]),
                }),
            },
            Some(8.0),
        )]),
        None,
    );
    let a = Arc::new(Packet::new(choice(vec![duplicate.clone(), nested.clone()])));
    let b = Arc::new(Packet::new(choice(vec![
        duplicate.clone(),
        option(Candidate::Sequence { items: vec![] }, Some(100.0)),
    ])));
    let before_a = a.to_json().unwrap();
    let before_b = b.to_json().unwrap();
    let inputs = [
        ("a".into(), OutputValue::Data(a.clone())),
        ("b".into(), OutputValue::Data(b.clone())),
    ]
    .into();
    let expected = choice(vec![
        duplicate.clone(),
        nested,
        duplicate,
        option(Candidate::Sequence { items: vec![] }, Some(100.0)),
    ]);
    for _ in 0..3 {
        let result = execute(&inputs, &json!({})).unwrap();
        assert_eq!(result["out"], data(expected.clone()));
        assert_eq!(a.to_json().unwrap(), before_a);
        assert_eq!(b.to_json().unwrap(), before_b);
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
fn copies_deep_option_values_iteratively_without_flattening() {
    let depth = 4_000;
    let mut root = text("終");
    for _ in 0..depth {
        root = Candidate::Sequence { items: vec![root] };
    }
    let input = Arc::new(Packet::new(choice(vec![option(root, Some(-0.0))])));
    let inputs = [("a".into(), OutputValue::Data(input.clone()))].into();
    let mut result = execute(&inputs, &json!({})).unwrap();
    let OutputValue::Data(output) = result.remove("out").unwrap() else {
        panic!("expected Data")
    };
    let Candidate::Choice { options } = &output.root else {
        panic!("expected Choice")
    };
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].score.unwrap().to_bits(), (-0.0f64).to_bits());
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
            vec![
                Port::packet("out")
                    .with_language("ja")
                    .with_root(RootConstraint::Choice),
            ],
        )
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([("out".into(), self.value.clone())].into())
    }
}
fn instance(id: &str, kind: &str) -> NodeInstance {
    NodeInstance {
        id: id.into(),
        kind: kind.into(),
        version: 1,
        config: None,
    }
}

#[test]
fn own_registry_engine_executes_optional_no_output_and_multiple_instances() {
    for available_a in [true, false] {
        for available_b in [true, false] {
            let a = if available_a {
                data(choice(vec![option(text("a"), Some(-1.0))]))
            } else {
                OutputValue::NoOutput
            };
            let b = if available_b {
                data(choice(vec![option(text("b"), None)]))
            } else {
                OutputValue::NoOutput
            };
            let mut registry = Registry::new();
            registry.register(ChoiceMerge).unwrap();
            registry
                .register(Source {
                    kind: "test.merge-a",
                    value: a,
                })
                .unwrap();
            registry
                .register(Source {
                    kind: "test.merge-b",
                    value: b,
                })
                .unwrap();
            let graph = Graph {
                version: 1,
                nodes: vec![
                    instance("a", "test.merge-a"),
                    instance("b", "test.merge-b"),
                    instance("merge", KIND),
                    instance("empty", KIND),
                ],
                edges: ["a", "b"]
                    .into_iter()
                    .map(|port| Edge {
                        from: Endpoint::new(port, "out"),
                        to: Endpoint::new("merge", port),
                    })
                    .collect(),
            };
            let mut expected = vec![];
            if available_a {
                expected.push(option(text("a"), Some(-1.0)));
            }
            if available_b {
                expected.push(option(text("b"), None));
            }
            let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
            for _ in 0..2 {
                let report = engine.run().unwrap();
                assert!(report.succeeded());
                for id in ["merge", "empty"] {
                    assert_eq!(report.nodes[id].status, NodeStatus::Executed);
                }
                assert_eq!(
                    report.packet("merge", "out").unwrap().root,
                    choice(expected.clone())
                );
                assert_eq!(report.packet("empty", "out").unwrap().root, choice(vec![]));
            }
        }
    }
}

#[test]
fn declares_optional_choice_ports_and_empty_config() {
    let definition = ChoiceMerge.definition();
    assert_eq!(definition.kind, KIND);
    assert_eq!(definition.version, 1);
    assert_eq!(definition.default_config, json!({}));
    assert_eq!(definition.inputs.len(), 2);
    assert_eq!(definition.outputs.len(), 1);
    for (input, id) in definition.inputs.iter().zip(["a", "b"]) {
        assert_eq!(input.port.id, id);
        assert!(!input.required);
        assert_eq!(input.port.root, RootConstraint::Choice);
        assert_eq!(input.port.language.as_deref(), Some("ja"));
    }
    assert_eq!(definition.outputs[0].id, "out");
    assert_eq!(definition.outputs[0].root, RootConstraint::Choice);
    assert_eq!(definition.outputs[0].language.as_deref(), Some("ja"));
}
