#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    ChoiceOption,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::packet_validate_constraints as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::PacketValidateConstraints;
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "test",
        input: None,
    }
}
fn build(spec: &Value) -> OutputValue {
    let root = match spec["root"].as_str().unwrap() {
        "text" => Candidate::Text {
            text: spec["text"].as_str().unwrap().into(),
            meta: None,
        },
        "sequence" => Candidate::Sequence {
            items: (0..spec["count"].as_u64().unwrap())
                .map(|_| Candidate::Text {
                    text: "x".into(),
                    meta: None,
                })
                .collect(),
        },
        "choice" => Candidate::Choice {
            options: (0..spec["count"].as_u64().unwrap())
                .map(|_| ChoiceOption {
                    value: Candidate::Text {
                        text: "x".into(),
                        meta: None,
                    },
                    score: None,
                })
                .collect(),
        },
        other => panic!("unknown root {other}"),
    };
    OutputValue::Data(Arc::new(Packet::new(root)))
}
fn run(input: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    PacketValidateConstraints.execute(context(), &[("in".into(), input)].into(), config)
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/packet_validate_constraints/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_cases_match() {
    let fixtures = fixture();
    for case in fixtures["cases"].as_array().unwrap() {
        let config = case.get("config").unwrap_or(&fixtures["defaultConfig"]);
        let input = build(&case["input"]);
        let outputs = run(input.clone(), config).unwrap();
        let (routed, other) = if case["expected"] == "valid" {
            ("valid", "invalid")
        } else {
            ("invalid", "valid")
        };
        assert_eq!(outputs[routed], input, "case={case}");
        assert_eq!(outputs[other], OutputValue::NoOutput, "case={case}");
    }
}
#[test]
fn failures_remain_distinct() {
    let config = PacketValidateConstraints.definition().default_config;
    let outputs = run(OutputValue::NoOutput, &config).unwrap();
    assert_eq!(outputs["valid"], OutputValue::NoOutput);
    assert_eq!(outputs["invalid"], OutputValue::NoOutput);
    let missing: Inputs = [].into();
    assert_eq!(
        PacketValidateConstraints
            .execute(context(), &missing, &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            PacketValidateConstraints
                .validate_config(invalid)
                .unwrap_err()
                .code,
            "invalid-config",
            "{invalid}"
        );
    }
}
#[test]
fn large_limits_do_not_wrap_on_wasm() {
    let config = json!({"maxCandidates":4294967296u64,"maxTextLength":4294967296u64,"allowedRoots":[]});
    for input in [
        json!({"root":"text","text":"a"}),
        json!({"root":"choice","count":1}),
    ] {
        let packet = build(&input);
        let outputs = run(packet.clone(), &config).unwrap();
        assert_eq!(outputs["valid"], packet);
        assert_eq!(outputs["invalid"], OutputValue::NoOutput);
    }
}
struct Source;
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new("test.source", 1, vec![], vec![Port::packet("out")])
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([("out".into(), build(&json!({"root":"text","text":"abcd"})))].into())
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
                config: Some(json!({"maxCandidates":null,"maxTextLength":3,"allowedRoots":[]})),
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
        report.packet("node", "invalid").unwrap().root,
        Candidate::Text {
            text: "abcd".into(),
            meta: None
        }
    );
}
