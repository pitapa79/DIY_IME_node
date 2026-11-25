#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::text_kanji_form as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextKanjiForm;
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
    TextKanjiForm.execute(context(), &[("in".into(), input)].into(), config)
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_kanji_form/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_cases_match() {
    let fixtures = fixture();
    for case in fixtures["cases"].as_array().unwrap() {
        let config = case.get("config").unwrap_or(&fixtures["defaultConfig"]);
        let outputs = run(data(case["input"].as_str().unwrap()), config).unwrap();
        assert_eq!(
            outputs["out"],
            data(case["expected"].as_str().unwrap()),
            "case={case}"
        );
    }
}
#[test]
fn unchanged_input_keeps_the_same_packet_and_meta() {
    let meta = crate::test_host::TextMeta {
        reading: Some("source".into()),
        source_span: Some([0, 1]),
    };
    let packet = Arc::new(Packet::new(Candidate::Text {
        text: "ひらがな".into(),
        meta: Some(meta),
    }));
    let config = TextKanjiForm.definition().default_config;
    let outputs = run(OutputValue::Data(packet.clone()), &config).unwrap();
    let OutputValue::Data(out) = &outputs["out"] else {
        panic!("Data required")
    };
    assert!(Arc::ptr_eq(out, &packet));
}
#[test]
fn ambiguous_groups_follow_the_documented_rule() {
    // to-shinjitai collapses every source; to-kyujitai uses the principal form.
    let to_new = json!({ "direction": "to-shinjitai" });
    let to_old = json!({ "direction": "to-kyujitai" });
    for source in ["辨", "瓣", "辯"] {
        let outputs = run(data(source), &to_new).unwrap();
        assert_eq!(outputs["out"], data("弁"), "{source}");
    }
    let outputs = run(data("弁"), &to_old).unwrap();
    assert_eq!(outputs["out"], data("辨"));
    for source in ["邊", "邉"] {
        let outputs = run(data(source), &to_new).unwrap();
        assert_eq!(outputs["out"], data("辺"), "{source}");
    }
    let outputs = run(data("辺"), &to_old).unwrap();
    assert_eq!(outputs["out"], data("邊"));
}
#[test]
fn failures_remain_distinct() {
    let config = TextKanjiForm.definition().default_config;
    assert_eq!(config, json!({ "direction": "to-shinjitai" }));
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap()["out"],
        OutputValue::NoOutput
    );
    let missing: Inputs = [].into();
    assert_eq!(
        TextKanjiForm
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
            TextKanjiForm.validate_config(invalid).unwrap_err().code,
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
        Ok([("out".into(), data("國語"))].into())
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
                config: Some(json!({ "direction": "to-shinjitai" })),
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
            text: "国語".into(),
            meta: None
        }
    );
}
