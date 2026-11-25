#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};
use crate::test_host::{
    ChoiceOption, TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use diy_ime2_nodes::opt_in::score_format as subject;
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::ScoreFormat;
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "test",
        input: None,
    }
}
fn build_choice(case: &Value) -> OutputValue {
    let options = case["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| {
            let value = if let Some(text) = candidate.get("text") {
                Candidate::Text {
                    text: text.as_str().unwrap().into(),
                    meta: None,
                }
            } else {
                Candidate::Sequence {
                    items: candidate["sequence"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|item| Candidate::Text {
                            text: item.as_str().unwrap().into(),
                            meta: None,
                        })
                        .collect(),
                }
            };
            ChoiceOption {
                value,
                score: candidate["score"].as_f64(),
            }
        })
        .collect();
    OutputValue::Data(Arc::new(Packet::new(Candidate::Choice { options })))
}
fn expected_sequence(case: &Value) -> OutputValue {
    let items = case["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| Candidate::Text {
            text: item["text"].as_str().unwrap().into(),
            meta: item["reading"].as_str().map(|reading| TextMeta {
                reading: Some(reading.into()),
                source_span: None,
            }),
        })
        .collect();
    OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items })))
}
fn run(input: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    ScoreFormat.execute(context(), &[("in".into(), input)].into(), config)
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../examples/nodes/score_format/contract.json")).unwrap()
}
#[test]
fn fixture_cases_match() {
    let fixtures = fixture();
    for case in fixtures["cases"].as_array().unwrap() {
        let config = case.get("config").unwrap_or(&fixtures["defaultConfig"]);
        let outputs = run(build_choice(case), config).unwrap();
        assert_eq!(outputs["out"], expected_sequence(case), "case={case}");
    }
}
#[test]
fn percent_format_does_not_overflow_for_finite_scores() {
    let input = OutputValue::Data(Arc::new(Packet::new(Candidate::Choice {
        options: vec![ChoiceOption {
            value: Candidate::Text {
                text: "large".into(),
                meta: None,
            },
            score: Some(f64::MAX),
        }],
    })));
    let outputs = run(input, &json!({"decimals":2,"percent":true})).unwrap();
    let OutputValue::Data(packet) = &outputs["out"] else {
        panic!("Data required");
    };
    let Candidate::Sequence { items } = &packet.root else {
        panic!("Sequence required");
    };
    let Candidate::Text { text, .. } = &items[0] else {
        panic!("Text required");
    };
    assert!(text.starts_with("179769313486231"));
    assert!(text.ends_with("00.00%"));
    assert_eq!(text.len(), 315);
}
#[test]
fn failures_remain_distinct() {
    let config = ScoreFormat.definition().default_config;
    assert_eq!(config, json!({ "decimals": 2, "percent": false }));
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap()["out"],
        OutputValue::NoOutput
    );
    let missing: Inputs = [].into();
    assert_eq!(
        ScoreFormat
            .execute(context(), &missing, &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    let wrong_root = OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
        text: "x".into(),
        meta: None,
    })));
    assert_eq!(run(wrong_root, &config).unwrap_err().code, "invalid-root");
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            ScoreFormat.validate_config(invalid).unwrap_err().code,
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
                    .with_root(RootConstraint::Choice),
            ],
        )
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        Ok([(
            "out".into(),
            build_choice(&json!({"candidates":[{"text":"か","score":0.5}]})),
        )]
        .into())
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
                config: Some(json!({ "decimals": 2, "percent": false })),
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
                text: "0.50".into(),
                meta: Some(TextMeta {
                    reading: Some("か".into()),
                    source_span: None
                })
            }]
        }
    );
}
