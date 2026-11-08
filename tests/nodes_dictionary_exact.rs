#[path = "support/mod.rs"]
mod test_host;
use std::sync::Arc;

use crate::test_host::{
    Candidate, ChoiceOption, Packet, TextMeta,
    node::{Node, OutputValue, RunContext},
    nodes::{
        candidate_selection::SelectCandidates,
        dictionary_exact::{
            ExactDictionary, MAX_CANDIDATES, MAX_CODE_POINTS, MAX_FILE_BYTES, MAX_LINE_BYTES,
            MAX_ROWS,
        },
        stringify::stringify,
    },
};
use serde_json::{Value, json};

const HEADER: &str = "reading\tsurface\tweight";
const SAMPLE: &[u8] = include_bytes!("../examples/proposals/dictionary-exact/entries.tsv");

fn text(value: &str) -> Packet {
    Packet::new(Candidate::Text {
        text: value.into(),
        meta: None,
    })
}
fn parse_rows(rows: &str) -> Result<ExactDictionary, crate::test_host::node::CoreError> {
    ExactDictionary::from_tsv_bytes(format!("{HEADER}\n{rows}").as_bytes())
}
fn options(packet: &Packet) -> &[ChoiceOption] {
    let Candidate::Choice { options } = &packet.root else {
        panic!("expected Choice");
    };
    options
}
fn option(value: &str, score: Option<f64>) -> ChoiceOption {
    ChoiceOption {
        value: text(value).root,
        score,
    }
}
fn format_code(input: &[u8]) -> String {
    ExactDictionary::from_tsv_bytes(input).unwrap_err().code
}

#[test]
fn all_thirteen_adopted_cases_match_the_dictionary() {
    let cases: Value =
        serde_json::from_str(include_str!("contracts/dictionary_exact.json")).unwrap();
    let dictionary = ExactDictionary::from_tsv_bytes(SAMPLE).unwrap();
    let cases = cases["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 13);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let input = if case["input"]["status"] == "no-output" {
            OutputValue::NoOutput
        } else {
            OutputValue::Data(Arc::new(
                serde_json::from_value(case["input"]["packet"].clone()).unwrap(),
            ))
        };
        let before = input.clone();
        match (
            case["expected"]["status"].as_str().unwrap(),
            dictionary.lookup_output(&input),
        ) {
            ("data", Ok(OutputValue::Data(packet))) => {
                let expected: Packet =
                    serde_json::from_value(case["expected"]["packet"].clone()).unwrap();
                assert_eq!(packet.as_ref(), &expected, "{id}");
            }
            ("no-output", Ok(OutputValue::NoOutput)) => {}
            ("error", Err(error)) => assert_eq!(error.code, case["expected"]["code"], "{id}"),
            (status, actual) => panic!("{id}: expected {status}, got {actual:?}"),
        }
        assert_eq!(input, before, "{id}: input mutation");
    }
}

#[test]
fn utf8_bom_lf_crlf_mixed_endings_and_optional_final_newline_are_supported() {
    let expected = parse_rows("a\t𠮷\t1\nb\t表記\t2").unwrap();
    for bom in ["", "\u{feff}"] {
        for ending in ["\n", "\r\n"] {
            for final_newline in ["", ending] {
                let input =
                    format!("{bom}{HEADER}{ending}a\t𠮷\t1{ending}b\t表記\t2{final_newline}");
                assert_eq!(
                    ExactDictionary::from_tsv_bytes(input.as_bytes()).unwrap(),
                    expected
                );
            }
        }
    }
    assert_eq!(parse_rows("a\t𠮷\t1\r\nb\t表記\t2").unwrap(), expected);
}

#[test]
fn invalid_utf8_is_rejected_without_replacement_and_reports_byte_offset() {
    for bytes in [
        vec![0xff],
        vec![0xc3, 0x28],
        vec![0xed, 0xa0, 0x80],
        vec![0xff, 0xfe, 0],
    ] {
        assert_eq!(format_code(&bytes), "dictionary-encoding");
    }
    let mut bytes = format!("{HEADER}\na\t").into_bytes();
    let position = bytes.len();
    bytes.extend([0xff, b'\t', b'1']);
    let error = ExactDictionary::from_tsv_bytes(&bytes).unwrap_err();
    assert_eq!(error.code, "dictionary-encoding");
    assert!(error.message.contains(&format!("byte {position}")));
    assert_eq!(
        format_code(format!("\u{feff}\u{feff}{HEADER}").as_bytes()),
        "dictionary-format"
    );
}

#[test]
fn header_column_empty_line_and_bare_cr_errors_are_not_silently_skipped() {
    for invalid in [
        "".to_string(),
        "reading\tweight\tsurface".into(),
        format!("{HEADER}\textra"),
        format!("{HEADER}\r"),
        format!("{HEADER}\na\tb\t1\r"),
        format!("{HEADER}\na\rb\tc\t1"),
        format!("{HEADER}\n\n"),
        format!("{HEADER}\na\tb\t1\n\nc\td\t2"),
        format!("{HEADER}\na\tb"),
        format!("{HEADER}\na\tb\t1\textra"),
        format!("{HEADER}\n#comment"),
    ] {
        assert_eq!(
            format_code(invalid.as_bytes()),
            "dictionary-format",
            "{invalid:?}"
        );
    }
    for valid in [
        HEADER.to_string(),
        format!("{HEADER}\n"),
        format!("{HEADER}\r\n"),
    ] {
        let dictionary = ExactDictionary::from_tsv_bytes(valid.as_bytes()).unwrap();
        assert_eq!(
            options(&dictionary.lookup_packet(&text("")).unwrap()),
            [option("", None)]
        );
    }
}

#[test]
fn empty_fields_and_nul_are_rejected_but_spaces_and_quotes_remain_literal() {
    for row in ["\tx\t1", "a\t\t1", "a\tx\t", "a\0\tx\t1", "a\tx\0\t1"] {
        assert_eq!(
            parse_rows(row).unwrap_err().code,
            "dictionary-format",
            "{row:?}"
        );
    }
    let dictionary = parse_rows(" \t \t1\n\"a\"\t\"表記\"\t2\n#\t記号\t3").unwrap();
    for (input, expected) in [(" ", " "), ("\"a\"", "\"表記\""), ("#", "記号")] {
        assert_eq!(
            options(&dictionary.lookup_packet(&text(input)).unwrap())[0].value,
            text(expected).root
        );
    }
    assert_eq!(
        options(&dictionary.lookup_packet(&text("a")).unwrap()),
        [option("a", None)]
    );
}

#[test]
fn weights_use_json_number_grammar_and_preserve_finite_signed_values() {
    for (raw, expected) in [
        ("0", 0.0),
        ("-0", -0.0),
        ("-1.25", -1.25),
        ("1e2", 100.0),
        ("-3E-2", -0.03),
        ("1e+3", 1000.0),
        ("1e308", 1e308),
        ("1e-999", 0.0),
    ] {
        let dictionary = parse_rows(&format!("a\tx\t{raw}")).unwrap();
        let score = options(&dictionary.lookup_packet(&text("a")).unwrap())[0]
            .score
            .unwrap();
        assert_eq!(score, expected, "{raw}");
        assert_eq!(
            score.is_sign_negative(),
            expected.is_sign_negative(),
            "{raw}"
        );
    }
    for invalid in [
        "", "+1", "01", "-01", ".5", "1.", "1e", "1e+", "1e-", "--1", "NaN", "inf", "Infinity",
        "1e309", "-1e309", " 1", "1 ", "1,000", "１", "0x10", "1_000", "1\0",
    ] {
        assert_eq!(
            parse_rows(&format!("a\tx\t{invalid}")).unwrap_err().code,
            "dictionary-format",
            "{invalid:?}"
        );
    }
}

#[test]
fn duplicate_pairs_are_rejected_even_if_weights_differ_with_original_line_location() {
    for score in ["1", "2"] {
        let error = parse_rows(&format!("a\tx\t1\nb\tx\t3\na\tx\t{score}")).unwrap_err();
        assert_eq!(error.code, "dictionary-format");
        assert!(error.message.contains("line 4"));
        assert!(error.message.contains("first occurrence at line 2"));
    }
    let dictionary = parse_rows("a\tx\t1\na\ty\t1\nb\tx\t1").unwrap();
    assert_eq!(
        options(&dictionary.lookup_packet(&text("a")).unwrap()).len(),
        2
    );
}

#[test]
fn lookup_preserves_literal_unicode_case_whitespace_and_unknown_control_text() {
    let dictionary =
        parse_rows("が\t蛾\t1\nA\t大文字\t2\na\t小文字\t3\n𠮷\tよし\t4\n30%\t三十パーセント\t5")
            .unwrap();
    for (input, output) in [
        ("が", "蛾"),
        ("A", "大文字"),
        ("a", "小文字"),
        ("𠮷", "よし"),
        ("30%", "三十パーセント"),
    ] {
        assert_eq!(
            options(&dictionary.lookup_packet(&text(input)).unwrap())[0].value,
            text(output).root
        );
    }
    for input in ["か\u{3099}", " が", "が ", "３０％", "がです", "A\nB\t\0"] {
        assert_eq!(
            options(&dictionary.lookup_packet(&text(input)).unwrap()),
            [option(input, None)]
        );
    }
}

#[test]
fn dictionary_choice_order_connects_to_existing_selection_and_stringification() {
    let dictionary = ExactDictionary::from_tsv_bytes(SAMPLE).unwrap();
    for (input, expected) in [
        ("こうすい", "降水"),
        ("かくりつ", "確率"),
        ("未登録", "未登録"),
        ("", ""),
    ] {
        let lookup = dictionary
            .lookup_output(&OutputValue::Data(Arc::new(text(input))))
            .unwrap();
        let context = RunContext {
            run_id: 1,
            plan_version: 1,
            node_id: "select",
            input: None,
        };
        let outputs = SelectCandidates
            .execute(
                context,
                &[("in".into(), lookup)].into(),
                &json!({"unscored":0}),
            )
            .unwrap();
        let OutputValue::Data(packet) = &outputs["out"] else {
            panic!("expected selected Data");
        };
        assert_eq!(stringify(packet).unwrap(), expected);
    }
}

#[test]
fn invalid_packets_are_rejected_before_roots_and_text_length() {
    let dictionary = parse_rows("").unwrap();
    let bad_version = Packet {
        version: 99,
        root: text("a").root,
    };
    let bad_meta = Packet::new(Candidate::Text {
        text: "a".into(),
        meta: Some(TextMeta {
            reading: None,
            source_span: Some([2, 1]),
        }),
    });
    let bad_score = Packet::new(Candidate::Choice {
        options: vec![option("a", Some(f64::NAN))],
    });
    for packet in [bad_version, bad_meta, bad_score] {
        assert_eq!(
            dictionary.lookup_packet(&packet).unwrap_err().code,
            "invalid-packet"
        );
    }
    for root in [
        Candidate::Sequence { items: vec![] },
        Candidate::Choice { options: vec![] },
    ] {
        assert_eq!(
            dictionary
                .lookup_packet(&Packet::new(root))
                .unwrap_err()
                .code,
            "invalid-root"
        );
    }
    assert_eq!(
        dictionary.lookup_output(&OutputValue::NoOutput).unwrap(),
        OutputValue::NoOutput
    );
}

#[test]
fn reading_surface_and_input_limits_count_unicode_code_points() {
    let at_limit = "𠮷".repeat(MAX_CODE_POINTS);
    let over = format!("{at_limit}𠮷");
    let dictionary = parse_rows(&format!("{at_limit}\tx\t1\nb\t{at_limit}\t2")).unwrap();
    assert_eq!(
        options(&dictionary.lookup_packet(&text(&at_limit)).unwrap()),
        [option("x", Some(1.0))]
    );
    assert_eq!(
        options(&dictionary.lookup_packet(&text("b")).unwrap()),
        [option(&at_limit, Some(2.0))]
    );
    for rows in [format!("{over}\tx\t1"), format!("a\t{over}\t1")] {
        assert_eq!(parse_rows(&rows).unwrap_err().code, "dictionary-limit");
    }
    assert_eq!(
        dictionary.lookup_packet(&text(&over)).unwrap_err().code,
        "dictionary-input-limit"
    );
    let unknown = parse_rows("").unwrap();
    assert_eq!(
        options(&unknown.lookup_packet(&text(&at_limit)).unwrap()),
        [option(&at_limit, None)]
    );
}

#[test]
fn candidate_limit_accepts_exactly_sixty_four_without_truncating() {
    let rows = (0..MAX_CANDIDATES)
        .map(|i| format!("a\t表記{i}\t{i}\n"))
        .collect::<String>();
    let dictionary = parse_rows(&rows).unwrap();
    let result = dictionary.lookup_packet(&text("a")).unwrap();
    assert_eq!(options(&result).len(), MAX_CANDIDATES);
    assert_eq!(
        options(&result)[MAX_CANDIDATES - 1],
        option("表記63", Some(63.0))
    );
    assert_eq!(
        parse_rows(&format!("{rows}a\t超過\t1")).unwrap_err().code,
        "dictionary-limit"
    );
    assert_eq!(
        parse_rows(&format!("{rows}a\t表記0\t9")).unwrap_err().code,
        "dictionary-format"
    );
}

#[test]
fn row_limit_is_checked_independently_from_candidate_count() {
    let rows = (0..MAX_ROWS)
        .map(|i| format!("key{i}\tx\t1\n"))
        .collect::<String>();
    let dictionary = parse_rows(&rows).unwrap();
    assert_eq!(
        options(&dictionary.lookup_packet(&text("key9999")).unwrap()),
        [option("x", Some(1.0))]
    );
    assert_eq!(
        parse_rows(&format!("{rows}extra\tx\t1")).unwrap_err().code,
        "dictionary-limit"
    );
}

fn long_numeric_line(key: &str, length: usize) -> String {
    let prefix = format!("{key}\tx\t1e+");
    assert!(prefix.len() < length);
    format!("{prefix}{}", "0".repeat(length - prefix.len()))
}

#[test]
fn line_limit_measures_bytes_excluding_lf_or_crlf() {
    for ending in ["", "\n", "\r\n"] {
        let row = long_numeric_line("a", MAX_LINE_BYTES);
        assert!(parse_rows(&format!("{row}{ending}")).is_ok());
        let over = long_numeric_line("a", MAX_LINE_BYTES + 1);
        assert_eq!(
            parse_rows(&format!("{over}{ending}")).unwrap_err().code,
            "dictionary-limit"
        );
    }
}

#[test]
fn file_byte_limit_includes_header_bom_and_line_endings() {
    let mut content = format!("\u{feff}{HEADER}\n");
    let mut row = 0;
    while content.len() < MAX_FILE_BYTES {
        let remaining = MAX_FILE_BYTES - content.len();
        let length = (remaining - 1).min(MAX_LINE_BYTES);
        content.push_str(&long_numeric_line(&format!("k{row}"), length));
        content.push('\n');
        row += 1;
    }
    assert_eq!(content.len(), MAX_FILE_BYTES);
    assert!(ExactDictionary::from_tsv_bytes(content.as_bytes()).is_ok());
    content.push('\n');
    assert_eq!(format_code(content.as_bytes()), "dictionary-limit");
    assert_eq!(
        format_code(&vec![0xff; MAX_FILE_BYTES + 1]),
        "dictionary-limit"
    );
}

#[test]
fn parsed_dictionary_owns_bytes_and_searches_do_not_mutate_inputs_or_previous_outputs() {
    let mut bytes = format!("{HEADER}\na\t表記\t1").into_bytes();
    let dictionary = ExactDictionary::from_tsv_bytes(&bytes).unwrap();
    bytes.fill(b'!');
    let input = Packet::new(Candidate::Text {
        text: "a".into(),
        meta: Some(TextMeta {
            reading: Some("エー".into()),
            source_span: Some([3, 4]),
        }),
    });
    let before = input.clone();
    let expected = dictionary.lookup_packet(&input).unwrap();
    let mut changed = expected.clone();
    let Candidate::Choice { options } = &mut changed.root else {
        unreachable!()
    };
    options[0].value = text("変更").root;
    for _ in 0..5 {
        assert_eq!(dictionary.lookup_packet(&input).unwrap(), expected);
    }
    assert_eq!(input, before);
    assert_ne!(changed, expected);
}

#[test]
fn separate_dictionaries_and_failed_loads_never_replace_a_previous_snapshot() {
    let first = parse_rows("a\t一\t1").unwrap();
    let second = parse_rows("a\t二\t2").unwrap();
    let before = first.clone();
    assert!(parse_rows("a\t途中\t1\na\t途中\t2").is_err());
    assert_eq!(first, before);
    for _ in 0..5 {
        assert_eq!(
            options(&first.lookup_packet(&text("a")).unwrap()),
            [option("一", Some(1.0))]
        );
        assert_eq!(
            options(&second.lookup_packet(&text("a")).unwrap()),
            [option("二", Some(2.0))]
        );
    }
}

#[test]
fn immutable_dictionary_can_be_queried_concurrently() {
    let dictionary = Arc::new(ExactDictionary::from_tsv_bytes(SAMPLE).unwrap());
    let expected = dictionary.lookup_packet(&text("こうすい")).unwrap();
    let threads = (0..8)
        .map(|_| {
            let dictionary = Arc::clone(&dictionary);
            std::thread::spawn(move || dictionary.lookup_packet(&text("こうすい")).unwrap())
        })
        .collect::<Vec<_>>();
    for thread in threads {
        assert_eq!(thread.join().unwrap(), expected);
    }
}

struct MemoryResources(std::collections::BTreeMap<String, Arc<[u8]>>);
impl crate::test_host::node::PrepareResources for MemoryResources {
    fn read(
        &mut self,
        reference: &str,
        maximum: usize,
    ) -> Result<Arc<[u8]>, crate::test_host::node::CoreError> {
        assert_eq!(maximum, MAX_FILE_BYTES);
        self.0.get(reference).cloned().ok_or_else(|| {
            crate::test_host::node::CoreError::new("resource-read", "test resource missing")
        })
    }
}
fn resources(bytes: &[u8]) -> MemoryResources {
    MemoryResources([("dictionary.tsv".into(), Arc::from(bytes))].into())
}
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "dictionary",
        input: None,
    }
}
#[test]
fn prepared_node_matches_all_thirteen_adopted_cases_and_does_not_change_input() {
    use crate::test_host::nodes::dictionary_exact::DictionaryExact;
    let node = DictionaryExact;
    let mut resources = resources(SAMPLE);
    let prepared = node
        .prepare(&json!({"path":"dictionary.tsv"}), &mut resources)
        .unwrap()
        .unwrap();
    let fixture: Value =
        serde_json::from_str(include_str!("contracts/dictionary_exact.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = if case["input"]["status"] == "no-output" {
            OutputValue::NoOutput
        } else {
            OutputValue::Data(Arc::new(
                serde_json::from_value(case["input"]["packet"].clone()).unwrap(),
            ))
        };
        let inputs: crate::test_host::node::Inputs = [("in".into(), input)].into();
        let before: crate::test_host::node::Inputs = inputs.clone();
        let actual = prepared.execute(context(), &inputs);
        match case["expected"]["status"].as_str().unwrap() {
            "error" => assert_eq!(actual.unwrap_err().code, case["expected"]["code"]),
            "no-output" => assert_eq!(actual.unwrap()["out"], OutputValue::NoOutput),
            "data" => assert_eq!(
                actual.unwrap()["out"],
                OutputValue::Data(Arc::new(
                    serde_json::from_value(case["expected"]["packet"].clone()).unwrap()
                ))
            ),
            other => panic!("unexpected fixture status {other}"),
        }
        assert_eq!(inputs, before);
    }
    assert_eq!(
        prepared
            .execute(
                context(),
                &[(
                    "in".into(),
                    OutputValue::Data(Arc::new(text(&"a".repeat(4097))))
                )]
                .into()
            )
            .unwrap_err()
            .code,
        "dictionary-input-limit"
    );
    assert_eq!(
        prepared
            .execute(context(), &Default::default())
            .unwrap_err()
            .code,
        "missing-input"
    );
}
#[test]
fn node_definition_strict_settings_and_direct_unprepared_error_are_explicit() {
    use crate::test_host::node::RootConstraint;
    use crate::test_host::nodes::dictionary_exact::{DictionaryExact, KIND};
    let node = DictionaryExact;
    let d = node.definition();
    assert_eq!(d.kind, KIND);
    assert_eq!(d.version, 1);
    assert_eq!(d.inputs[0].port.root, RootConstraint::Text);
    assert_eq!(d.inputs[0].port.language.as_deref(), Some("ja"));
    assert_eq!(d.outputs[0].root, RootConstraint::Choice);
    assert_eq!(d.default_config, json!({"path":"dictionary.tsv"}));
    for config in [
        json!(null),
        json!([]),
        json!({}),
        json!({"path":null}),
        json!({"path":3}),
        json!({"path":""}),
        json!({"path":"a","unknown":true}),
        json!({"path":"a\0b"}),
        json!({"path":"a".repeat(4097)}),
    ] {
        assert_eq!(
            node.validate_config(&config).unwrap_err().code,
            "invalid-config",
            "{config}"
        );
    }
    assert_eq!(
        node.execute(context(), &Default::default(), &d.default_config)
            .unwrap_err()
            .code,
        "dictionary-not-prepared"
    );
}
#[test]
fn node_maps_host_failures_and_rejects_bad_or_oversized_provider_bytes() {
    use crate::test_host::node::{CoreError, PrepareResources};
    use crate::test_host::nodes::dictionary_exact::DictionaryExact;
    struct Failure(&'static str);
    impl PrepareResources for Failure {
        fn read(&mut self, _: &str, _: usize) -> Result<Arc<[u8]>, CoreError> {
            Err(CoreError::new(self.0, "test failure"))
        }
    }
    let node = DictionaryExact;
    let config = node.definition().default_config;
    for (source, expected) in [
        ("resource-unavailable", "dictionary-path"),
        ("resource-path", "dictionary-path"),
        ("resource-read", "dictionary-read"),
        ("resource-busy", "dictionary-read"),
        ("resource-deadline", "dictionary-read"),
        ("resource-platform", "dictionary-read"),
        ("resource-limit", "dictionary-limit"),
    ] {
        assert_eq!(
            node.prepare(&config, &mut Failure(source))
                .err()
                .unwrap()
                .code,
            expected
        );
    }
    for (bytes, code) in [
        (vec![0xff], "dictionary-encoding"),
        (b"broken".to_vec(), "dictionary-format"),
        (vec![b'x'; MAX_FILE_BYTES + 1], "dictionary-limit"),
    ] {
        assert_eq!(
            node.prepare(&config, &mut resources(&bytes))
                .err()
                .unwrap()
                .code,
            code
        );
    }
}
#[test]
fn same_registry_dictionary_instances_keep_separate_snapshots_across_prepare_and_rollback() {
    use crate::test_host::{
        builtins::register_builtin_nodes,
        graph::{ExecutionPlan, Graph},
        node::{InputMode, Registry, RuntimeInput},
        runtime::Engine,
    };
    let mut registry = Registry::new();
    register_builtin_nodes(&mut registry).unwrap();
    let graph=Graph::from_json(&json!({"version":1,"nodes":[{"id":"input","kind":"input.text","version":1},{"id":"kana","kind":"text.romaji-to-hiragana","version":1},{"id":"a","kind":"text.dictionary-exact","version":1,"config":{"path":"A"}},{"id":"b","kind":"text.dictionary-exact","version":1,"config":{"path":"B"}}],"edges":[{"from":{"node":"input","port":"out"},"to":{"node":"kana","port":"in"}},{"from":{"node":"kana","port":"out"},"to":{"node":"a","port":"in"}},{"from":{"node":"kana","port":"out"},"to":{"node":"b","port":"in"}}]}).to_string()).unwrap();
    let mut memory = MemoryResources(
        [
            (
                "A".into(),
                Arc::from(b"reading\tsurface\tweight\n\xe3\x81\x8b\tA\t1\n".as_slice()),
            ),
            (
                "B".into(),
                Arc::from(b"reading\tsurface\tweight\n\xe3\x81\x8b\tB\t1\n".as_slice()),
            ),
        ]
        .into(),
    );
    let mut engine =
        Engine::new(ExecutionPlan::compile_with_resources(&graph, &registry, &mut memory).unwrap());
    let input = RuntimeInput {
        revision: 1,
        text: "ka".into(),
        mode: InputMode::Compose,
    };
    let first = engine.run_with_input(&input).unwrap();
    assert_eq!(
        options(first.packet("a", "out").unwrap()),
        [option("A", Some(1.0))]
    );
    assert_eq!(
        options(first.packet("b", "out").unwrap()),
        [option("B", Some(1.0))]
    );
    memory.0.insert(
        "A".into(),
        Arc::from(b"reading\tsurface\tweight\n\xe3\x81\x8b\tnew\t1\n".as_slice()),
    );
    engine.prepare_plan(
        ExecutionPlan::compile_with_resources(&graph, &registry, &mut memory).unwrap(),
    );
    assert_eq!(
        options(
            engine
                .run_with_input(&input)
                .unwrap()
                .packet("a", "out")
                .unwrap()
        ),
        [option("A", Some(1.0))]
    );
    engine.activate_pending().unwrap();
    assert_eq!(
        options(
            engine
                .run_with_input(&input)
                .unwrap()
                .packet("a", "out")
                .unwrap()
        ),
        [option("new", Some(1.0))]
    );
    engine.rollback().unwrap();
    assert_eq!(
        options(
            engine
                .run_with_input(&input)
                .unwrap()
                .packet("a", "out")
                .unwrap()
        ),
        [option("A", Some(1.0))]
    );
}
#[cfg(windows)]
#[test]
fn real_cli_runs_registered_dictionary_pipeline_with_explicit_root_and_rejects_unconfigured_root() {
    use crate::test_host::resources::RESOURCE_ROOT_ENV;
    use std::{path::Path, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/nodes/dictionary_exact");
    let graph = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/graphs/dictionary-fixed.json");
    let output = Command::new(env!("CARGO_BIN_EXE_diy-ime2"))
        .args(["run"])
        .arg(&graph)
        .env(RESOURCE_ROOT_ENV, &root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["nodes"]["result"]["outputs"]["out"]["packet"]["root"]["text"],
        "降水"
    );
    let denied = Command::new(env!("CARGO_BIN_EXE_diy-ime2"))
        .args(["run"])
        .arg(&graph)
        .env_remove(RESOURCE_ROOT_ENV)
        .output()
        .unwrap();
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("dictionary-path"));
}
