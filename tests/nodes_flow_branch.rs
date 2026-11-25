#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::flow_branch as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::FlowBranch;
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
fn run(data_in: OutputValue, selector_in: OutputValue) -> Result<Outputs, CoreError> {
    let inputs: Inputs = [("data".into(), data_in), ("selector".into(), selector_in)].into();
    FlowBranch.execute(context(), &inputs, &json!({}))
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../examples/nodes/flow_branch/contract.json")).unwrap()
}
#[test]
fn fixture_cases_match() {
    for case in fixture()["cases"].as_array().unwrap() {
        let data_in = case["data"]
            .as_str()
            .map(data)
            .unwrap_or(OutputValue::NoOutput);
        let selector_in = case["selector"]
            .as_str()
            .map(data)
            .unwrap_or(OutputValue::NoOutput);
        let result = run(data_in.clone(), selector_in);
        if let Some(error) = case.get("error") {
            assert_eq!(
                result.unwrap_err().code,
                error.as_str().unwrap(),
                "case={case}"
            );
        } else if case.get("noOutput").and_then(Value::as_bool) == Some(true) {
            let outputs = result.unwrap();
            assert_eq!(outputs["a"], OutputValue::NoOutput, "case={case}");
            assert_eq!(outputs["b"], OutputValue::NoOutput, "case={case}");
        } else {
            let outputs = result.unwrap();
            let routed = case["routed"].as_str().unwrap();
            let other = if routed == "a" { "b" } else { "a" };
            assert_eq!(outputs[routed], data_in, "case={case}");
            assert_eq!(outputs[other], OutputValue::NoOutput, "case={case}");
        }
    }
}
#[test]
fn failures_remain_distinct() {
    let missing: Inputs = [("data".into(), data("x"))].into();
    assert_eq!(
        FlowBranch
            .execute(context(), &missing, &json!({}))
            .unwrap_err()
            .code,
        "missing-input"
    );
    let wrong_root =
        OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items: vec![] })));
    assert_eq!(run(data("x"), wrong_root).unwrap_err().code, "invalid-root");
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            FlowBranch.validate_config(invalid).unwrap_err().code,
            "invalid-config",
            "{invalid}"
        );
    }
}
struct DataSource;
impl Node for DataSource {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new("test.data", 1, vec![], vec![Port::packet("out")])
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([("out".into(), data("payload"))].into())
    }
}
struct SelectorSource;
impl Node for SelectorSource {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.selector",
            1,
            vec![],
            vec![Port::packet("out").with_root(RootConstraint::Text)],
        )
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([("out".into(), data("b"))].into())
    }
}
#[test]
fn registry_and_engine_use_the_actual_node() {
    let mut registry = Registry::new();
    registry.register(DataSource).unwrap();
    registry.register(SelectorSource).unwrap();
    diy_ime2_nodes::opt_in::register(&mut registry).unwrap();
    let graph = Graph {
        version: 1,
        nodes: vec![
            NodeInstance {
                id: "data".into(),
                kind: "test.data".into(),
                version: 1,
                config: None,
            },
            NodeInstance {
                id: "selector".into(),
                kind: "test.selector".into(),
                version: 1,
                config: None,
            },
            NodeInstance {
                id: "node".into(),
                kind: subject::KIND.into(),
                version: 1,
                config: Some(json!({})),
            },
        ],
        edges: vec![
            Edge {
                from: Endpoint::new("data", "out"),
                to: Endpoint::new("node", "data"),
            },
            Edge {
                from: Endpoint::new("selector", "out"),
                to: Endpoint::new("node", "selector"),
            },
        ],
    };
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    assert!(report.succeeded());
    assert_eq!(
        report.packet("node", "b").unwrap().root,
        Candidate::Text {
            text: "payload".into(),
            meta: None
        }
    );
}
