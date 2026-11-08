#[path = "support/mod.rs"]
mod test_host;
use crate::test_host::{
    Candidate, ChoiceOption, Packet,
    builtins::{
        initial_e2e::{
            EXPECTED, HIRAGANA, KanaKanjiCandidates, ROMAJI, RomajiToHiragana, Segment,
            SelectCandidates, Stringify,
        },
        register_builtin_nodes,
    },
    graph::{ExecutionPlan, Graph},
    node::{CoreError, Inputs, Node, OutputValue, Outputs, Registry, RunContext},
    runtime::{Engine, NodeStatus, RunReport},
};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::Arc,
};

fn sample() -> Graph {
    Graph::from_json(include_str!("../examples/graphs/weather.json")).unwrap()
}
fn registry() -> Registry {
    let mut registry = Registry::new();
    register_builtin_nodes(&mut registry).unwrap();
    registry
}
fn engine(graph: &Graph) -> Engine {
    Engine::new(ExecutionPlan::compile(graph, &registry()).unwrap())
}
fn text(value: &str) -> Candidate {
    Candidate::Text {
        text: value.into(),
        meta: None,
    }
}
fn root<'a>(report: &'a RunReport, id: &str) -> &'a Candidate {
    &report.packet(id, "out").unwrap().root
}
fn sequence(candidate: &Candidate) -> &[Candidate] {
    let Candidate::Sequence { items } = candidate else {
        panic!("expected Sequence")
    };
    items
}
fn choice(candidate: &Candidate) -> &[ChoiceOption] {
    let Candidate::Choice { options } = candidate else {
        panic!("expected Choice")
    };
    options
}
fn content(candidate: &Candidate) -> &str {
    let Candidate::Text { text, .. } = candidate else {
        panic!("expected Text")
    };
    text
}
fn execute(node: &impl Node, root: Candidate, config: Value) -> Result<Outputs, CoreError> {
    let inputs: Inputs = [("in".into(), OutputValue::Data(Arc::new(Packet::new(root))))].into();
    node.execute(
        RunContext {
            run_id: 1,
            plan_version: 1,
            node_id: "test",
            input: None,
        },
        &inputs,
        &config,
    )
}
fn emitted(outputs: &Outputs) -> &Candidate {
    let OutputValue::Data(packet) = &outputs["out"] else {
        panic!("expected data")
    };
    &packet.root
}

#[test]
fn weather_graph_runs_seven_stages_and_is_repeatable() {
    let mut engine = engine(&sample());
    let first = engine.run().unwrap();
    let second = engine.run().unwrap();
    assert!(first.succeeded());
    assert!(second.succeeded());
    assert_ne!(first.run_id, second.run_id);
    assert_eq!(first.nodes.len(), 7);
    for id in [
        "input", "hiragana", "segments", "kanji", "select", "result", "output",
    ] {
        assert_eq!(first.nodes[id].status, NodeStatus::Executed);
        assert_eq!(first.packet(id, "out"), second.packet(id, "out"));
    }
    assert_eq!(content(root(&first, "input")), ROMAJI);
    assert_eq!(content(root(&first, "hiragana")), HIRAGANA);
    assert_eq!(sequence(root(&first, "select")).len(), 6);
    assert_eq!(content(root(&first, "result")), EXPECTED);
    assert_eq!(content(root(&first, "output")), EXPECTED);
}

#[test]
fn segmentation_preserves_alternative_sequences_and_code_point_spans() {
    let report = engine(&sample()).run().unwrap();
    let options = choice(root(&report, "segments"));
    assert_eq!(options.len(), 2);
    assert_eq!(options[0].score, Some(85.0));
    assert_eq!(options[1].score, Some(15.0));
    assert_eq!(
        sequence(&options[0].value)
            .iter()
            .map(content)
            .collect::<Vec<_>>(),
        ["ほんじつ", "の", "こうすいかくりつ", "は", "30%", "です"]
    );
    assert_eq!(
        sequence(&options[1].value)
            .iter()
            .map(content)
            .collect::<Vec<_>>(),
        [
            "ほんじつ",
            "の",
            "こうすい",
            "かくりつ",
            "は",
            "30%",
            "です"
        ]
    );
    let chars: Vec<_> = HIRAGANA.chars().collect();
    for option in options {
        let mut end = 0;
        for item in sequence(&option.value) {
            let Candidate::Text {
                text,
                meta: Some(meta),
            } = item
            else {
                panic!("missing metadata")
            };
            let [start, stop] = meta.source_span.unwrap();
            assert_eq!(start, end);
            assert_eq!(chars[start..stop].iter().collect::<String>(), *text);
            assert_eq!(meta.reading.as_deref(), Some(text.as_str()));
            end = stop;
        }
        assert_eq!(end, chars.len());
    }
}

#[test]
fn dictionary_keeps_nested_choices_scores_particles_and_metadata() {
    let report = engine(&sample()).run().unwrap();
    let before = choice(root(&report, "segments"));
    let after = choice(root(&report, "kanji"));
    assert_eq!(after.len(), before.len());
    for (source, converted) in before.iter().zip(after) {
        assert_eq!(source.score, converted.score);
        let input = sequence(&source.value);
        let output = sequence(&converted.value);
        assert_eq!(input.len(), output.len());
        for (original, result) in input.iter().zip(output) {
            let Candidate::Text {
                text: reading,
                meta,
            } = original
            else {
                unreachable!()
            };
            if ["ほんじつ", "こうすいかくりつ", "こうすい", "かくりつ"].contains(&reading.as_str())
            {
                let variants = choice(result);
                assert!(variants.len() >= 2);
                for variant in variants {
                    let Candidate::Text {
                        meta: converted_meta,
                        ..
                    } = &variant.value
                    else {
                        panic!("expected Text")
                    };
                    assert_eq!(converted_meta, meta);
                }
            } else {
                assert_eq!(original, result);
            }
        }
    }
    assert_eq!(
        content(&choice(&sequence(&after[0].value)[0])[0].value),
        "本日"
    );
    assert_eq!(
        content(&choice(&sequence(&after[0].value)[2])[0].value),
        "降水確率"
    );
}

#[test]
fn old_missing_kaku_input_is_not_corrected_to_the_expected_sentence() {
    let graph = Graph::from_json(include_str!("../examples/graphs/weather-negative.json")).unwrap();
    let report = engine(&graph).run().unwrap();
    assert!(report.succeeded());
    assert_eq!(
        content(root(&report, "hiragana")),
        "ほんじつのこうすいりつは30%です"
    );
    assert_eq!(content(root(&report, "output")), "本日の降水率は30%です");
    assert_ne!(content(root(&report, "output")), EXPECTED);
}

#[test]
fn scores_are_local_to_each_choice_and_selection_does_not_stringify() {
    // 外側の 2 と 1 を比較。内側の 9999 を外側と比較・加算しない。
    let candidate = Candidate::Choice {
        options: vec![
            ChoiceOption {
                score: Some(2.0),
                value: Candidate::Sequence {
                    items: vec![
                        Candidate::Choice {
                            options: vec![
                                ChoiceOption {
                                    score: Some(-20.0),
                                    value: text("先"),
                                },
                                ChoiceOption {
                                    score: Some(-10.0),
                                    value: text("本日"),
                                },
                            ],
                        },
                        text("30%"),
                    ],
                },
            },
            ChoiceOption {
                score: Some(1.0),
                value: Candidate::Choice {
                    options: vec![ChoiceOption {
                        score: Some(9999.0),
                        value: text("誤選択"),
                    }],
                },
            },
        ],
    };
    let selected = execute(&SelectCandidates, candidate, json!({"unscored":0})).unwrap();
    assert_eq!(
        emitted(&selected),
        &Candidate::Sequence {
            items: vec![text("本日"), text("30%")]
        }
    );
    let result = execute(&Stringify, emitted(&selected).clone(), json!({})).unwrap();
    assert_eq!(content(emitted(&result)), "本日30%");
    let tied = Candidate::Choice {
        options: vec![
            ChoiceOption {
                score: None,
                value: text("先"),
            },
            ChoiceOption {
                score: Some(7.0),
                value: text("後"),
            },
        ],
    };
    assert_eq!(
        content(emitted(
            &execute(&SelectCandidates, tied, json!({"unscored":7})).unwrap()
        )),
        "先"
    );
}

#[test]
fn empty_candidates_invalid_packets_and_unselected_choices_are_distinct() {
    assert_eq!(
        execute(
            &SelectCandidates,
            Candidate::Choice { options: vec![] },
            json!({"unscored":0})
        )
        .unwrap()["out"],
        OutputValue::NoOutput
    );
    let empty_sequence = execute(
        &SelectCandidates,
        Candidate::Sequence { items: vec![] },
        json!({"unscored":0}),
    )
    .unwrap();
    assert_eq!(
        emitted(&empty_sequence),
        &Candidate::Sequence { items: vec![] }
    );
    assert_eq!(
        execute(&Stringify, Candidate::Choice { options: vec![] }, json!({}))
            .unwrap_err()
            .code,
        "unresolved-choice"
    );
    let invalid = Candidate::Choice {
        options: vec![ChoiceOption {
            value: text("a"),
            score: Some(f64::NAN),
        }],
    };
    assert_eq!(
        execute(&SelectCandidates, invalid, json!({"unscored":0}))
            .unwrap_err()
            .code,
        "invalid-packet"
    );
    assert_eq!(
        execute(
            &RomajiToHiragana,
            Candidate::Sequence { items: vec![] },
            json!({})
        )
        .unwrap_err()
        .code,
        "invalid-root"
    );
}

#[test]
fn failed_conversion_blocks_downstream_and_invalid_input_settings_are_rejected() {
    let mut graph = sample();
    graph.nodes[0].config = Some(json!({"mode":"fixed","text":"honjituq"}));
    let report = engine(&graph).run().unwrap();
    assert!(!report.succeeded());
    assert_eq!(report.nodes["hiragana"].status, NodeStatus::Failed);
    assert_eq!(
        report.nodes["hiragana"]
            .diagnostic
            .as_ref()
            .unwrap()
            .error
            .code,
        "unsupported-romaji"
    );
    assert_eq!(report.nodes["output"].status, NodeStatus::Blocked);
    assert!(report.packet("output", "out").is_none());
    graph.nodes[0].config = Some(json!({"mode":"keyboard","text":ROMAJI}));
    assert!(ExecutionPlan::compile(&graph, &registry()).is_err());
}

#[test]
fn romaji_variants_symbols_and_unknown_unicode_survive_each_stage() {
    for (input, expected) in [
        ("HONJITSU30%", "ほんじつ30%"),
        ("konnichiha", "こんにちは"),
        ("kin'en", "きんえん"),
        ("gakkou", "がっこう"),
        ("𠮷30.5% desu", "𠮷30.5% です"),
    ] {
        assert_eq!(
            content(emitted(
                &execute(&RomajiToHiragana, text(input), json!({})).unwrap()
            )),
            expected
        );
    }
    let segmented = execute(&Segment, text("𠮷30.5%です"), json!({})).unwrap();
    let mapped = execute(&KanaKanjiCandidates, emitted(&segmented).clone(), json!({})).unwrap();
    let selected = execute(
        &SelectCandidates,
        emitted(&mapped).clone(),
        json!({"unscored":0}),
    )
    .unwrap();
    assert_eq!(
        content(emitted(
            &execute(&Stringify, emitted(&selected).clone(), json!({})).unwrap()
        )),
        "𠮷30.5%です"
    );
}

#[test]
fn cli_logs_output_to_stderr_and_keeps_stdout_valid_json() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_diy-ime2"))
        .args(["run", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(include_bytes!("../examples/graphs/weather.json"))
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result["nodes"]["output"]["outputs"]["out"]["packet"]["root"]["text"],
        EXPECTED
    );
    assert!(String::from_utf8(output.stderr).unwrap().contains(EXPECTED));
}
