#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, Packet, node};

#[path = "../src/nodes/hiragana_katakana/mod.rs"]
mod subject;

use std::sync::Arc;

use crate::test_host::{
    ChoiceOption, TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext,
    },
    runtime::{Engine, NodeStatus},
};
use serde_json::{Value, json};
use subject::{HiraganaKatakana, KIND};

fn text(value: &str) -> Candidate {
    Candidate::Text {
        text: value.into(),
        meta: None,
    }
}
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "kana",
        input: None,
    }
}
fn run(value: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    HiraganaKatakana.execute(context(), &[("in".into(), value)].into(), config)
}
fn data(packet: Packet) -> OutputValue {
    OutputValue::Data(Arc::new(packet))
}

#[test]
fn matches_all_independent_contract_cases() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../examples/nodes/hiragana_katakana/contract.json"
    ))
    .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 12);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let input = if case["inputStatus"] == "no-output" {
            OutputValue::NoOutput
        } else {
            data(serde_json::from_value(case["packet"].clone()).unwrap())
        };
        let result = run(
            input,
            case.get("config").unwrap_or(&fixture["defaultConfig"]),
        );
        match case["expected"]["status"].as_str().unwrap() {
            "data" => {
                let result = result.unwrap_or_else(|error| panic!("{id}: {error}"));
                let OutputValue::Data(packet) = &result["out"] else {
                    panic!("{id}: expected Data")
                };
                assert_eq!(
                    serde_json::to_value(packet.as_ref()).unwrap(),
                    case["expected"]["packet"],
                    "{id}"
                );
            }
            "no-output" => assert_eq!(result.unwrap()["out"], OutputValue::NoOutput, "{id}"),
            "error" => assert_eq!(result.unwrap_err().code, case["expected"]["code"], "{id}"),
            other => panic!("unknown status: {other}"),
        }
    }
}

#[test]
fn preserves_unmapped_unicode_boundaries_and_combining_characters() {
    let original =
        "\u{3040}\u{3097}\u{3098}\u{3099}\u{309a}\u{309b}\u{309c}\u{309f}ー・ｶﾞ𠮷👩‍💻e\u{301}\r\n";
    let result = run(data(Packet::new(text(original))), &json!({})).unwrap();
    assert_eq!(result["out"], data(Packet::new(text(original))));
    let result = run(data(Packet::new(text("か\u{3099}が"))), &json!({})).unwrap();
    assert_eq!(result["out"], data(Packet::new(text("カ\u{3099}ガ"))));
}

#[test]
fn does_not_mutate_input_and_preserves_metadata_only_when_unchanged() {
    for (original, expected, changed) in [("かな", "カナ", true), ("カナ", "カナ", false)] {
        let meta = Some(TextMeta {
            reading: Some("かな".into()),
            source_span: Some([2, 4]),
        });
        let packet = Arc::new(Packet::new(Candidate::Text {
            text: original.into(),
            meta: meta.clone(),
        }));
        let before = packet.to_json().unwrap();
        for _ in 0..3 {
            let result = run(OutputValue::Data(packet.clone()), &json!({})).unwrap();
            let expected = Packet::new(Candidate::Text {
                text: expected.into(),
                meta: if changed { None } else { meta.clone() },
            });
            assert_eq!(result["out"], data(expected));
            assert_eq!(packet.to_json().unwrap(), before);
        }
    }
}

#[test]
fn rejects_invalid_configs_and_missing_input_before_processing() {
    for config in [
        json!(null),
        json!([]),
        json!(0),
        json!(false),
        json!(""),
        json!({"normalize":true}),
    ] {
        assert_eq!(
            HiraganaKatakana.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        assert_eq!(
            run(OutputValue::NoOutput, &config).unwrap_err().code,
            "invalid-config"
        );
    }
    assert_eq!(
        HiraganaKatakana
            .execute(context(), &Inputs::new(), &json!({}))
            .unwrap_err()
            .code,
        "missing-input"
    );
}

#[test]
fn validates_the_entire_packet_before_rejecting_the_root() {
    let invalid = Packet::new(Candidate::Choice {
        options: vec![ChoiceOption {
            value: text("かな"),
            score: Some(f64::NAN),
        }],
    });
    assert_eq!(
        run(data(invalid), &json!({})).unwrap_err().code,
        "invalid-packet"
    );
    let invalid = Packet::new(Candidate::Text {
        text: "かな".into(),
        meta: Some(TextMeta {
            reading: None,
            source_span: Some([3, 1]),
        }),
    });
    assert_eq!(
        run(data(invalid), &json!({})).unwrap_err().code,
        "invalid-packet"
    );
}

struct Source(OutputValue);
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.hiragana-source",
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
fn instance(id: &str, kind: &str) -> NodeInstance {
    NodeInstance {
        id: id.into(),
        kind: kind.into(),
        version: 1,
        config: None,
    }
}

#[test]
fn executes_multiple_instances_in_its_own_registry_and_skips_no_output() {
    for source in [data(Packet::new(text("ひらがな"))), OutputValue::NoOutput] {
        let mut registry = Registry::new();
        registry.register(HiraganaKatakana).unwrap();
        registry.register(Source(source.clone())).unwrap();
        let graph = Graph {
            version: 1,
            nodes: vec![
                instance("source", "test.hiragana-source"),
                instance("a", KIND),
                instance("b", KIND),
            ],
            edges: ["a", "b"]
                .into_iter()
                .map(|target| Edge {
                    from: Endpoint::new("source", "out"),
                    to: Endpoint::new(target, "in"),
                })
                .collect(),
        };
        let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
        for _ in 0..2 {
            let report = engine.run().unwrap();
            assert!(report.succeeded());
            for id in ["a", "b"] {
                if matches!(source, OutputValue::NoOutput) {
                    assert_eq!(report.nodes[id].status, NodeStatus::Skipped);
                } else {
                    assert_eq!(report.packet(id, "out").unwrap().root, text("ヒラガナ"));
                }
            }
        }
    }
}

#[test]
fn declares_text_ja_ports_and_no_settings() {
    let definition = HiraganaKatakana.definition();
    assert_eq!(definition.kind, KIND);
    assert_eq!(definition.version, 1);
    assert_eq!(definition.default_config, json!({}));
    assert_eq!(definition.inputs.len(), 1);
    assert_eq!(definition.outputs.len(), 1);
    assert!(definition.inputs[0].required);
    for port in [&definition.inputs[0].port, &definition.outputs[0]] {
        assert_eq!(port.language.as_deref(), Some("ja"));
        assert_eq!(port.root, RootConstraint::Text);
    }
}
