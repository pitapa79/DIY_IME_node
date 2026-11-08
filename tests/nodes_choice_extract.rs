#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, ChoiceOption, Packet, node, tree};
#[path = "../src/nodes/choice_extract/mod.rs"]
mod subject;
use crate::test_host::{
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    runtime::Engine,
};
use node::{
    CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RootConstraint,
    RunContext,
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::ChoiceExtract;
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "node",
        input: None,
    }
}
fn packet(root: &Value) -> Packet {
    Packet::from_json(&json!({"version":1,"root":root}).to_string()).unwrap()
}
fn fixtures() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/choice_extract/contract.json"
    ))
    .unwrap()
}
fn run(value: OutputValue, config: &Value) -> Result<OutputValue, CoreError> {
    Ok(ChoiceExtract.execute(context(), &[("in".into(), value)].into(), config)?["out"].clone())
}
#[test]
fn contract_cases_use_actual_node_and_do_not_modify_input() {
    for case in fixtures()["cases"].as_array().unwrap() {
        let original = packet(&case["input"]);
        let input = Arc::new(original.clone());
        let output = run(OutputValue::Data(Arc::clone(&input)), &case["config"]).unwrap();
        if case["expected"].is_null() {
            assert_eq!(output, OutputValue::NoOutput);
        } else {
            let OutputValue::Data(result) = output else {
                panic!("expected Data")
            };
            assert_eq!(result.root, packet(&case["expected"]).root);
        }
        assert_eq!(*input, original);
    }
}
#[test]
fn configuration_packet_and_absence_failures_stay_distinct() {
    let config = ChoiceExtract.definition().default_config;
    let mut invalids = vec![Value::Null, json!([])];
    let mut unknown = config.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("unexpected".into(), json!(0));
    invalids.push(unknown);
    for key in config.as_object().unwrap().keys() {
        let mut partial = config.clone();
        partial.as_object_mut().unwrap().remove(key);
        invalids.push(partial);
    }
    for invalid in invalids {
        assert_eq!(
            ChoiceExtract.validate_config(&invalid).unwrap_err().code,
            "invalid-config"
        );
    }
    for invalid in fixtures()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            ChoiceExtract.validate_config(invalid).unwrap_err().code,
            "invalid-config"
        );
    }
    assert_eq!(
        run(OutputValue::NoOutput, &config).unwrap(),
        OutputValue::NoOutput
    );
    assert_eq!(
        ChoiceExtract
            .execute(context(), &Inputs::new(), &config)
            .unwrap_err()
            .code,
        "missing-input"
    );
    let invalid = Packet {
        version: 99,
        root: Candidate::Text {
            text: "x".into(),
            meta: None,
        },
    };
    assert_eq!(
        run(OutputValue::Data(Arc::new(invalid)), &config)
            .unwrap_err()
            .code,
        "invalid-packet"
    );
    let invalid = Packet::new(Candidate::Choice {
        options: vec![ChoiceOption {
            value: Candidate::Text {
                text: "x".into(),
                meta: None,
            },
            score: Some(f64::NAN),
        }],
    });
    assert_eq!(
        run(OutputValue::Data(Arc::new(invalid)), &config)
            .unwrap_err()
            .code,
        "invalid-packet"
    );
    assert_eq!(
        run(
            OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                text: "x".into(),
                meta: None
            }))),
            &config
        )
        .unwrap_err()
        .code,
        "invalid-root"
    );
    for case in fixtures()["errorCases"].as_array().unwrap() {
        assert_eq!(
            run(
                OutputValue::Data(Arc::new(packet(&case["input"]))),
                &case["config"]
            )
            .unwrap_err()
            .code,
            case["error"].as_str().unwrap()
        );
    }
}
struct Source(Packet);
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
        Ok([("out".into(), OutputValue::Data(Arc::new(self.0.clone())))].into())
    }
}
#[test]
fn registry_engine_uses_the_implementation() {
    let fixture = fixtures();
    let case = &fixture["cases"][0];
    let mut registry = Registry::new();
    registry.register(Source(packet(&case["input"]))).unwrap();
    registry.register(ChoiceExtract).unwrap();
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
            packet(&case["expected"]).root
        );
    }
}

fn subject_node() -> ChoiceExtract {
    ChoiceExtract
}

#[test]
fn preserves_nested_scores_and_text_metadata_outside_the_operated_level() {
    let meta = crate::test_host::TextMeta {
        reading: Some("ふかい".into()),
        source_span: Some([1, 3]),
    };
    let deep = Candidate::Sequence {
        items: vec![
            Candidate::Text {
                text: "深い😀".into(),
                meta: Some(meta.clone()),
            },
            Candidate::Choice {
                options: vec![ChoiceOption {
                    value: Candidate::Text {
                        text: "inner".into(),
                        meta: None,
                    },
                    score: Some(7.0),
                }],
            },
        ],
    };
    let root = match subject_node().definition().inputs[0].port.root {
        RootConstraint::Choice => Candidate::Choice {
            options: vec![ChoiceOption {
                value: deep,
                score: Some(2.0),
            }],
        },
        RootConstraint::Sequence => Candidate::Sequence { items: vec![deep] },
        _ => deep,
    };
    let output = run(
        OutputValue::Data(Arc::new(Packet::new(root))),
        &subject_node().definition().default_config,
    )
    .unwrap();
    let OutputValue::Data(output) = output else {
        panic!("default operation must retain the only candidate");
    };
    let mut pending = vec![&output.root];
    let mut metadata_found = false;
    let mut nested_score_found = false;
    while let Some(candidate) = pending.pop() {
        match candidate {
            Candidate::Text { text, meta: actual } => {
                if text == "深い😀" {
                    assert_eq!(actual, &Some(meta.clone()));
                    metadata_found = true;
                }
            }
            Candidate::Sequence { items } => pending.extend(items),
            Candidate::Choice { options } => {
                for option in options {
                    if matches!(&option.value,Candidate::Text{text,..} if text=="inner") {
                        assert_eq!(option.score, Some(7.0));
                        nested_score_found = true;
                    }
                    pending.push(&option.value);
                }
            }
        }
    }
    assert!(metadata_found);
    assert!(nested_score_found);
}
