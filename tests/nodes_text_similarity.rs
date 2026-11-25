#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::text_similarity as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextSimilarity;
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
    TextSimilarity.execute(context(), &inputs, config)
}
fn pair(a: &str, b: &str) -> Inputs {
    [("a".into(), data(a)), ("b".into(), data(b))].into()
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_similarity/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_cases_match() {
    for case in fixture()["cases"].as_array().unwrap() {
        let outputs = run(
            pair(case["a"].as_str().unwrap(), case["b"].as_str().unwrap()),
            &json!({}),
        )
        .unwrap();
        assert_eq!(
            outputs["out"],
            data(case["expected"].as_str().unwrap()),
            "case={case}"
        );
    }
}
#[test]
fn input_limit_is_enforced_per_side() {
    let too_long = "x".repeat(4097);
    assert_eq!(
        run(pair(&too_long, "x"), &json!({})).unwrap_err().code,
        "input-limit"
    );
    assert_eq!(
        run(pair("x", &too_long), &json!({})).unwrap_err().code,
        "input-limit"
    );
    // Exactly 4096 code points is accepted.
    let exact = "x".repeat(4096);
    let outputs = run(pair(&exact, &exact), &json!({})).unwrap();
    assert_eq!(outputs["out"], data("1"));
}
#[test]
fn failures_remain_distinct() {
    let config = TextSimilarity.definition().default_config;
    assert_eq!(config, json!({}));
    assert_eq!(
        run(Inputs::new(), &config).unwrap_err().code,
        "missing-input"
    );
    let no_output: Inputs = [("a".into(), OutputValue::NoOutput), ("b".into(), data("x"))].into();
    let outputs = run(no_output, &config).unwrap();
    assert_eq!(outputs["out"], OutputValue::NoOutput);
    let wrong_root: Inputs = [
        (
            "a".into(),
            OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items: vec![] }))),
        ),
        ("b".into(), data("x")),
    ]
    .into();
    assert_eq!(run(wrong_root, &config).unwrap_err().code, "invalid-root");
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            TextSimilarity.validate_config(invalid).unwrap_err().code,
            "invalid-config",
            "{invalid}"
        );
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
fn registry_and_engine_use_the_actual_node() {
    let mut registry = Registry::new();
    registry.register(Source("abc".into())).unwrap();
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
                config: Some(json!({})),
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
        Candidate::Text {
            text: "1".into(),
            meta: None
        }
    );
}
