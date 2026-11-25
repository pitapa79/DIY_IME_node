#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::text_snippet_expand as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextSnippetExpand;
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
    TextSnippetExpand.execute(context(), &inputs, config)
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_snippet_expand/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_cases_match() {
    for case in fixture()["cases"].as_array().unwrap() {
        let inputs: Inputs = case["inputs"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, text)| (id.clone(), data(text.as_str().unwrap())))
            .collect();
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
            data(case["expected"].as_str().unwrap()),
            "case={}",
            case["id"]
        );
    }
}
#[test]
fn no_output_propagates_only_through_used_ports() {
    let config = json!({ "template": "{a}" });
    // A used port that is NoOutput clears the output.
    let inputs: Inputs = [("a".into(), OutputValue::NoOutput)].into();
    let outputs = run(inputs, &config).unwrap();
    assert_eq!(outputs["out"], OutputValue::NoOutput);
    // An unused port that is NoOutput (which is also how the runtime
    // delivers unconnected optional ports) changes nothing.
    let inputs: Inputs = [("a".into(), data("A")), ("d".into(), OutputValue::NoOutput)].into();
    let outputs = run(inputs, &config).unwrap();
    assert_eq!(outputs["out"], data("A"));
}
#[test]
fn size_limits_are_enforced() {
    // Template over 4096 bytes is a config error.
    let long_template = json!({ "template": "x".repeat(4097) });
    assert_eq!(
        TextSnippetExpand
            .validate_config(&long_template)
            .unwrap_err()
            .code,
        "invalid-config"
    );
    // Expansion over 1 MiB is an output error.
    let big = "x".repeat(1024 * 1024 + 1);
    let inputs: Inputs = [("a".into(), data(&big))].into();
    assert_eq!(
        run(inputs, &json!({ "template": "{a}" })).unwrap_err().code,
        "output-limit"
    );
}
#[test]
fn failures_remain_distinct() {
    let config = TextSnippetExpand.definition().default_config;
    assert_eq!(config, json!({ "template": "{a}" }));
    let wrong_root: Inputs = [(
        "a".into(),
        OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items: vec![] }))),
    )]
    .into();
    assert_eq!(run(wrong_root, &config).unwrap_err().code, "invalid-root");
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            TextSnippetExpand.validate_config(invalid).unwrap_err().code,
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
    registry.register(Source("世界".into())).unwrap();
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
                config: Some(json!({ "template": "こんにちは{a}" })),
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
            text: "こんにちは世界".into(),
            meta: None
        }
    );
}
