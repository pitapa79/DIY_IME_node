#[path = "support/mod.rs"]
mod test_host;
pub use crate::test_host::{Candidate, ChoiceOption, Packet, node, tree};
#[path = "../src/nodes/choice_deduplicate/mod.rs"]
mod subject;

use crate::test_host::{
    TextMeta,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext,
    },
    runtime::{Engine, NodeStatus, ResolvedValue},
};
use serde_json::{Value, json};
use std::sync::Arc;
use subject::{ChoiceDeduplicate, KIND, deduplicate};

fn text(value: &str) -> Candidate {
    Candidate::Text {
        text: value.into(),
        meta: None,
    }
}
fn option(value: Candidate, score: Option<f64>) -> ChoiceOption {
    ChoiceOption { value, score }
}
fn choice(options: Vec<ChoiceOption>) -> Packet {
    Packet::new(Candidate::Choice { options })
}
fn options(packet: &Packet) -> &[ChoiceOption] {
    let Candidate::Choice { options } = &packet.root else {
        panic!("expected Choice")
    };
    options
}
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "dedup",
        input: None,
    }
}
fn run(input: OutputValue, config: &Value) -> Result<Outputs, CoreError> {
    ChoiceDeduplicate.execute(context(), &[("in".into(), input)].into(), config)
}

#[test]
fn fixture_cases_are_executed_against_the_node() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../examples/nodes/choice_deduplicate/contract.json"
    ))
    .unwrap();
    assert_eq!(fixture["kind"], KIND);
    for case in fixture["cases"].as_array().unwrap() {
        let input = Packet::from_json(&case["input"].to_string()).unwrap();
        let expected = Packet::from_json(&case["expected"].to_string()).unwrap();
        let output = run(OutputValue::Data(Arc::new(input)), &fixture["config"]).unwrap();
        assert_eq!(
            output["out"],
            OutputValue::Data(Arc::new(expected)),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn keeps_first_score_order_and_input_unchanged() {
    let input = choice(vec![
        option(text("a"), None),
        option(text("b"), Some(2.0)),
        option(text("a"), Some(99.0)),
        option(text("b"), None),
    ]);
    let before = input.clone();
    for _ in 0..3 {
        let result = deduplicate(&input).unwrap();
        assert_eq!(options(&result), &options(&before)[..2]);
        assert_eq!(input, before);
    }
}

#[test]
fn metadata_unicode_and_structure_are_significant() {
    let with_meta = |span| Candidate::Text {
        text: "𠮷👩‍💻".into(),
        meta: Some(TextMeta {
            reading: Some("よし".into()),
            source_span: span,
        }),
    };
    let distinct = vec![
        text("é"),
        text("e\u{301}"),
        text("𠮷👩‍💻"),
        with_meta(None),
        with_meta(Some([0, 1])),
        Candidate::Sequence {
            items: vec![text("a"), text("b")],
        },
        Candidate::Sequence {
            items: vec![text("b"), text("a")],
        },
        Candidate::Sequence { items: vec![] },
        text(""),
    ];
    let mut source: Vec<_> = distinct
        .iter()
        .cloned()
        .map(|value| option(value, None))
        .collect();
    source.extend(distinct.into_iter().map(|value| option(value, Some(100.0))));
    let result = deduplicate(&choice(source.clone())).unwrap();
    assert_eq!(options(&result), &source[..9]);
}

#[test]
fn nested_choices_keep_their_duplicates_and_scores() {
    let nested = |score| Candidate::Choice {
        options: vec![option(text("同じ"), score), option(text("同じ"), Some(1.0))],
    };
    let first = nested(Some(1.0));
    let second = nested(Some(2.0));
    let missing = nested(None);
    let result = deduplicate(&choice(vec![
        option(first.clone(), Some(8.0)),
        option(first.clone(), Some(99.0)),
        option(second.clone(), None),
        option(missing.clone(), None),
    ]))
    .unwrap();
    assert_eq!(
        options(&result),
        &[
            option(first, Some(8.0)),
            option(second, None),
            option(missing, None)
        ]
    );
}

#[test]
fn empty_choice_remains_data_and_no_output_remains_no_output() {
    let empty = choice(vec![]);
    assert_eq!(deduplicate(&empty).unwrap(), empty);
    assert_eq!(
        run(OutputValue::NoOutput, &json!({})).unwrap()["out"],
        OutputValue::NoOutput
    );
}

#[test]
fn rejects_invalid_settings_even_without_data() {
    for config in [
        Value::Null,
        json!([]),
        json!({"unexpected":true}),
        json!(false),
    ] {
        assert_eq!(
            run(OutputValue::NoOutput, &config).unwrap_err().code,
            "invalid-config"
        );
    }
    assert_eq!(
        ChoiceDeduplicate
            .execute(context(), &Inputs::new(), &json!({}))
            .unwrap_err()
            .code,
        "missing-input"
    );
}

#[test]
fn rejects_invalid_packets_and_non_choice_roots() {
    for root in [text("text"), Candidate::Sequence { items: vec![] }] {
        assert_eq!(
            deduplicate(&Packet::new(root)).unwrap_err().code,
            "invalid-root"
        );
    }
    let invalid_score = choice(vec![option(text("x"), Some(f64::INFINITY))]);
    assert_eq!(
        deduplicate(&invalid_score).unwrap_err().code,
        "invalid-packet"
    );
    let invalid_meta = choice(vec![option(
        Candidate::Text {
            text: "x".into(),
            meta: Some(TextMeta {
                reading: None,
                source_span: Some([2, 1]),
            }),
        },
        None,
    )]);
    assert_eq!(
        deduplicate(&invalid_meta).unwrap_err().code,
        "invalid-packet"
    );
    let mut invalid_version = choice(vec![]);
    invalid_version.version = 2;
    assert_eq!(
        deduplicate(&invalid_version).unwrap_err().code,
        "invalid-packet"
    );
}

#[test]
fn compares_and_reconstructs_deep_values_without_a_new_depth_limit() {
    let deep = || {
        let mut value = text("deep");
        for _ in 0..128 {
            value = Candidate::Sequence { items: vec![value] };
        }
        value
    };
    let result = deduplicate(&choice(vec![
        option(deep(), Some(1.0)),
        option(deep(), Some(2.0)),
    ]))
    .unwrap();
    assert_eq!(options(&result).len(), 1);
    assert_eq!(options(&result)[0].score, Some(1.0));
    let mut value = &options(&result)[0].value;
    for _ in 0..128 {
        let Candidate::Sequence { items } = value else {
            panic!("shape changed")
        };
        value = &items[0];
    }
    assert_eq!(value, &text("deep"));
}

struct Source(OutputValue);
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.dedup-source",
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
        Ok([("out".into(), self.0.clone())].into())
    }
}
fn graph() -> Graph {
    Graph {
        version: 1,
        nodes: vec![
            NodeInstance {
                id: "source".into(),
                kind: "test.dedup-source".into(),
                version: 1,
                config: None,
            },
            NodeInstance {
                id: "dedup".into(),
                kind: KIND.into(),
                version: 1,
                config: None,
            },
        ],
        edges: vec![Edge {
            from: Endpoint::new("source", "out"),
            to: Endpoint::new("dedup", "in"),
        }],
    }
}

#[test]
fn private_registry_engine_executes_and_skips_required_no_output() {
    let input = choice(vec![
        option(text("選択肢"), Some(2.0)),
        option(text("選択肢"), Some(9.0)),
    ]);
    for source in [OutputValue::Data(Arc::new(input)), OutputValue::NoOutput] {
        let skip = matches!(source, OutputValue::NoOutput);
        let mut registry = Registry::new();
        registry.register(Source(source)).unwrap();
        registry.register(ChoiceDeduplicate).unwrap();
        let mut definition = graph();
        let mut engine = Engine::new(ExecutionPlan::compile(&definition, &registry).unwrap());
        let report = engine.run().unwrap();
        assert!(report.succeeded());
        if skip {
            assert_eq!(report.nodes["dedup"].status, NodeStatus::Skipped);
            assert!(matches!(
                report.nodes["dedup"].outputs["out"],
                ResolvedValue::NoOutput
            ));
        } else {
            assert_eq!(
                options(report.packet("dedup", "out").unwrap()),
                &[option(text("選択肢"), Some(2.0))]
            );
        }
        definition.nodes[1].config = Some(json!({"extra":0}));
        assert_eq!(
            ExecutionPlan::compile(&definition, &registry)
                .err()
                .unwrap()
                .code,
            "invalid-config"
        );
    }
}
