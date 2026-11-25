#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::text_quantity_format as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextQuantityFormat;
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
fn run(input: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    TextQuantityFormat.execute(context(), &[("in".into(), input)].into(), config)
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_quantity_format/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_cases_match() {
    for case in fixture()["cases"].as_array().unwrap() {
        let result = run(data(case["input"].as_str().unwrap()), &json!({}));
        if let Some(error) = case.get("error") {
            assert_eq!(
                result.unwrap_err().code,
                error.as_str().unwrap(),
                "case={case}"
            );
        } else {
            assert_eq!(
                result.unwrap()["out"],
                data(case["expected"].as_str().unwrap()),
                "case={case}"
            );
        }
    }
}
#[test]
fn six_books_does_not_geminate() {
    assert_eq!(
        run(data("6冊"), &json!({})).unwrap()["out"],
        data("ろくさつ")
    );
    assert_eq!(
        run(data("16冊"), &json!({})).unwrap()["out"],
        data("じゅうろくさつ")
    );
    assert_eq!(run(data("6個"), &json!({})).unwrap()["out"], data("ろっこ"));
}
#[test]
fn failures_remain_distinct() {
    let config = TextQuantityFormat.definition().default_config;
    assert_eq!(config, json!({}));
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap()["out"],
        OutputValue::NoOutput
    );
    let missing: Inputs = [].into();
    assert_eq!(
        TextQuantityFormat
            .execute(context(), &missing, &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    let wrong_root =
        OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items: vec![] })));
    assert_eq!(run(wrong_root, &config).unwrap_err().code, "invalid-root");
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            TextQuantityFormat
                .validate_config(invalid)
                .unwrap_err()
                .code,
            "invalid-config",
            "{invalid}"
        );
    }
}
struct Source;
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
        Ok([("out".into(), data("3本"))].into())
    }
}
#[test]
fn registry_and_engine_use_the_actual_node() {
    let mut registry = Registry::new();
    registry.register(Source).unwrap();
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
        edges: vec![Edge {
            from: Endpoint::new("source", "out"),
            to: Endpoint::new("node", "in"),
        }],
    };
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    assert!(report.succeeded());
    assert_eq!(
        report.packet("node", "out").unwrap().root,
        Candidate::Text {
            text: "さんぼん".into(),
            meta: None
        }
    );
}
