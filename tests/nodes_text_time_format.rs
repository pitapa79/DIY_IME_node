#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::text_time_format as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::TextTimeFormat;
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
    let outputs = TextTimeFormat.execute(context(), &[("in".into(), input)].into(), config)?;
    Ok(outputs["out"].clone())
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_time_format/contract.json"
    ))
    .unwrap()
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
        assert_eq!(out_meta, &if input == expected { Some(meta) } else { None });
    }
}
#[test]
fn noon_midnight_and_24h_boundaries() {
    let to_h12 = json!({ "from": "h24", "to": "h12" });
    let text = |input: &str, config: &Value| {
        let OutputValue::Data(packet) = run(data(input), config).unwrap() else {
            panic!("Data required")
        };
        let Candidate::Text { text, .. } = &packet.root else {
            panic!("Text required")
        };
        text.clone()
    };
    assert_eq!(text("0:00", &to_h12), "12:00 AM");
    assert_eq!(text("12:00", &to_h12), "12:00 PM");
    assert_eq!(
        text("24:00", &json!({ "from": "h24", "to": "h24" })),
        "24:00"
    );
    // Seconds presence survives conversion.
    assert_eq!(text("1:02", &to_h12), "1:02 AM");
    assert_eq!(text("1:02:03", &to_h12), "1:02:03 AM");
}
#[test]
fn invalid_times_are_rejected() {
    for (input, from, to) in [
        ("24:00", "h24", "h12"),
        ("24:00", "h24", "japanese"),
        ("24:01", "h24", "h24"),
        ("25:00", "h24", "h12"),
        ("12:60", "h24", "h12"),
        ("12:00:60", "h24", "h12"),
        ("0:30 AM", "h12", "h24"),
        ("13:00 PM", "h12", "h24"),
        ("12:5", "h24", "h12"),
        ("午前0時30分", "japanese", "h24"),
        ("午後13時00分", "japanese", "h24"),
        ("午前9時", "japanese", "h24"),
        ("", "h24", "h12"),
    ] {
        let config = json!({ "from": from, "to": to });
        assert_eq!(
            run(data(input), &config).unwrap_err().code,
            "invalid-time",
            "{input}"
        );
    }
}
#[test]
fn failures_remain_distinct() {
    let config = TextTimeFormat.definition().default_config;
    assert_eq!(config, json!({ "from": "h24", "to": "japanese" }));
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap(),
        OutputValue::NoOutput
    );
    assert_eq!(
        TextTimeFormat
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
            TextTimeFormat.validate_config(invalid).unwrap_err().code,
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
    let case = &fixtures["cases"][0];
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
