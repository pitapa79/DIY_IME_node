#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::flow_input_select as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::FlowInputSelect;
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "test",
        input: None,
    }
}
fn data(text: &str) -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
        text: text.into(),
        meta: None,
    })))
}
fn run(inputs: Inputs, config: &Value) -> Result<Outputs, CoreError> {
    FlowInputSelect.execute(context(), &inputs, config)
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/flow_input_select/contract.json"
    ))
    .unwrap()
}
fn side(value: &Value) -> Option<OutputValue> {
    if value.is_null() {
        None
    } else {
        Some(data(value.as_str().unwrap()))
    }
}
#[test]
fn fixture_cases_match() {
    for case in fixture()["cases"].as_array().unwrap() {
        let mut pairs: Vec<(String, OutputValue)> = Vec::new();
        if let Some(a) = side(&case["a"]) {
            pairs.push(("a".into(), a));
        }
        if let Some(b) = side(&case["b"]) {
            pairs.push(("b".into(), b));
        }
        let inputs: Inputs = pairs.into_iter().collect();
        let outputs = run(inputs, &case["config"]).unwrap();
        let expected = side(&case["expected"]).unwrap_or(OutputValue::NoOutput);
        assert_eq!(outputs["out"], expected, "case={}", case["id"]);
    }
}
#[test]
fn only_the_selected_side_is_validated() {
    let broken = OutputValue::Data(Arc::new(Packet {
        version: 99,
        root: Candidate::Text {
            text: "x".into(),
            meta: None,
        },
    }));
    // a-first: a is selected and broken -> invalid-packet, b never reached.
    let inputs: Inputs = [("a".into(), broken.clone()), ("b".into(), data("B"))].into();
    assert_eq!(
        run(inputs, &json!({ "priority": "a-first" }))
            .unwrap_err()
            .code,
        "invalid-packet"
    );
    // b-first: b is selected; the broken a is not validated at all.
    let inputs: Inputs = [("a".into(), broken), ("b".into(), data("B"))].into();
    let outputs = run(inputs, &json!({ "priority": "b-first" })).unwrap();
    assert_eq!(outputs["out"], data("B"));
}
#[test]
fn any_root_can_be_selected_unchanged() {
    let sequence = OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items: vec![] })));
    let inputs: Inputs = [("a".into(), sequence.clone())].into();
    let outputs = run(inputs, &json!({ "priority": "a-first" })).unwrap();
    assert_eq!(outputs["out"], sequence);
}
#[test]
fn failures_remain_distinct() {
    let config = FlowInputSelect.definition().default_config;
    assert_eq!(config, json!({ "priority": "a-first" }));
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            FlowInputSelect.validate_config(invalid).unwrap_err().code,
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
    registry.register(Source(data("A"))).unwrap();
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
                config: Some(json!({ "priority": "a-first" })),
            },
        ],
        edges: vec![Edge {
            from: Endpoint::new("source", "out"),
            to: Endpoint::new("node", "a"),
        }],
    };
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    assert!(report.succeeded());
    assert_eq!(
        report.packet("node", "out").unwrap().root,
        Candidate::Text {
            text: "A".into(),
            meta: None
        }
    );
}
