#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
#[path = "../src/nodes/japanese_punctuation/mod.rs"]
mod japanese_punctuation;
use crate::test_host::{
    TextMeta,
    graph::{ExecutionPlan, Graph},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext,
    },
    runtime::{Engine, ResolvedValue},
};
use japanese_punctuation::{JapanesePunctuation, KIND, Style, convert};
use serde_json::{Value, json};
use std::sync::Arc;
struct Source(OutputValue);
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
        Ok([("out".into(), self.0.clone())].into())
    }
}
fn run(input: OutputValue, config: Option<Value>) -> crate::test_host::runtime::RunReport {
    let mut registry = Registry::new();
    registry.register(Source(input)).unwrap();
    registry.register(JapanesePunctuation).unwrap();
    let mut node = json!({"id":"punctuation","kind":KIND,"version":1});
    if let Some(config) = config {
        node["config"] = config;
    }
    let graph=Graph::from_json(&json!({"version":1,"nodes":[{"id":"source","kind":"test.source","version":1},node],"edges":[{"from":{"node":"source","port":"out"},"to":{"node":"punctuation","port":"in"}}]}).to_string()).unwrap();
    Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap())
        .run()
        .unwrap()
}
#[test]
fn standalone_cases_use_the_real_node_and_preserve_input_while_omitting_output_meta() {
    let contract: Value = serde_json::from_str(include_str!(
        "../examples/nodes/japanese_punctuation/contract.json"
    ))
    .unwrap();
    for case in contract["cases"].as_array().unwrap() {
        let packet = Arc::new(Packet::new(Candidate::Text {
            text: case["input"].as_str().unwrap().into(),
            meta: Some(TextMeta {
                reading: Some("reading".into()),
                source_span: Some([0, 1]),
            }),
        }));
        let before = (*packet).clone();
        let report = run(
            OutputValue::Data(packet.clone()),
            case.get("config").cloned(),
        );
        let Candidate::Text { text, meta } = &report.packet("punctuation", "out").unwrap().root
        else {
            panic!("expected Text");
        };
        assert_eq!(text, case["expected"].as_str().unwrap());
        assert!(meta.is_none());
        assert_eq!(*packet, before);
        assert!(Arc::ptr_eq(
            report.packet("source", "out").unwrap(),
            &packet
        ));
    }
}
#[test]
fn all_styles_are_idempotent_and_leave_ascii_decimals_and_non_punctuation_unchanged() {
    for style in [Style::Japanese, Style::Fullwidth, Style::Ascii] {
        let input = "3.14,a.b　\t\n😀！？!?,、。．，";
        let once = convert(input, style);
        assert_eq!(convert(&once, style), once);
        assert_eq!(once.chars().count(), input.chars().count());
        assert_eq!(
            convert("3.14,a.b　\t\n😀！？!?", style),
            "3.14,a.b　\t\n😀！？!?"
        );
    }
}
#[test]
fn no_output_and_invalid_config_packet_root_and_absence_remain_distinct() {
    let report = run(OutputValue::NoOutput, None);
    assert!(matches!(
        report.nodes["punctuation"].outputs["out"],
        ResolvedValue::NoOutput
    ));
    for config in [
        Value::Null,
        json!({}),
        json!([]),
        json!({"style":true}),
        json!({"style":"unknown"}),
        json!({"style":"ascii","extra":1}),
    ] {
        assert_eq!(
            JapanesePunctuation
                .validate_config(&config)
                .unwrap_err()
                .code,
            "invalid-config"
        );
    }
    let context = RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "punctuation",
        input: None,
    };
    let config = JapanesePunctuation.definition().default_config;
    assert_eq!(
        JapanesePunctuation
            .execute(context, &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    let mut packet = Packet::new(Candidate::Sequence { items: vec![] });
    let inputs = [("in".into(), OutputValue::Data(Arc::new(packet.clone())))].into();
    assert_eq!(
        JapanesePunctuation
            .execute(context, &inputs, &config)
            .unwrap_err()
            .code,
        "invalid-root"
    );
    packet.version = 2;
    let inputs = [("in".into(), OutputValue::Data(Arc::new(packet)))].into();
    assert_eq!(
        JapanesePunctuation
            .execute(context, &inputs, &config)
            .unwrap_err()
            .code,
        "invalid-packet"
    );
}
