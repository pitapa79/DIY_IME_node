#[path = "support/mod.rs"]
mod test_host;
use std::sync::Arc;

use crate::test_host::{
    Candidate, ChoiceOption, Packet, TextMeta,
    builtins::register_builtin_nodes,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext,
    },
    nodes::stringify::{KIND, Stringify, stringify},
    runtime::{Engine, NodeStatus, ResolvedValue},
};
use serde_json::{Value, json};

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
        node_id: "stringify",
        input: None,
    }
}
fn run(value: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    Stringify.execute(context(), &[("in".into(), value)].into(), config)
}
fn data(packet: Packet) -> OutputValue {
    OutputValue::Data(Arc::new(packet))
}
fn assert_text(value: &OutputValue, expected: &str) {
    let OutputValue::Data(packet) = value else {
        panic!("expected Data, got {value:?}")
    };
    assert_eq!(packet.as_ref(), &Packet::new(text(expected)));
}

#[test]
fn all_contract_cases_match_the_node_adapter() {
    let fixture: Value = serde_json::from_str(include_str!("contracts/stringify.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 12);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let input = if case["inputStatus"] == "no-output" {
            OutputValue::NoOutput
        } else {
            data(serde_json::from_value(case["packet"].clone()).unwrap())
        };
        let result = run(
            input,
            case.get("config").unwrap_or(&fixture["defaultConfig"]),
        );
        match case["expected"]["status"].as_str().unwrap() {
            "data" => {
                let output = result.unwrap_or_else(|error| panic!("{id}: {error}"));
                let OutputValue::Data(packet) = &output["out"] else {
                    panic!("{id}: expected Data")
                };
                assert_eq!(
                    serde_json::to_value(packet.as_ref()).unwrap(),
                    case["expected"]["packet"],
                    "{id}"
                );
            }
            "no-output" => assert_eq!(result.unwrap()["out"], OutputValue::NoOutput, "{id}"),
            "error" => assert_eq!(result.unwrap_err().code, case["expected"]["code"], "{id}"),
            other => panic!("unsupported fixture status {other}"),
        }
    }
}

#[test]
fn config_validation_rejects_non_objects_and_all_unknown_keys() {
    Stringify.validate_config(&json!({})).unwrap();
    for config in [
        json!(null),
        json!([]),
        json!(false),
        json!(0),
        json!(""),
        json!({"separator":""}),
        json!({"unknown":null}),
    ] {
        assert_eq!(
            Stringify.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            run(OutputValue::NoOutput, &config).unwrap_err().code,
            "invalid-config"
        );
    }
    assert_eq!(
        Stringify
            .execute(context(), &Inputs::new(), &json!({}))
            .unwrap_err()
            .code,
        "missing-input"
    );
}

#[test]
fn input_is_unchanged_and_output_is_reproducible_without_meta() {
    let packet = Arc::new(Packet::new(Candidate::Sequence {
        items: vec![
            Candidate::Text {
                text: "𠮷e\u{301}".into(),
                meta: Some(TextMeta {
                    reading: Some("よし".into()),
                    source_span: Some([0, 3]),
                }),
            },
            Candidate::Sequence {
                items: vec![text("👩‍💻\n"), text("30%\tです")],
            },
        ],
    }));
    let before = packet.to_json().unwrap();
    for _ in 0..3 {
        assert_text(
            &run(OutputValue::Data(packet.clone()), &json!({})).unwrap()["out"],
            "𠮷e\u{301}👩‍💻\n30%\tです",
        );
        assert_eq!(packet.to_json().unwrap(), before);
    }
}

#[test]
fn validates_the_entire_packet_before_reporting_an_unresolved_choice() {
    let invalid = Packet::new(Candidate::Choice {
        options: vec![ChoiceOption {
            value: text("a"),
            score: Some(f64::NAN),
        }],
    });
    assert_eq!(stringify(&invalid).unwrap_err().code, "invalid-packet");
    let invalid = Packet::new(Candidate::Sequence {
        items: vec![
            Candidate::Choice { options: vec![] },
            Candidate::Text {
                text: "bad-span".into(),
                meta: Some(TextMeta {
                    reading: None,
                    source_span: Some([3, 1]),
                }),
            },
        ],
    });
    assert_eq!(stringify(&invalid).unwrap_err().code, "invalid-packet");
}

#[test]
fn walks_deep_sequences_without_recursive_calls() {
    let mut root = text("終");
    for _ in 0..10_000 {
        root = Candidate::Sequence {
            items: vec![text("a"), root],
        };
    }
    let packet = Packet::new(root);
    assert_eq!(stringify(&packet).unwrap(), "a".repeat(10_000) + "終");
    // Rust標準の派生Dropによる再帰も避け、テストの所有データを反復的に解放する。
    let mut pending = vec![packet.root];
    while let Some(root) = pending.pop() {
        if let Candidate::Sequence { items } = root {
            pending.extend(items);
        }
    }
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
fn node(id: &str, kind: &str) -> NodeInstance {
    NodeInstance {
        id: id.into(),
        kind: kind.into(),
        version: 1,
        config: None,
    }
}
fn edge(source: &str, target: &str) -> Edge {
    Edge {
        from: Endpoint::new(source, "out"),
        to: Endpoint::new(target, "in"),
    }
}

#[test]
fn individual_registry_executes_multiple_instances_and_contains_failures_and_no_output() {
    let mut registry = Registry::new();
    registry.register(Stringify).unwrap();
    for source in [
        Source {
            kind: "test.text",
            value: data(Packet::new(Candidate::Sequence {
                items: vec![text("本日"), text("です")],
            })),
        },
        Source {
            kind: "test.choice",
            value: data(Packet::new(Candidate::Sequence {
                items: vec![text("prefix"), Candidate::Choice { options: vec![] }],
            })),
        },
        Source {
            kind: "test.empty",
            value: OutputValue::NoOutput,
        },
    ] {
        registry.register(source).unwrap();
    }
    let graph = Graph {
        version: 1,
        nodes: vec![
            node("source", "test.text"),
            node("choice", "test.choice"),
            node("empty", "test.empty"),
            node("a", KIND),
            node("b", KIND),
            node("failed", KIND),
            node("blocked", KIND),
            node("skipped", KIND),
        ],
        edges: vec![
            edge("source", "a"),
            edge("source", "b"),
            edge("choice", "failed"),
            edge("failed", "blocked"),
            edge("empty", "skipped"),
        ],
    };
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    for _ in 0..3 {
        let report = engine.run().unwrap();
        for id in ["a", "b"] {
            assert_eq!(report.packet(id, "out").unwrap().root, text("本日です"));
        }
        assert_eq!(report.nodes["failed"].status, NodeStatus::Failed);
        let ResolvedValue::Error(error) = &report.nodes["failed"].outputs["out"] else {
            panic!("failure must not return partial text")
        };
        assert_eq!(error.error.code, "unresolved-choice");
        assert_eq!(report.nodes["blocked"].status, NodeStatus::Blocked);
        assert_eq!(report.nodes["skipped"].status, NodeStatus::Skipped);
        assert!(matches!(
            report.nodes["skipped"].outputs["out"],
            ResolvedValue::NoOutput
        ));
        assert_eq!(report.plan_version, 1);
    }
}

#[test]
fn definition_and_standard_registry_match_the_contract() {
    let definition = Stringify.definition();
    assert_eq!((definition.kind.as_str(), definition.version), (KIND, 1));
    assert_eq!(definition.default_config, json!({}));
    assert_eq!(definition.inputs.len(), 1);
    assert!(definition.inputs[0].required);
    assert_eq!(definition.inputs[0].port.id, "in");
    assert_eq!(definition.inputs[0].port.root, RootConstraint::Any);
    assert_eq!(definition.inputs[0].port.language.as_deref(), Some("ja"));
    assert_eq!(definition.outputs.len(), 1);
    assert_eq!(definition.outputs[0].id, "out");
    assert_eq!(definition.outputs[0].root, RootConstraint::Text);
    assert_eq!(definition.outputs[0].language.as_deref(), Some("ja"));
    let mut registry = Registry::new();
    register_builtin_nodes(&mut registry).unwrap();
    assert!(registry.definitions().any(|entry| entry.kind == KIND));
    let mut graph = Graph::from_json(include_str!("../examples/graphs/stringify.json")).unwrap();
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    assert!(report.succeeded());
    assert_eq!(
        report.packet("result", "out").unwrap().root,
        text("本日の降水確率は30%です")
    );
    graph
        .nodes
        .iter_mut()
        .find(|entry| entry.id == "result")
        .unwrap()
        .config = Some(json!({"separator":" "}));
    assert_eq!(
        ExecutionPlan::compile(&graph, &registry)
            .err()
            .unwrap()
            .code,
        "invalid-config"
    );
}
