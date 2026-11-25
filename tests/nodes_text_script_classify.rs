#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::text_script_classify as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextScriptClassify;
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
fn run(input: OutputValue, config: &Value) -> Result<OutputValue, CoreError> {
    let outputs = TextScriptClassify.execute(context(), &[("in".into(), input)].into(), config)?;
    Ok(outputs["out"].clone())
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_script_classify/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_values_are_correct_and_meta_is_never_inherited() {
    for case in fixture()["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let expected = case["expected"].as_str().unwrap();
        let meta = TextMeta {
            reading: Some("source".into()),
            source_span: Some([0, input.chars().count()]),
        };
        let packet = Arc::new(Packet::new(Candidate::Text {
            text: input.into(),
            meta: Some(meta),
        }));
        let result = run(OutputValue::Data(packet), &case["config"]).unwrap();
        let OutputValue::Data(out) = result else {
            panic!("empty Text must remain Data");
        };
        let Candidate::Text {
            text,
            meta: out_meta,
        } = &out.root
        else {
            panic!("Text required")
        };
        assert_eq!(text, expected, "input={input:?}");
        assert_eq!(out_meta, &None, "the count never inherits meta: {input:?}");
    }
}
#[test]
fn classification_rules() {
    let label = |input: &str| {
        let OutputValue::Data(packet) = run(data(input), &json!({})).unwrap() else {
            panic!("Data required")
        };
        let Candidate::Text { text, .. } = &packet.root else {
            panic!("Text required")
        };
        text.clone()
    };
    // Neutral characters never decide the label on their own.
    assert_eq!(label("ひらがな、テスト"), "mixed");
    assert_eq!(label("ひらがな、"), "hiragana");
    assert_eq!(label("2024年"), "mixed");
    assert_eq!(label("年"), "kanji");
    // Combining marks inherit the previous class.
    assert_eq!(label("か\u{3099}"), "hiragana");
    assert_eq!(label("\u{3099}"), "neutral");
}
#[test]
fn katakana_block_punctuation_is_neutral() {
    assert_eq!(run(data("・"), &json!({})).unwrap(), data("neutral"));
    assert_eq!(run(data("a・"), &json!({})).unwrap(), data("latin"));
}
#[test]
fn failures_remain_distinct() {
    let config = TextScriptClassify.definition().default_config;
    assert_eq!(config, json!({}));
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap(),
        OutputValue::NoOutput
    );
    assert_eq!(
        TextScriptClassify
            .execute(context(), &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    assert_eq!(
        run(
            OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items: vec![] }))),
            &config
        )
        .unwrap_err()
        .code,
        "invalid-root"
    );
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            TextScriptClassify
                .validate_config(invalid)
                .unwrap_err()
                .code,
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
    let fixtures = fixture();
    let case = &fixtures["cases"][1];
    let mut registry = Registry::new();
    registry
        .register(Source(case["input"].as_str().unwrap().into()))
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
                config: Some(case["config"].clone()),
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
            text: case["expected"].as_str().unwrap().into(),
            meta: None
        }
    );
}
