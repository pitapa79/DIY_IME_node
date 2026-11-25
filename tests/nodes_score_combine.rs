#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    ChoiceOption, TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::score_combine as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::ScoreCombine;
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "test",
        input: None,
    }
}
fn build(spec: &Value) -> Candidate {
    if let Some(text) = spec.get("text") {
        let meta = spec.get("reading").map(|reading| TextMeta {
            reading: Some(reading.as_str().unwrap().into()),
            source_span: None,
        });
        Candidate::Text {
            text: text.as_str().unwrap().into(),
            meta,
        }
    } else if let Some(items) = spec.get("sequence") {
        Candidate::Sequence {
            items: items.as_array().unwrap().iter().map(build).collect(),
        }
    } else {
        let options = spec["choice"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| ChoiceOption {
                value: build(&option["value"]),
                score: option["score"].as_f64(),
            })
            .collect();
        Candidate::Choice { options }
    }
}
fn data(spec: &Value) -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(build(spec))))
}
fn run(inputs: Inputs, config: &Value) -> Result<Outputs, CoreError> {
    ScoreCombine.execute(context(), &inputs, config)
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/score_combine/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_cases_match() {
    for case in fixture()["cases"].as_array().unwrap() {
        let inputs: Inputs = [
            ("a".into(), data(&case["a"])),
            ("b".into(), data(&case["b"])),
        ]
        .into();
        let result = run(inputs, &case["config"]);
        if let Some(error) = case.get("error") {
            assert_eq!(
                result.unwrap_err().code,
                error.as_str().unwrap(),
                "case={}",
                case["id"]
            );
            continue;
        }
        let outputs = result.unwrap();
        assert_eq!(
            outputs["out"],
            data(&case["expected"]),
            "case={}",
            case["id"]
        );
    }
}
#[test]
fn no_output_on_either_side_clears_the_output() {
    let config = json!({ "weightA": 1.0, "weightB": 1.0, "missing": "zero" });
    for inputs in [
        [
            ("a".into(), OutputValue::NoOutput),
            ("b".into(), data(&json!({ "choice": [] }))),
        ]
        .into(),
        [
            ("a".into(), data(&json!({ "choice": [] }))),
            ("b".into(), OutputValue::NoOutput),
        ]
        .into(),
    ] {
        let outputs = run(inputs, &config).unwrap();
        assert_eq!(outputs["out"], OutputValue::NoOutput);
    }
}
#[test]
fn failures_remain_distinct() {
    let config = ScoreCombine.definition().default_config;
    assert_eq!(
        config,
        json!({ "weightA": 1.0, "weightB": 1.0, "missing": "zero" })
    );
    assert_eq!(
        run(Inputs::new(), &config).unwrap_err().code,
        "missing-input"
    );
    let wrong_root: Inputs = [
        ("a".into(), data(&json!({ "text": "x" }))),
        ("b".into(), data(&json!({ "choice": [] }))),
    ]
    .into();
    assert_eq!(run(wrong_root, &config).unwrap_err().code, "invalid-root");
    let broken: Inputs = [
        (
            "a".into(),
            OutputValue::Data(Arc::new(Packet {
                version: 99,
                root: build(&json!({ "choice": [] })),
            })),
        ),
        ("b".into(), data(&json!({ "choice": [] }))),
    ]
    .into();
    assert_eq!(run(broken, &config).unwrap_err().code, "invalid-packet");
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            ScoreCombine.validate_config(invalid).unwrap_err().code,
            "invalid-config",
            "{invalid}"
        );
    }
}
struct Source(OutputValue);
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.source",
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
        Ok([("out".into(), self.0.clone())].into())
    }
}
#[test]
fn registry_and_engine_use_the_actual_node() {
    let mut registry = Registry::new();
    registry
        .register(Source(data(
            &json!({ "choice": [{ "value": { "text": "X" }, "score": 1.5 }] }),
        )))
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
                config: Some(json!({ "weightA": 1.0, "weightB": 1.0, "missing": "zero" })),
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
        report.packet("node", "out").unwrap().root,
        Candidate::Choice {
            options: vec![ChoiceOption {
                value: Candidate::Text {
                    text: "X".into(),
                    meta: None,
                },
                score: Some(3.0),
            }],
        }
    );
}
