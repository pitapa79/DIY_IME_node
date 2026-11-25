#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::text_number_unit_extract as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextNumberUnitExtract;
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
    let outputs =
        TextNumberUnitExtract.execute(context(), &[("in".into(), input)].into(), config)?;
    Ok(outputs["out"].clone())
}
fn items(input: &str) -> Vec<(String, Option<[usize; 2]>)> {
    items_with(input, &json!({ "limit": 64 }))
}
fn items_with(input: &str, config: &Value) -> Vec<(String, Option<[usize; 2]>)> {
    let OutputValue::Data(packet) = run(data(input), config).unwrap() else {
        panic!("Data required")
    };
    let Candidate::Sequence { items } = &packet.root else {
        panic!("Sequence required")
    };
    items
        .iter()
        .map(|item| {
            let Candidate::Text { text, meta } = item else {
                panic!("Text items required")
            };
            assert!(meta.as_ref().is_none_or(|m| m.reading.is_none()));
            (text.clone(), meta.as_ref().and_then(|m| m.source_span))
        })
        .collect()
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_number_unit_extract/contract.json"
    ))
    .unwrap()
}
#[test]
fn fixture_values_and_spans_are_correct() {
    for case in fixture()["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let result = items_with(input, &case["config"]);
        let expected: Vec<(String, Option<[usize; 2]>)> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                let span = item["span"].as_array().unwrap();
                (
                    item["text"].as_str().unwrap().to_string(),
                    Some([
                        span[0].as_u64().unwrap() as usize,
                        span[1].as_u64().unwrap() as usize,
                    ]),
                )
            })
            .collect();
        assert_eq!(result, expected, "input={input:?}");
    }
}
#[test]
fn signs_ranges_and_limits() {
    // A sign joins the number only when it does not follow a digit.
    assert_eq!(items("気温-5℃"), vec![("-5℃".to_string(), Some([2, 5]))]);
    assert_eq!(items("1-2個"), vec![("2個".to_string(), Some([2, 4]))]);
    // Numbers without units are skipped, and the limit caps the output.
    assert_eq!(items("123"), Vec::<(String, Option<[usize; 2]>)>::new());
    assert_eq!(
        items_with("1個2個", &json!({ "limit": 1 })),
        vec![("1個".to_string(), Some([0, 2]))]
    );
}
#[test]
fn failures_remain_distinct() {
    let config = TextNumberUnitExtract.definition().default_config;
    assert_eq!(config, json!({ "limit": 64 }));
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap(),
        OutputValue::NoOutput
    );
    assert_eq!(
        TextNumberUnitExtract
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
            TextNumberUnitExtract
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
    let mut registry = Registry::new();
    registry.register(Source("全部で5個".into())).unwrap();
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
                config: Some(json!({ "limit": 64 })),
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
        Candidate::Sequence {
            items: vec![Candidate::Text {
                text: "5個".into(),
                meta: Some(TextMeta {
                    reading: None,
                    source_span: Some([3, 5]),
                }),
            }],
        }
    );
}
