#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    ChoiceOption,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::packet_test_data as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::PacketTestData;
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "test",
        input: None,
    }
}
fn trigger() -> OutputValue {
    OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
        text: "go".into(),
        meta: None,
    })))
}
fn expected(case: &Value) -> OutputValue {
    let spec = &case["expected"];
    let root = if let Some(text) = spec.get("text") {
        Candidate::Text {
            text: text.as_str().unwrap().into(),
            meta: None,
        }
    } else if let Some(items) = spec.get("sequence") {
        Candidate::Sequence {
            items: items
                .as_array()
                .unwrap()
                .iter()
                .map(|item| Candidate::Text {
                    text: item.as_str().unwrap().into(),
                    meta: None,
                })
                .collect(),
        }
    } else {
        Candidate::Choice {
            options: spec["choice"]
                .as_array()
                .unwrap()
                .iter()
                .map(|option| ChoiceOption {
                    value: Candidate::Text {
                        text: option["text"].as_str().unwrap().into(),
                        meta: None,
                    },
                    score: option["score"].as_f64(),
                })
                .collect(),
        }
    };
    OutputValue::Data(Arc::new(Packet::new(root)))
}
fn run(input: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    PacketTestData.execute(context(), &[("trigger".into(), input)].into(), config)
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/packet_test_data/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_cases_match() {
    for case in fixture()["cases"].as_array().unwrap() {
        let outputs = run(trigger(), &case["config"]).unwrap();
        assert_eq!(outputs["out"], expected(case), "case={case}");
        if let OutputValue::Data(packet) = &outputs["out"] {
            packet.validate().unwrap();
        }
    }
}
#[test]
fn generation_is_deterministic() {
    let config = json!({ "shape": "choice", "size": 4, "seed": 7 });
    let first = run(trigger(), &config).unwrap();
    let second = run(trigger(), &config).unwrap();
    assert_eq!(first["out"], second["out"]);
}
#[test]
fn failures_remain_distinct() {
    let config = PacketTestData.definition().default_config;
    assert_eq!(config, json!({ "shape": "text", "size": 16, "seed": 0 }));
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap()["out"],
        OutputValue::NoOutput
    );
    let missing: Inputs = [].into();
    assert_eq!(
        PacketTestData
            .execute(context(), &missing, &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            PacketTestData.validate_config(invalid).unwrap_err().code,
            "invalid-config",
            "{invalid}"
        );
    }
}
struct Source;
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new("test.source", 1, vec![], vec![Port::packet("out")])
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([("out".into(), trigger())].into())
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
                config: Some(json!({ "shape": "text", "size": 5, "seed": 0 })),
            },
        ],
        edges: vec![Edge {
            from: Endpoint::new("source", "out"),
            to: Endpoint::new("node", "trigger"),
        }],
    };
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    assert!(report.succeeded());
    assert_eq!(
        report.packet("node", "out").unwrap().root,
        Candidate::Text {
            text: "つけあとう".into(),
            meta: None
        }
    );
}
