#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node, tree};

#[path = "../src/nodes/sequence_build/mod.rs"]
mod subject;

use std::sync::Arc;

use crate::test_host::{
    ChoiceOption, TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext,
    },
    runtime::{Engine, NodeStatus},
};
use serde_json::{Value, json};
use subject::{KIND, SequenceBuild};

fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "build",
        input: None,
    }
}
fn text(value: &str) -> Candidate {
    Candidate::Text {
        text: value.into(),
        meta: None,
    }
}
fn sequence(items: Vec<Candidate>) -> Packet {
    Packet::new(Candidate::Sequence { items })
}
fn data(root: Candidate) -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(root)))
}
fn run(inputs: &Inputs, config: &Value) -> Result<Outputs, CoreError> {
    SequenceBuild.execute(context(), inputs, config)
}
fn assert_packet(value: &OutputValue, expected: &Packet) {
    let OutputValue::Data(actual) = value else {
        panic!("expected Data")
    };
    assert_eq!(actual.as_ref(), expected);
}
fn fixture_inputs(case: &Value) -> Inputs {
    let mut inputs = Inputs::new();
    for id in ["a", "b"] {
        if case
            .get("noOutput")
            .and_then(Value::as_array)
            .is_some_and(|ids| ids.iter().any(|value| value == id))
        {
            inputs.insert(id.into(), OutputValue::NoOutput);
        } else if let Some(value) = case.get(id) {
            inputs.insert(
                id.into(),
                data(serde_json::from_value(value.clone()).unwrap()),
            );
        }
    }
    inputs
}

#[test]
fn all_contract_examples_match_the_node() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../examples/nodes/sequence_build/contract.json"
    ))
    .unwrap();
    assert_eq!(fixture["kind"], KIND);
    for case in fixture["cases"].as_array().unwrap() {
        let output = run(
            &fixture_inputs(case),
            case.get("config").unwrap_or(&fixture["defaultConfig"]),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
        let expected = Packet::new(serde_json::from_value(case["expected"].clone()).unwrap());
        assert_packet(&output["out"], &expected);
    }
}

#[test]
fn declaration_uses_optional_any_inputs_and_sequence_output() {
    let definition = SequenceBuild.definition();
    assert_eq!(definition.kind, KIND);
    assert_eq!(definition.version, 1);
    assert_eq!(definition.default_config, json!({"items":[]}));
    assert_eq!(definition.inputs.len(), 2);
    for (input, id) in definition.inputs.iter().zip(["a", "b"]) {
        assert_eq!(input.port.id, id);
        assert!(!input.required);
        assert_eq!(input.port.root, RootConstraint::Any);
        assert_eq!(input.port.language.as_deref(), Some("ja"));
    }
    assert_eq!(definition.outputs.len(), 1);
    assert_eq!(definition.outputs[0].id, "out");
    assert_eq!(definition.outputs[0].root, RootConstraint::Sequence);
}

#[test]
fn missing_and_no_output_inputs_are_omitted_but_empty_data_is_kept() {
    let config = json!({"items":[]});
    for inputs in [
        Inputs::new(),
        [("a".into(), OutputValue::NoOutput)].into(),
        [("b".into(), OutputValue::NoOutput)].into(),
        [
            ("a".into(), OutputValue::NoOutput),
            ("b".into(), OutputValue::NoOutput),
        ]
        .into(),
    ] {
        assert_packet(&run(&inputs, &config).unwrap()["out"], &sequence(vec![]));
    }
    let inputs = [
        ("a".into(), data(Candidate::Sequence { items: vec![] })),
        ("b".into(), data(Candidate::Choice { options: vec![] })),
    ]
    .into();
    assert_packet(
        &run(&inputs, &config).unwrap()["out"],
        &sequence(vec![
            Candidate::Sequence { items: vec![] },
            Candidate::Choice { options: vec![] },
        ]),
    );
}

#[test]
fn metadata_scores_inputs_and_config_are_unchanged() {
    let original = Arc::new(Packet::new(Candidate::Choice {
        options: vec![
            ChoiceOption {
                value: Candidate::Text {
                    text: "猫".into(),
                    meta: Some(TextMeta {
                        reading: Some("ねこ".into()),
                        source_span: Some([4, 8]),
                    }),
                },
                score: Some(-3.5),
            },
            ChoiceOption {
                value: text("犬"),
                score: None,
            },
        ],
    }));
    let before = original.as_ref().clone();
    let config =
        json!({"items":[{"type":"text","text":"先","meta":{"reading":"さき","sourceSpan":[0,2]}}]});
    let before_config = config.clone();
    let inputs = [
        ("a".into(), OutputValue::Data(Arc::clone(&original))),
        ("b".into(), data(text("後"))),
    ]
    .into();
    let output = run(&inputs, &config).unwrap();
    let first: Candidate = serde_json::from_value(config["items"][0].clone()).unwrap();
    assert_packet(
        &output["out"],
        &sequence(vec![first, before.clone().root, text("後")]),
    );
    assert_eq!(original.as_ref(), &before);
    assert_eq!(config, before_config);
}

#[test]
fn strict_configuration_rejects_missing_unknown_null_and_wrong_types() {
    for config in [
        Value::Null,
        json!([]),
        json!({}),
        json!({"items":null}),
        json!({"items":""}),
        json!({"items":{}}),
        json!({"items":false}),
        json!({"items":[],"extra":1}),
        json!({"other":[]}),
    ] {
        assert_eq!(
            SequenceBuild.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            run(&Inputs::new(), &config).unwrap_err().code,
            "invalid-config"
        );
    }
}

#[test]
fn configured_candidates_reject_invalid_structure_score_and_metadata() {
    for candidate in [
        Value::Null,
        json!({"type":"text","text":"x","extra":1}),
        json!({"type":"text"}),
        json!({"type":"text","text":"x","meta":null}),
        json!({"type":"text","text":"x","meta":{"sourceSpan":[2,1]}}),
        json!({"type":"choice","options":[{"value":{"type":"text","text":"x"},"score":null}]}),
        json!({"type":"choice","options":[{"value":{"type":"text","text":"x"},"score":"NaN"}]}),
        json!({"type":"sequence","items":[{"type":"unknown"}]}),
    ] {
        let config = json!({"items":[candidate]});
        assert_eq!(
            SequenceBuild.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            run(&Inputs::new(), &config).unwrap_err().code,
            "invalid-config"
        );
    }
}

#[test]
fn either_input_packet_is_validated_before_copying() {
    for id in ["a", "b"] {
        let mut invalid_version = Packet::new(text("x"));
        invalid_version.version = 999;
        let invalid_meta = Packet::new(Candidate::Text {
            text: "x".into(),
            meta: Some(TextMeta {
                reading: None,
                source_span: Some([8, 2]),
            }),
        });
        for invalid in [invalid_version, invalid_meta] {
            let mut inputs: Inputs =
                [("a".into(), data(text("a"))), ("b".into(), data(text("b")))].into();
            inputs.insert(id.into(), OutputValue::Data(Arc::new(invalid)));
            assert_eq!(
                run(&inputs, &json!({"items":[]})).unwrap_err().code,
                "invalid-packet"
            );
        }
        for score in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut inputs = Inputs::new();
            inputs.insert(
                id.into(),
                data(Candidate::Choice {
                    options: vec![ChoiceOption {
                        value: text("bad"),
                        score: Some(score),
                    }],
                }),
            );
            assert_eq!(
                run(&inputs, &json!({"items":[]})).unwrap_err().code,
                "invalid-packet"
            );
        }
    }
}

// テストの後始末も反復にし、既存Candidateの再帰的Dropを深いデータへ使わない。
fn dispose(root: Candidate) {
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        match value {
            Candidate::Text { .. } => {}
            Candidate::Sequence { items } => pending.extend(items),
            Candidate::Choice { options } => pending.extend(options.into_iter().map(|o| o.value)),
        }
    }
}
fn nested_depth(root: &Candidate) -> usize {
    let mut value = root;
    let mut depth = 0;
    loop {
        match value {
            Candidate::Sequence { items } => {
                assert_eq!(items.len(), 1);
                value = &items[0];
                depth += 1;
            }
            Candidate::Text { text, meta } => {
                assert_eq!(text, "深🙂");
                assert_eq!(meta.as_ref().unwrap().reading.as_deref(), Some("ふか"));
                return depth;
            }
            Candidate::Choice { .. } => panic!("unexpected Choice"),
        }
    }
}

#[test]
fn deeply_nested_input_is_copied_iteratively_without_flattening() {
    let depth = 4000;
    let mut root = Candidate::Text {
        text: "深🙂".into(),
        meta: Some(TextMeta {
            reading: Some("ふか".into()),
            source_span: None,
        }),
    };
    for _ in 0..depth {
        root = Candidate::Sequence { items: vec![root] };
    }
    let original = Arc::new(Packet::new(root));
    let inputs = [("a".into(), OutputValue::Data(Arc::clone(&original)))].into();
    let mut output = run(&inputs, &json!({"items":[]})).unwrap();
    let OutputValue::Data(result) = output.remove("out").unwrap() else {
        panic!("expected Data")
    };
    assert_eq!(nested_depth(&original.root), depth);
    assert_eq!(nested_depth(&result.root), depth + 1);
    drop(inputs);
    dispose(Arc::try_unwrap(original).ok().unwrap().root);
    dispose(Arc::try_unwrap(result).ok().unwrap().root);
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
fn registry(a: OutputValue, b: OutputValue) -> Registry {
    let mut registry = Registry::default();
    registry
        .register(Source {
            kind: "test.sequence-a",
            value: a,
        })
        .unwrap();
    registry
        .register(Source {
            kind: "test.sequence-b",
            value: b,
        })
        .unwrap();
    registry.register(SequenceBuild).unwrap();
    registry
}
fn connected_graph() -> Graph {
    let nodes = vec![
        instance("a", "test.sequence-a", None),
        instance("b", "test.sequence-b", None),
        instance("plain", KIND, None),
        instance(
            "configured",
            KIND,
            Some(json!({"items":[{"type":"text","text":"設定"}]})),
        ),
    ];
    let mut edges = vec![];
    for target in ["plain", "configured"] {
        for input in ["a", "b"] {
            edges.push(Edge {
                from: Endpoint::new(input, "out"),
                to: Endpoint::new(target, input),
            });
        }
    }
    Graph {
        version: 1,
        nodes,
        edges,
    }
}

#[test]
fn engine_connects_two_inputs_and_keeps_instance_configuration_separate() {
    let a = Candidate::Sequence {
        items: vec![text("a")],
    };
    let b = Candidate::Choice {
        options: vec![ChoiceOption {
            value: text("b"),
            score: Some(2.0),
        }],
    };
    let registry = registry(data(a.clone()), data(b.clone()));
    let mut engine = Engine::new(ExecutionPlan::compile(&connected_graph(), &registry).unwrap());
    for _ in 0..2 {
        let report = engine.run().unwrap();
        assert!(report.succeeded());
        assert_eq!(
            report.packet("plain", "out").unwrap().as_ref(),
            &sequence(vec![a.clone(), b.clone()])
        );
        assert_eq!(
            report.packet("configured", "out").unwrap().as_ref(),
            &sequence(vec![text("設定"), a.clone(), b.clone()])
        );
    }
}

#[test]
fn engine_executes_without_optional_connections_and_uses_empty_default() {
    let mut registry = Registry::default();
    registry.register(SequenceBuild).unwrap();
    let graph = Graph {
        version: 1,
        nodes: vec![instance("build", KIND, None)],
        edges: vec![],
    };
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    assert_eq!(report.nodes["build"].status, NodeStatus::Executed);
    assert_eq!(
        report.packet("build", "out").unwrap().as_ref(),
        &sequence(vec![])
    );
    let invalid = Graph {
        version: 1,
        nodes: vec![instance("build", KIND, Some(json!({})))],
        edges: vec![],
    };
    let error = ExecutionPlan::compile(&invalid, &registry).err().unwrap();
    assert_eq!(error.code, "invalid-config");
}

#[test]
fn engine_omits_optional_no_output_without_skipping_sequence_build() {
    for (a, b, expected) in [
        (OutputValue::NoOutput, data(text("b")), vec![text("b")]),
        (data(text("a")), OutputValue::NoOutput, vec![text("a")]),
        (OutputValue::NoOutput, OutputValue::NoOutput, vec![]),
    ] {
        let registry = registry(a, b);
        let mut engine =
            Engine::new(ExecutionPlan::compile(&connected_graph(), &registry).unwrap());
        let report = engine.run().unwrap();
        assert_eq!(report.nodes["plain"].status, NodeStatus::Executed);
        assert_eq!(
            report.packet("plain", "out").unwrap().as_ref(),
            &sequence(expected)
        );
    }
}
