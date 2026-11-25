#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    ChoiceOption, TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::packet_compare as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::PacketCompare;
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
                score: Some(option["score"].as_f64().unwrap()),
            })
            .collect();
        Candidate::Choice { options }
    }
}
fn data(spec: &Value) -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(build(spec))))
}
fn run(inputs: Inputs, config: &Value) -> Result<Outputs, CoreError> {
    PacketCompare.execute(context(), &inputs, config)
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/packet_compare/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_cases_match() {
    for case in fixture()["cases"].as_array().unwrap() {
        let a = data(&case["a"]);
        let inputs: Inputs = [("a".into(), a.clone()), ("b".into(), data(&case["b"]))].into();
        let outputs = run(inputs, &case["config"]).unwrap();
        let (matched, unmatched) = if case["expected"] == "matched" {
            (a, OutputValue::NoOutput)
        } else {
            (OutputValue::NoOutput, a)
        };
        assert_eq!(outputs["matched"], matched, "case={}", case["id"]);
        assert_eq!(outputs["unmatched"], unmatched, "case={}", case["id"]);
    }
}
#[test]
fn no_output_on_either_side_clears_both_outputs() {
    let config = json!({ "mode": "exact" });
    for inputs in [
        [
            ("a".into(), OutputValue::NoOutput),
            ("b".into(), data(&json!({ "text": "x" }))),
        ]
        .into(),
        [
            ("a".into(), data(&json!({ "text": "x" }))),
            ("b".into(), OutputValue::NoOutput),
        ]
        .into(),
    ] {
        let outputs = run(inputs, &config).unwrap();
        assert_eq!(outputs["matched"], OutputValue::NoOutput);
        assert_eq!(outputs["unmatched"], OutputValue::NoOutput);
    }
}
#[test]
fn failures_remain_distinct() {
    let config = PacketCompare.definition().default_config;
    assert_eq!(config, json!({ "mode": "exact" }));
    assert_eq!(
        run(Inputs::new(), &config).unwrap_err().code,
        "missing-input"
    );
    let only_a: Inputs = [("a".into(), data(&json!({ "text": "x" })))].into();
    assert_eq!(run(only_a, &config).unwrap_err().code, "missing-input");
    let broken: Inputs = [
        (
            "a".into(),
            OutputValue::Data(Arc::new(Packet {
                version: 99,
                root: build(&json!({ "text": "x" })),
            })),
        ),
        ("b".into(), data(&json!({ "text": "x" }))),
    ]
    .into();
    assert_eq!(run(broken, &config).unwrap_err().code, "invalid-packet");
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            PacketCompare.validate_config(invalid).unwrap_err().code,
            "invalid-config",
            "{invalid}"
        );
    }
}
struct Source(OutputValue);
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new("test.source", 1, vec![], vec![Port::packet("out")])
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([("out".into(), self.0.clone())].into())
    }
}
#[test]
fn registry_and_engine_use_the_actual_node() {
    let mut registry = Registry::new();
    registry
        .register(Source(data(&json!({ "text": "same" }))))
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
                config: Some(json!({ "mode": "content" })),
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
            text: "same".into(),
            meta: None
        }
    );
}
