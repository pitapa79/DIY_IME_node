//! `text.select-candidates@1` の検査。契約: docs/node-contracts-v0.1.md
//! 受け入れ例（tests/contracts/candidate_selection.json）の全ケースを実装へ照合し、
//! 例にない境界・不変性・反復・インスタンス分離を追加で検査する。
//! 統合後の標準Registryと、テスト用ソースを登録したRegistryを使う。
#[path = "support/mod.rs"]
mod test_host;
use std::sync::Arc;

use crate::test_host::{
    Candidate, ChoiceOption, Packet, TextMeta,
    builtins::{initial_e2e::EXPECTED, register_builtin_nodes},
    graph::{ExecutionPlan, Graph},
    node::{
        CoreError, DataType, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext,
    },
    nodes::candidate_selection::{KIND, SelectCandidates},
    runtime::{Engine, NodeStatus, ResolvedValue},
};
use serde_json::{Value, json};

const CONTRACT: &str = include_str!("contracts/candidate_selection.json");
const EXAMPLE: &str = include_str!("../examples/nodes/candidate_selection/select-by-score.json");

#[test]
fn standard_pipeline_connects_runtime_input_selection_and_stringification() {
    use crate::test_host::node::{InputMode, RuntimeInput};
    let graph =
        Graph::from_json(include_str!("../examples/graphs/practical-pipeline.json")).unwrap();
    let mut standard = Registry::new();
    register_builtin_nodes(&mut standard).unwrap();
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &standard).unwrap());
    for (index, (text, expected)) in [
        ("honjitunokousuikakurituha30%desu", EXPECTED),
        ("", ""),
        ("𠮷30%", "𠮷30%"),
        ("honjitunokousuikakurituha30%desu", EXPECTED),
    ]
    .into_iter()
    .enumerate()
    {
        let input = RuntimeInput {
            revision: index as u64 + 1,
            text: text.into(),
            mode: InputMode::Compose,
        };
        let report = engine.run_with_input(&input).unwrap();
        assert!(report.succeeded(), "{text:?}");
        assert_eq!(report.plan_version, 1);
        assert_eq!(report.input_revision, Some(input.revision));
        assert!(!contains_choice(
            &report.packet("select", "out").unwrap().root
        ));
        assert_eq!(
            report.packet("result", "out").unwrap().root,
            Candidate::Text {
                text: expected.into(),
                meta: None
            }
        );
    }
}

// ---- 補助 ----------------------------------------------------------------

/// テスト専用の言語 `ja` のソース。入力ポートの言語が `ja` のノードへ接続するために使う。
/// `emit: false` なら明示的なNoOutputを出す。
struct JaSource;
impl Node for JaSource {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            "test.ja-source",
            1,
            vec![],
            vec![Port::packet("out").with_language("ja")],
        );
        definition.default_config =
            json!({"packet": {"version": 1, "root": {"type": "text", "text": ""}}});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        let object = config
            .as_object()
            .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
        let packet = object
            .get("packet")
            .ok_or_else(|| CoreError::new("invalid-config", "packet is required"))?;
        Packet::from_json(&packet.to_string())
            .map_err(|error| CoreError::new("invalid-config", error.to_string()))?;
        Ok(())
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, config: &Value) -> Result<Outputs, CoreError> {
        let value = if config.get("emit") == Some(&json!(false)) {
            OutputValue::NoOutput
        } else {
            OutputValue::Data(Arc::new(
                Packet::from_json(&config["packet"].to_string())
                    .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?,
            ))
        };
        Ok([("out".to_owned(), value)].into())
    }
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    register_builtin_nodes(&mut registry).unwrap();
    registry.register(JaSource).unwrap();
    registry
}

fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "select",
        input: None,
    }
}

fn text(value: &str) -> Candidate {
    Candidate::Text {
        text: value.into(),
        meta: None,
    }
}
fn seq(items: Vec<Candidate>) -> Candidate {
    Candidate::Sequence { items }
}
fn choice(options: Vec<(Option<f64>, Candidate)>) -> Candidate {
    Candidate::Choice {
        options: options
            .into_iter()
            .map(|(score, value)| ChoiceOption { value, score })
            .collect(),
    }
}
fn scored(score: f64, value: Candidate) -> (Option<f64>, Candidate) {
    (Some(score), value)
}
fn unscored(value: Candidate) -> (Option<f64>, Candidate) {
    (None, value)
}
fn config(unscored: f64) -> Value {
    json!({ "unscored": unscored })
}

fn run(root: Candidate, config: &Value) -> Result<OutputValue, CoreError> {
    run_input(OutputValue::Data(Arc::new(Packet::new(root))), config)
}
fn run_input(input: OutputValue, config: &Value) -> Result<OutputValue, CoreError> {
    let inputs: Inputs = [("in".to_owned(), input)].into();
    let mut outputs: Outputs = SelectCandidates.execute(context(), &inputs, config)?;
    assert_eq!(outputs.len(), 1, "only the declared `out` is returned");
    Ok(outputs.remove("out").expect("out"))
}
/// 成功してDataが返ることを前提に、出力の根を取り出す。
fn selected(root: Candidate, config: &Value) -> Candidate {
    match run(root, config).unwrap() {
        OutputValue::Data(packet) => {
            packet.validate().unwrap();
            assert_eq!(packet.version, 1);
            packet.root.clone()
        }
        OutputValue::NoOutput => panic!("expected Data, got NoOutput"),
    }
}
fn is_no_output(root: Candidate, config: &Value) -> bool {
    matches!(run(root, config).unwrap(), OutputValue::NoOutput)
}
fn error_code(result: Result<OutputValue, CoreError>) -> String {
    result.expect_err("expected an error").code
}
fn packet_of(value: &Value) -> Packet {
    Packet::from_json(&value.to_string()).unwrap()
}
fn contains_choice(candidate: &Candidate) -> bool {
    match candidate {
        Candidate::Choice { .. } => true,
        Candidate::Sequence { items } => items.iter().any(contains_choice),
        Candidate::Text { .. } => false,
    }
}

// ---- 契約の受け入れ例 --------------------------------------------------------

#[test]
fn contract_metadata_matches_the_implementation() {
    let contract: Value = serde_json::from_str(CONTRACT).unwrap();
    assert_eq!(contract["nodeKind"], KIND);
    let definition = SelectCandidates.definition();
    assert_eq!(definition.kind, KIND);
    // JSONの数値表記（0と0.0）の差ではなく、値として比較する。
    assert_eq!(
        definition.default_config["unscored"].as_f64(),
        contract["defaultConfig"]["unscored"].as_f64()
    );
    SelectCandidates
        .validate_config(&definition.default_config)
        .unwrap();
}

#[test]
fn every_contract_case_is_satisfied_by_direct_execution() {
    let contract: Value = serde_json::from_str(CONTRACT).unwrap();
    let default = contract["defaultConfig"].clone();
    let cases = contract["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let config = case.get("config").unwrap_or(&default);
        let expected = &case["expected"];
        let result = SelectCandidates.validate_config(config).and_then(|()| {
            let input = if case["inputStatus"] == "no-output" {
                OutputValue::NoOutput
            } else {
                OutputValue::Data(Arc::new(
                    serde_json::from_value::<Packet>(case["packet"].clone()).unwrap(),
                ))
            };
            run_input(input, config)
        });
        match expected["status"].as_str().unwrap() {
            "data" => match result.unwrap_or_else(|error| panic!("{id}: {error}")) {
                OutputValue::Data(packet) => {
                    assert_eq!(*packet, packet_of(&expected["packet"]), "{id}")
                }
                OutputValue::NoOutput => panic!("{id}: expected Data, got NoOutput"),
            },
            "no-output" => {
                assert_eq!(result.unwrap(), OutputValue::NoOutput, "{id}");
            }
            "error" => assert_eq!(
                result.expect_err(id).code,
                expected["code"].as_str().unwrap(),
                "{id}"
            ),
            other => panic!("{id}: unsupported expected status {other}"),
        }
    }
}

#[test]
fn every_contract_case_is_satisfied_through_the_engine() {
    let contract: Value = serde_json::from_str(CONTRACT).unwrap();
    for case in contract["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let expected = &case["expected"];
        let node_config = case.get("config").cloned();
        // 不正Packetは上流のソース自体が出力検査で拒否されるため、Engine経由では扱わない（直接実行で検査）。
        if expected["code"] == "invalid-packet" {
            continue;
        }
        let mut nodes = vec![json!({"id": "select", "kind": KIND, "version": 1})];
        // 上流のNoOutputは、test.ja-sourceのemit=falseで作る。
        let source_config = if case["inputStatus"] == "no-output" {
            json!({"packet": {"version": 1, "root": {"type": "text", "text": ""}}, "emit": false})
        } else {
            json!({"packet": case["packet"]})
        };
        nodes.push(
            json!({"id": "source", "kind": "test.ja-source", "version": 1,
            "config": source_config}),
        );
        let edges = [("source", "out", "select", "in")];
        if let Some(config) = node_config {
            nodes[0]["config"] = config;
        }
        let graph = json!({
            "version": 1,
            "nodes": nodes,
            "edges": edges.iter().map(|(a, b, c, d)| json!({
                "from": {"node": a, "port": b}, "to": {"node": c, "port": d}})).collect::<Vec<_>>(),
        });
        let graph = Graph::from_json(&graph.to_string()).unwrap();
        let plan = ExecutionPlan::compile(&graph, &registry());
        if expected["status"] == "error" {
            // 不正な設定は計画準備時に拒否され、実行されない。
            assert_eq!(plan.err().expect(id).code, expected["code"], "{id}");
            continue;
        }
        let report = Engine::new(plan.unwrap_or_else(|error| panic!("{id}: {error}")))
            .run()
            .unwrap();
        assert!(report.succeeded(), "{id}");
        match expected["status"].as_str().unwrap() {
            "data" => {
                assert_eq!(report.nodes["select"].status, NodeStatus::Executed, "{id}");
                assert_eq!(
                    **report.packet("select", "out").expect(id),
                    packet_of(&expected["packet"]),
                    "{id}"
                );
            }
            "no-output" => {
                // 上流のNoOutputでは、必須入力が欠けるためノード自体がSkippedになる。
                let expected_status = if case["inputStatus"] == "no-output" {
                    NodeStatus::Skipped
                } else {
                    NodeStatus::Executed
                };
                assert_eq!(report.nodes["select"].status, expected_status, "{id}");
                assert!(
                    matches!(
                        report.nodes["select"].outputs["out"],
                        ResolvedValue::NoOutput
                    ),
                    "{id}"
                );
            }
            other => panic!("{id}: unsupported expected status {other}"),
        }
    }
}

// ---- 定義・設定 --------------------------------------------------------------

#[test]
fn definition_follows_the_contract() {
    let definition = SelectCandidates.definition();
    assert_eq!((definition.kind.as_str(), definition.version), (KIND, 1));
    assert_eq!(definition.default_config, json!({"unscored": 0.0}));
    assert_eq!(definition.inputs.len(), 1);
    let input = &definition.inputs[0];
    assert!(input.required);
    assert_eq!(input.port.id, "in");
    assert_eq!(input.port.language.as_deref(), Some("ja"));
    assert_eq!(input.port.root, RootConstraint::Any);
    assert_eq!(
        input.port.data_type,
        DataType {
            id: "text-packet".into(),
            version: 1
        }
    );
    assert_eq!(definition.outputs.len(), 1);
    let output = &definition.outputs[0];
    assert_eq!(output.id, "out");
    assert_eq!(output.language.as_deref(), Some("ja"));
    assert_eq!(output.root, RootConstraint::Any);
}

#[test]
fn config_accepts_only_one_finite_number() {
    for valid in [
        json!({"unscored": 0}),
        json!({"unscored": 0.0}),
        json!({"unscored": -0.0}),
        json!({"unscored": 7}),
        json!({"unscored": -3.5}),
        json!({"unscored": 1.7976931348623157e308}),
        json!({"unscored": -1.7976931348623157e308}),
        json!({"unscored": 5e-324}),
    ] {
        SelectCandidates
            .validate_config(&valid)
            .unwrap_or_else(|error| panic!("{valid}: {error}"));
    }
    for invalid in [
        json!({}),
        json!({"unscored": "NaN"}),
        json!({"unscored": "1"}),
        json!({"unscored": null}),
        json!({"unscored": true}),
        json!({"unscored": [1]}),
        json!({"unscored": {}}),
        json!({"unscored": 1, "extra": 1}),
        json!({"Unscored": 1}),
        json!(null),
        json!(1),
        json!("unscored"),
        json!([{"unscored": 1}]),
    ] {
        assert_eq!(
            SelectCandidates
                .validate_config(&invalid)
                .expect_err(&invalid.to_string())
                .code,
            "invalid-config",
            "{invalid}"
        );
    }
}

#[test]
fn execute_rejects_an_invalid_config_without_returning_partial_output() {
    let input = OutputValue::Data(Arc::new(Packet::new(text("a"))));
    assert_eq!(
        error_code(run_input(input, &json!({"unscored": "NaN"}))),
        "invalid-config"
    );
}

#[test]
fn omitted_config_uses_the_default_and_a_given_config_is_not_merged() {
    // 設定省略はdefault_config（unscored=0）。
    let omitted = json!({
        "version": 1,
        "nodes": [
            {"id": "source", "kind": "test.ja-source", "version": 1, "config": {"packet": {"version": 1, "root":
                {"type": "choice", "options": [{"value": {"type": "text", "text": "省略"}},
                                               {"score": -1, "value": {"type": "text", "text": "負"}}]}}}},
            {"id": "select", "kind": KIND, "version": 1}
        ],
        "edges": [{"from": {"node": "source", "port": "out"}, "to": {"node": "select", "port": "in"}}]
    });
    let graph = Graph::from_json(&omitted.to_string()).unwrap();
    let report = Engine::new(ExecutionPlan::compile(&graph, &registry()).unwrap())
        .run()
        .unwrap();
    assert_eq!(report.packet("select", "out").unwrap().root, text("省略"));

    // 部分的な設定（キー欠落）はdefaultとマージされず、計画準備時に拒否される。
    let mut partial = omitted;
    partial["nodes"][1]["config"] = json!({});
    let graph = Graph::from_json(&partial.to_string()).unwrap();
    assert_eq!(
        ExecutionPlan::compile(&graph, &registry())
            .err()
            .unwrap()
            .code,
        "invalid-config"
    );
}

#[test]
fn the_node_registers_alongside_existing_kinds_without_overwriting_them() {
    let registry = registry();
    let kinds: Vec<_> = registry
        .definitions()
        .map(|definition| definition.kind.as_str())
        .collect();
    assert!(kinds.contains(&KIND));
    assert!(kinds.contains(&"demo.select-text"));
    // 重複登録は拒否される（旧ノードを上書きしない）。
    let mut duplicate = Registry::new();
    duplicate.register(SelectCandidates).unwrap();
    assert_eq!(
        duplicate.register(SelectCandidates).unwrap_err().code,
        "duplicate-kind"
    );
}

// ---- スコアと選択 ------------------------------------------------------------

#[test]
fn picks_the_highest_score_within_one_choice() {
    let root = choice(vec![
        scored(1.0, text("低")),
        scored(5.0, text("高")),
        scored(3.0, text("中")),
    ]);
    assert_eq!(selected(root, &config(0.0)), text("高"));
}

#[test]
fn ties_keep_the_earliest_option() {
    let root = choice(vec![
        scored(2.0, text("一")),
        scored(2.0, text("二")),
        scored(2.0, text("三")),
    ]);
    assert_eq!(selected(root, &config(0.0)), text("一"));

    // 先頭が低く、後続の2つが同点なら、その2つのうち先の方。
    let root = choice(vec![
        scored(1.0, text("低")),
        scored(2.0, text("高A")),
        scored(2.0, text("高B")),
    ]);
    assert_eq!(selected(root, &config(0.0)), text("高A"));
}

#[test]
fn omitted_scores_are_evaluated_as_the_configured_value() {
    // 省略の評価値と明示スコアが同点なら、先頭が勝つ。
    let tie_after = choice(vec![unscored(text("省略")), scored(7.0, text("明示"))]);
    assert_eq!(selected(tie_after.clone(), &config(7.0)), text("省略"));
    let tie_before = choice(vec![scored(7.0, text("明示")), unscored(text("省略"))]);
    assert_eq!(selected(tie_before, &config(7.0)), text("明示"));
    // unscoredが高ければ省略が勝ち、低ければ明示が勝つ。
    assert_eq!(selected(tie_after.clone(), &config(7.5)), text("省略"));
    assert_eq!(selected(tie_after, &config(6.5)), text("明示"));
    // 既定0.0では、負の明示スコアより省略が優先される。
    let root = choice(vec![scored(-1.0, text("負")), unscored(text("省略"))]);
    assert_eq!(selected(root, &config(0.0)), text("省略"));
}

#[test]
fn negative_scores_and_negative_unscored_are_valid() {
    let all_negative = choice(vec![
        scored(-5.0, text("a")),
        scored(-1.0, text("b")),
        scored(-3.0, text("c")),
    ]);
    assert_eq!(selected(all_negative, &config(0.0)), text("b"));
    // unscoredが負でも使える。-3は-5より大きく、-1より小さい。
    let root = choice(vec![
        scored(-5.0, text("明示低")),
        unscored(text("省略")),
        scored(-4.0, text("明示中")),
    ]);
    assert_eq!(selected(root.clone(), &config(-3.0)), text("省略"));
    assert_eq!(selected(root, &config(-10.0)), text("明示中"));
}

#[test]
fn zero_and_negative_zero_are_a_tie() {
    let root = choice(vec![scored(-0.0, text("先")), scored(0.0, text("後"))]);
    assert_eq!(selected(root, &config(0.0)), text("先"));
    let root = choice(vec![scored(0.0, text("先")), scored(-0.0, text("後"))]);
    assert_eq!(selected(root, &config(0.0)), text("先"));
}

#[test]
fn extreme_finite_scores_are_compared_correctly() {
    let root = choice(vec![
        scored(f64::MIN, text("最小")),
        scored(f64::MIN_POSITIVE, text("微小")),
        scored(f64::MAX, text("最大")),
        scored(f64::MAX, text("最大2")),
    ]);
    assert_eq!(selected(root, &config(0.0)), text("最大"));
    let root = choice(vec![scored(f64::MIN, text("最小")), unscored(text("省略"))]);
    assert_eq!(selected(root, &config(f64::MIN)), text("最小"));
}

#[test]
fn a_single_option_is_selected_regardless_of_its_score() {
    let root = choice(vec![scored(-1e300, text("唯一"))]);
    assert_eq!(selected(root, &config(0.0)), text("唯一"));
}

#[test]
fn scores_are_neither_summed_nor_compared_across_choices() {
    // 外側は外側のスコアだけで選ぶ。内側の巨大なスコアは影響しない。
    let root = choice(vec![
        scored(1.0, choice(vec![scored(9999.0, text("外低・内高"))])),
        scored(2.0, choice(vec![scored(-9999.0, text("外高・内低"))])),
    ]);
    assert_eq!(selected(root, &config(0.0)), text("外高・内低"));

    // 選んだ枝の内側のChoiceは、その内側のスコアだけで選ぶ。
    let root = choice(vec![scored(
        0.0,
        seq(vec![
            choice(vec![scored(-2.0, text("A1")), scored(-1.0, text("A2"))]),
            choice(vec![scored(100.0, text("B1")), scored(50.0, text("B2"))]),
        ]),
    )]);
    assert_eq!(
        selected(root, &config(0.0)),
        seq(vec![text("A2"), text("B1")])
    );
}

#[test]
fn scores_are_not_normalized() {
    // 合計が1でなくても、差が極端でも、単純な大小比較だけで決まる。
    let root = choice(vec![
        scored(0.1, text("a")),
        scored(0.1000000000000001, text("b")),
    ]);
    assert_eq!(selected(root, &config(0.0)), text("b"));
    let root = choice(vec![
        scored(1e300, text("巨大")),
        scored(1e-300, text("微小")),
    ]);
    assert_eq!(selected(root, &config(0.0)), text("巨大"));
}

// ---- 構造・メタデータ・Unicode -----------------------------------------------

#[test]
fn sequence_order_and_nesting_are_preserved() {
    let root = seq(vec![
        text("一"),
        seq(vec![text("二"), seq(vec![]), seq(vec![text("三")])]),
        choice(vec![scored(1.0, seq(vec![text("四"), text("五")]))]),
        text("六"),
    ]);
    let expected = seq(vec![
        text("一"),
        seq(vec![text("二"), seq(vec![]), seq(vec![text("三")])]),
        seq(vec![text("四"), text("五")]),
        text("六"),
    ]);
    let output = selected(root, &config(0.0));
    assert_eq!(output, expected);
    assert!(!contains_choice(&output));
}

#[test]
fn nested_choices_on_the_chosen_path_are_all_resolved() {
    let root = choice(vec![
        scored(
            2.0,
            choice(vec![
                scored(
                    1.0,
                    seq(vec![
                        choice(vec![scored(3.0, text("x")), scored(4.0, text("y"))]),
                        choice(vec![unscored(text("p")), scored(-1.0, text("q"))]),
                    ]),
                ),
                scored(0.0, text("捨")),
            ]),
        ),
        scored(1.0, text("外側で負け")),
    ]);
    let output = selected(root, &config(0.0));
    assert_eq!(output, seq(vec![text("y"), text("p")]));
    assert!(!contains_choice(&output));
}

#[test]
fn text_and_metadata_are_kept_exactly_and_not_joined() {
    let meta = TextMeta {
        reading: Some("ほんじつ".into()),
        source_span: Some([3, 7]),
    };
    let reading_only = TextMeta {
        reading: Some("よみ".into()),
        source_span: None,
    };
    let span_only = TextMeta {
        reading: None,
        source_span: Some([2, 2]),
    };
    let with_meta = |value: &str, meta: &TextMeta| Candidate::Text {
        text: value.into(),
        meta: Some(meta.clone()),
    };
    let root = seq(vec![
        choice(vec![
            scored(1.0, with_meta("本日", &meta)),
            scored(0.0, text("先")),
        ]),
        with_meta("読", &reading_only),
        with_meta("区間", &span_only),
        text("素"),
    ]);
    let expected = seq(vec![
        with_meta("本日", &meta),
        with_meta("読", &reading_only),
        with_meta("区間", &span_only),
        text("素"),
    ]);
    assert_eq!(selected(root, &config(0.0)), expected);
}

#[test]
fn unicode_text_is_preserved_byte_for_byte() {
    let samples = [
        "",
        "日本語のテキスト",
        "😀👨‍👩‍👧‍👦🇯🇵",
        "e\u{301}",         // 結合文字
        "\u{FF76}\u{FF9E}", // 半角カナ
        "a\0b",             // NULを含む
        "  先頭と末尾の空白\t\n",
        "\u{10FFFF}",
        "𠮷野家",
    ];
    for sample in samples {
        let root = choice(vec![scored(1.0, text(sample)), scored(0.0, text("捨"))]);
        assert_eq!(selected(root, &config(0.0)), text(sample));
        // 選択肢の中の列でも順序と内容が変わらない。
        let root = seq(vec![text(sample), text(sample)]);
        assert_eq!(
            selected(root, &config(0.0)),
            seq(vec![text(sample), text(sample)])
        );
    }
}

#[test]
fn unselected_branches_are_not_included_in_the_result() {
    let root = choice(vec![
        scored(1.0, seq(vec![text("捨1"), text("捨2")])),
        scored(2.0, text("採用")),
        scored(0.0, choice(vec![scored(1.0, text("捨3"))])),
    ]);
    let output = selected(root, &config(0.0));
    assert_eq!(output, text("採用"));
}

// ---- 空の扱い ------------------------------------------------------------------

#[test]
fn empty_text_and_empty_sequence_are_data_not_no_output() {
    assert_eq!(selected(text(""), &config(0.0)), text(""));
    assert_eq!(selected(seq(vec![]), &config(0.0)), seq(vec![]));
    assert_eq!(
        selected(seq(vec![text(""), seq(vec![]), text("")]), &config(0.0)),
        seq(vec![text(""), seq(vec![]), text("")])
    );
    // 選ばれた空のText・空のSequenceもDataのまま。
    let root = choice(vec![scored(1.0, text("")), scored(0.0, text("x"))]);
    assert_eq!(selected(root, &config(0.0)), text(""));
    let root = choice(vec![scored(1.0, seq(vec![])), scored(0.0, text("x"))]);
    assert_eq!(selected(root, &config(0.0)), seq(vec![]));
}

#[test]
fn an_empty_choice_on_the_selection_path_makes_the_whole_output_no_output() {
    assert!(is_no_output(choice(vec![]), &config(0.0)));
    assert!(is_no_output(
        seq(vec![text("前"), choice(vec![])]),
        &config(0.0)
    ));
    assert!(is_no_output(
        seq(vec![choice(vec![]), text("後")]),
        &config(0.0)
    ));
    // 深い位置・複数段の入れ子でも伝播する。
    let deep = seq(vec![
        text("a"),
        choice(vec![scored(
            1.0,
            seq(vec![text("b"), seq(vec![choice(vec![])])]),
        )]),
    ]);
    assert!(is_no_output(deep, &config(0.0)));
    // 選ばれたChoiceの中身が空のChoiceでも同じ。
    let root = choice(vec![scored(1.0, choice(vec![])), scored(0.0, text("x"))]);
    assert!(is_no_output(root, &config(0.0)));
}

#[test]
fn an_empty_choice_in_an_unselected_branch_does_not_matter() {
    let root = choice(vec![
        scored(1.0, text("保持")),
        scored(0.0, choice(vec![])),
        scored(-1.0, seq(vec![text("x"), choice(vec![])])),
    ]);
    assert_eq!(selected(root, &config(0.0)), text("保持"));
    // 空のChoiceが後ろにあっても、別の選択肢がそれより高ければ選ばれない。
    let root = seq(vec![choice(vec![
        scored(1.0, text("A")),
        scored(2.0, choice(vec![])),
    ])]);
    assert!(is_no_output(root, &config(0.0)));
}

#[test]
fn the_empty_choice_decision_follows_the_score_not_the_position() {
    // 空Choiceのスコアが低いので採用されず、結果はData。
    let root = choice(vec![
        scored(-5.0, choice(vec![])),
        scored(0.0, text("採用")),
    ]);
    assert_eq!(selected(root, &config(0.0)), text("採用"));
    // 省略スコアはunscoredとして評価される: unscored=3なら空Choiceが勝つ。
    let root = choice(vec![unscored(choice(vec![])), scored(2.0, text("x"))]);
    assert!(is_no_output(root.clone(), &config(3.0)));
    assert_eq!(selected(root, &config(1.0)), text("x"));
}

// ---- 異常系 --------------------------------------------------------------------

#[test]
fn invalid_packets_are_rejected_even_when_the_bad_part_is_not_selected() {
    let bad_version = Packet {
        version: 2,
        root: text("a"),
    };
    let nan_unselected = Packet::new(choice(vec![
        scored(1.0, text("a")),
        scored(f64::NAN, text("b")),
    ]));
    let infinite = Packet::new(choice(vec![scored(f64::INFINITY, text("a"))]));
    let neg_infinite = Packet::new(seq(vec![choice(vec![scored(
        f64::NEG_INFINITY,
        text("a"),
    )])]));
    let reversed_span = Packet::new(choice(vec![
        scored(5.0, text("ok")),
        scored(
            0.0,
            Candidate::Text {
                text: "bad".into(),
                meta: Some(TextMeta {
                    reading: None,
                    source_span: Some([5, 1]),
                }),
            },
        ),
    ]));
    for (name, packet) in [
        ("version", bad_version),
        ("nan", nan_unselected),
        ("inf", infinite),
        ("-inf", neg_infinite),
        ("span", reversed_span),
    ] {
        let input = OutputValue::Data(Arc::new(packet));
        assert_eq!(
            error_code(run_input(input, &config(0.0))),
            "invalid-packet",
            "{name}"
        );
    }
}

#[test]
fn no_output_input_returns_no_output_and_a_missing_input_is_an_error() {
    assert_eq!(
        run_input(OutputValue::NoOutput, &config(0.0)).unwrap(),
        OutputValue::NoOutput
    );
    let error = SelectCandidates
        .execute(context(), &Inputs::new(), &config(0.0))
        .expect_err("missing required input");
    assert_eq!(error.code, "missing-input");
}

#[test]
fn a_non_finite_number_cannot_become_a_valid_config() {
    // JSONはNaN・無限大を表現できず、serde_jsonではnullになるため設定として拒否される。
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let config = json!({"unscored": value});
        assert_eq!(
            SelectCandidates.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        let input = OutputValue::Data(Arc::new(Packet::new(text("a"))));
        assert_eq!(error_code(run_input(input, &config)), "invalid-config");
    }
}

// ---- 不変性・再現性・分離 ------------------------------------------------------

#[test]
fn the_input_packet_is_not_modified() {
    let original = Packet::new(seq(vec![
        choice(vec![
            scored(1.0, text("a")),
            unscored(choice(vec![scored(2.0, text("b")), scored(3.0, text("c"))])),
        ]),
        text("d"),
    ]));
    let shared = Arc::new(original.clone());
    let input = OutputValue::Data(Arc::clone(&shared));
    let output = run_input(input.clone(), &config(5.0)).unwrap();
    assert_eq!(*shared, original);
    let OutputValue::Data(output) = output else {
        panic!("expected Data")
    };
    // 出力は入力とは別のPacketで、入力側の共有カウントに残らない。
    assert!(!Arc::ptr_eq(&shared, &output));
    drop(output);
    drop(input);
    assert_eq!(Arc::strong_count(&shared), 1);
    assert_eq!(*shared, original);
}

#[test]
fn the_same_input_and_config_give_the_same_output_every_time() {
    let root = seq(vec![
        choice(vec![
            scored(0.5, text("a")),
            scored(0.5, text("b")),
            unscored(text("c")),
        ]),
        choice(vec![scored(-1.0, text("d")), scored(-1.0, text("e"))]),
    ]);
    let first = run(root.clone(), &config(0.5)).unwrap();
    for _ in 0..50 {
        assert_eq!(run(root.clone(), &config(0.5)).unwrap(), first);
    }
    assert_eq!(
        first,
        OutputValue::Data(Arc::new(Packet::new(seq(vec![text("a"), text("d")]))))
    );
}

#[test]
fn deeply_nested_trees_do_not_overflow_the_stack() {
    // JSONの深さ上限（32）を超えるデータもコード上は作れるため、再帰に依存しないことを確認する。
    const DEPTH: usize = 2_000;
    let mut root = text("葉");
    for level in 0..DEPTH {
        root = if level % 2 == 0 {
            seq(vec![root])
        } else {
            choice(vec![scored(1.0, root), scored(0.0, text("捨"))])
        };
    }
    let output = selected(root, &config(0.0));
    assert!(!contains_choice(&output));
    // 解決後は、Sequenceが入れ子に残る。
    let mut depth = 0;
    let mut cursor = &output;
    while let Candidate::Sequence { items } = cursor {
        depth += 1;
        cursor = &items[0];
    }
    assert_eq!(depth, DEPTH / 2);
    assert_eq!(*cursor, text("葉"));
    // 深い木は再帰的なdropでスタックを使い切らないよう、明示的に平坦化して破棄する。
    std::mem::forget(output);
}

#[test]
fn wide_choices_select_the_maximum_efficiently() {
    let options: Vec<_> = (0..50_000)
        .map(|index| scored(f64::from(index % 1000), text(&index.to_string())))
        .collect();
    // 最大スコア999の最初の要素はindex 999。
    assert_eq!(selected(choice(options), &config(0.0)), text("999"));
}

#[test]
fn separate_instances_in_one_registry_do_not_interfere() {
    let graph = json!({
        "version": 1,
        "nodes": [
            {"id": "source", "kind": "test.ja-source", "version": 1, "config": {"packet": {"version": 1, "root":
                {"type": "choice", "options": [
                    {"value": {"type": "text", "text": "省略"}},
                    {"score": 5, "value": {"type": "text", "text": "五"}}]}}}},
            {"id": "low", "kind": KIND, "version": 1, "config": {"unscored": 0}},
            {"id": "high", "kind": KIND, "version": 1, "config": {"unscored": 100}},
            {"id": "default", "kind": KIND, "version": 1},
            {"id": "chain", "kind": KIND, "version": 1, "config": {"unscored": 50}}
        ],
        "edges": [
            {"from": {"node": "source", "port": "out"}, "to": {"node": "low", "port": "in"}},
            {"from": {"node": "source", "port": "out"}, "to": {"node": "high", "port": "in"}},
            {"from": {"node": "source", "port": "out"}, "to": {"node": "default", "port": "in"}},
            {"from": {"node": "high", "port": "out"}, "to": {"node": "chain", "port": "in"}}
        ]
    });
    let graph = Graph::from_json(&graph.to_string()).unwrap();
    let registry = registry();
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    for _ in 0..3 {
        let report = engine.run().unwrap();
        assert!(report.succeeded());
        let root = |id: &str| report.packet(id, "out").unwrap().root.clone();
        assert_eq!(root("low"), text("五")); // unscored=0 < 5
        assert_eq!(root("high"), text("省略")); // unscored=100 > 5
        assert_eq!(root("default"), text("五")); // 既定0
        assert_eq!(root("chain"), text("省略")); // 選択済みTextをそのまま通す
    }
    // 別のRegistry・別のEngineでも結果は同じ。
    let other = Engine::new(ExecutionPlan::compile(&graph, &self::registry()).unwrap())
        .run()
        .unwrap();
    assert_eq!(other.packet("low", "out").unwrap().root, text("五"));
    assert_eq!(other.packet("high", "out").unwrap().root, text("省略"));
}

#[test]
fn interleaved_runs_on_the_same_node_value_keep_no_state() {
    let node = SelectCandidates;
    let make = |winner: &str| {
        Arc::new(Packet::new(choice(vec![
            scored(1.0, text("負")),
            scored(2.0, text(winner)),
        ])))
    };
    let run_with = |packet: &Arc<Packet>, unscored: f64| {
        let inputs: Inputs = [("in".to_owned(), OutputValue::Data(Arc::clone(packet)))].into();
        node.execute(context(), &inputs, &config(unscored)).unwrap()
    };
    let a = make("A");
    let b = make("B");
    let first_a = run_with(&a, 0.0);
    let first_b = run_with(&b, 0.0);
    assert_eq!(run_with(&a, 0.0), first_a);
    assert_eq!(run_with(&b, 0.0), first_b);
    assert_ne!(first_a, first_b);
    // 設定を変えた実行が、その後の実行へ影響しない。
    let _ = run_with(&a, 100.0);
    assert_eq!(run_with(&a, 0.0), first_a);
}

// ---- サンプル ------------------------------------------------------------------

#[test]
fn the_example_graph_runs_on_the_existing_engine_with_an_individual_registry() {
    let graph = Graph::from_json(EXAMPLE).unwrap();
    // 標準Registryからもそのまま実行できる。
    let mut standard = Registry::new();
    register_builtin_nodes(&mut standard).unwrap();
    assert!(ExecutionPlan::compile(&graph, &standard).is_ok());
    let report = Engine::new(ExecutionPlan::compile(&graph, &registry()).unwrap())
        .run()
        .unwrap();
    assert!(report.succeeded());
    assert_eq!(report.nodes["select"].status, NodeStatus::Executed);
    // 既存のe2e.select-candidatesと同じ選択結果になる。
    let selected = &report.packet("select", "out").unwrap().root;
    assert!(!contains_choice(selected));
    let Candidate::Sequence { items } = selected else {
        panic!("expected a Sequence")
    };
    assert_eq!(items.len(), 6);
    // 選択後のSequenceをそのまま文字列化すると、期待する文章になる。
    let joined: String = items
        .iter()
        .map(|item| match item {
            Candidate::Text { text, .. } => text.as_str(),
            other => panic!("expected Text, got {other:?}"),
        })
        .collect();
    assert_eq!(joined, EXPECTED);
}
