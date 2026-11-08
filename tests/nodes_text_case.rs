#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
#[path = "../src/nodes/text_case/mod.rs"]
mod subject;
use crate::test_host::{
    TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextCase;
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
    let outputs = TextCase.execute(context(), &[("in".into(), input)].into(), config)?;
    Ok(outputs["out"].clone())
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../examples/nodes/text_case/contract.json")).unwrap()
}
#[test]
fn fixture_values_and_metadata_are_correct() {
    for case in fixture()["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let expected = case["expected"].as_str().unwrap();
        let meta = TextMeta {
            reading: Some("source".into()),
            source_span: Some([0, input.chars().count()]),
        };
        let packet = Arc::new(Packet::new(Candidate::Text {
            text: input.into(),
            meta: Some(meta.clone()),
        }));
        let result = run(OutputValue::Data(Arc::clone(&packet)), &case["config"]).unwrap();
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
        assert_eq!(out_meta, &if input == expected { Some(meta) } else { None });
        assert_eq!(
            packet.root,
            Candidate::Text {
                text: input.into(),
                meta: Some(TextMeta {
                    reading: Some("source".into()),
                    source_span: Some([0, input.chars().count()])
                })
            }
        );
    }
}
#[test]
fn failures_remain_distinct() {
    let config = TextCase.definition().default_config;
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap(),
        OutputValue::NoOutput
    );
    assert_eq!(
        TextCase
            .execute(context(), &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    assert_eq!(
        run(
            OutputValue::Data(Arc::new(Packet {
                version: 99,
                root: Candidate::Text {
                    text: "a".into(),
                    meta: None
                }
            })),
            &config
        )
        .unwrap_err()
        .code,
        "invalid-packet"
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
    let mut unknown = config.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("unexpected".into(), json!(true));
    for invalid in [Value::Null, json!({}), json!([]), unknown] {
        assert_eq!(
            TextCase.validate_config(&invalid).unwrap_err().code,
            "invalid-config"
        );
    }
    for key in config.as_object().unwrap().keys() {
        let mut partial = config.clone();
        partial.as_object_mut().unwrap().remove(key);
        assert_eq!(
            TextCase.validate_config(&partial).unwrap_err().code,
            "invalid-config"
        );
    }
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            TextCase.validate_config(invalid).unwrap_err().code,
            "invalid-config"
        );
    }
    for case in fixture()["errorCases"].as_array().unwrap() {
        assert_eq!(
            run(data(case["input"].as_str().unwrap()), &case["config"])
                .unwrap_err()
                .code,
            case["error"].as_str().unwrap()
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
    let case = &fixtures["cases"][0];
    let mut registry = Registry::new();
    registry
        .register(Source(case["input"].as_str().unwrap().into()))
        .unwrap();
    registry.register(TextCase).unwrap();
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
    for _ in 0..2 {
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
}
