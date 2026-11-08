#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
#[path = "../src/nodes/ascii_width/mod.rs"]
mod ascii_width;
use crate::test_host::{
    TextMeta,
    graph::{ExecutionPlan, Graph},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext,
    },
    runtime::{Engine, ResolvedValue},
};
use ascii_width::{AsciiWidth, KIND, Mode, convert};
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
fn run(value: OutputValue, config: Option<Value>) -> crate::test_host::runtime::RunReport {
    let mut registry = Registry::new();
    registry.register(Source(value)).unwrap();
    registry.register(AsciiWidth).unwrap();
    let mut transform = json!({"id":"width","kind":KIND,"version":1});
    if let Some(config) = config {
        transform["config"] = config;
    }
    let graph=Graph::from_json(&json!({"version":1,"nodes":[{"id":"source","kind":"test.source","version":1},transform],"edges":[{"from":{"node":"source","port":"out"},"to":{"node":"width","port":"in"}}]}).to_string()).unwrap();
    Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap())
        .run()
        .unwrap()
}
#[test]
fn isolated_contract_runs_the_real_node_and_preserves_input_but_omits_output_meta() {
    let contract: Value =
        serde_json::from_str(include_str!("../examples/nodes/ascii_width/contract.json")).unwrap();
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
        let Candidate::Text { text, meta } = &report.packet("width", "out").unwrap().root else {
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
fn every_ascii_scalar_roundtrips_and_unrelated_unicode_is_unchanged() {
    let ascii: String = (0..=127)
        .map(|value| char::from_u32(value).unwrap())
        .collect();
    for space in [false, true] {
        let wide = convert(&ascii, Mode::Fullwidth, space);
        assert_eq!(wide.chars().count(), ascii.chars().count());
        assert_eq!(convert(&wide, Mode::Halfwidth, space), ascii);
    }
    let untouched = "\0\t\n\r\u{7f}\u{a0}カタカナｶﾅ漢字゙😀\u{10ffff}";
    assert_eq!(convert(untouched, Mode::Fullwidth, true), untouched);
    assert_eq!(convert(untouched, Mode::Halfwidth, true), untouched);
    assert_eq!(convert("!~", Mode::Fullwidth, false), "！～");
    assert_eq!(convert("！～", Mode::Halfwidth, false), "!~");
}
#[test]
fn no_output_and_all_invalid_categories_are_distinct() {
    let report = run(OutputValue::NoOutput, None);
    assert!(matches!(
        report.nodes["width"].outputs["out"],
        ResolvedValue::NoOutput
    ));
    for config in [
        Value::Null,
        json!({}),
        json!([]),
        json!({"mode":"fullwidth"}),
        json!({"mode":"unknown","space":true}),
        json!({"mode":"fullwidth","space":0}),
        json!({"mode":"halfwidth","space":true,"extra":1}),
    ] {
        assert_eq!(
            AsciiWidth.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
    }
    let context = RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "width",
        input: None,
    };
    let config = AsciiWidth.definition().default_config;
    assert_eq!(
        AsciiWidth
            .execute(context, &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    let mut packet = Packet::new(Candidate::Choice { options: vec![] });
    let inputs = [("in".into(), OutputValue::Data(Arc::new(packet.clone())))].into();
    assert_eq!(
        AsciiWidth
            .execute(context, &inputs, &config)
            .unwrap_err()
            .code,
        "invalid-root"
    );
    packet.version = 2;
    let inputs = [("in".into(), OutputValue::Data(Arc::new(packet)))].into();
    assert_eq!(
        AsciiWidth
            .execute(context, &inputs, &config)
            .unwrap_err()
            .code,
        "invalid-packet"
    );
}
