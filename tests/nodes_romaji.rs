//! ローマ字変換ノードの検査。受け入れ例（tests/contracts/romaji.json）を実装へ照合し、
//! 境界・Unicode・設定・不変性・インスタンス分離を追加で確認する。
#[path = "support/mod.rs"]
mod test_host;
use std::sync::Arc;

use crate::test_host::{
    Candidate, Packet,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    node::{
        CoreError, InputMode, Inputs, Node, OutputValue, Outputs, Registry, RunContext,
        RuntimeInput,
    },
    nodes::romaji::{Config, Mode, RomajiToHiragana, Unknown, convert},
    runtime::{Engine, NodeStatus, ResolvedValue},
};
use serde_json::{Value, json};

const CONTRACT: &str = include_str!("contracts/romaji.json");

fn default_config() -> Value {
    json!({"overrides": [], "unknown": "preserve"})
}
fn text_packet(text: &str) -> Packet {
    Packet::new(Candidate::Text {
        text: text.into(),
        meta: None,
    })
}
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "romaji",
        input: None,
    }
}
fn run_node(packet: Packet, config: &Value) -> Result<Outputs, CoreError> {
    let inputs: Inputs = [("in".to_string(), OutputValue::Data(Arc::new(packet)))].into();
    RomajiToHiragana.execute(context(), &inputs, config)
}
fn out_text(outputs: &Outputs, port: &str) -> String {
    let OutputValue::Data(packet) = &outputs[port] else {
        panic!("{port} should be Data")
    };
    let Candidate::Text { text, meta } = &packet.root else {
        panic!("{port} should be Text")
    };
    assert!(meta.is_none(), "{port} must not carry meta");
    text.clone()
}
fn convert_json(text: &str, mode: Mode, config: &Value) -> Result<(String, String), CoreError> {
    let config = Config::from_value(config)?;
    convert(text, mode, &config).map(|c| (c.text, c.pending))
}
fn finalize(text: &str) -> String {
    convert_json(text, Mode::Finalize, &default_config())
        .unwrap()
        .0
}

fn contract_cases() -> Vec<Value> {
    let contract: Value = serde_json::from_str(CONTRACT).unwrap();
    contract["cases"].as_array().unwrap().clone()
}

/// 全ケースを変換の核へ照合する（packet / inputStatus を使うケースはNode側で検査）。
#[test]
fn contract_cases_match_the_conversion_core() {
    let mut checked = 0;
    for case in contract_cases() {
        let id = case["id"].as_str().unwrap();
        if case.get("packet").is_some() || case.get("inputStatus").is_some() {
            continue;
        }
        let mode = match case["mode"].as_str().unwrap() {
            "compose" => Mode::Compose,
            "finalize" => Mode::Finalize,
            other => panic!("{id}: unknown mode {other}"),
        };
        let config = case.get("config").cloned().unwrap_or_else(default_config);
        let actual = convert_json(case["text"].as_str().unwrap(), mode, &config);
        let expected = &case["expected"];
        match expected["status"].as_str().unwrap() {
            "data" => {
                let (text, pending) = actual.unwrap_or_else(|e| panic!("{id}: {e}"));
                assert_eq!(text, expected["text"].as_str().unwrap(), "{id}: text");
                assert_eq!(
                    pending,
                    expected["pending"].as_str().unwrap(),
                    "{id}: pending"
                );
            }
            "error" => {
                let error = actual.expect_err(id);
                assert_eq!(error.code, expected["code"].as_str().unwrap(), "{id}");
            }
            other => panic!("{id}: unexpected status {other}"),
        }
        checked += 1;
    }
    assert!(checked >= 19, "contract cases were skipped: {checked}");
}

/// Nodeアダプター経由で、compose / finalize・不正入力・NoOutputを検査する。
#[test]
fn contract_cases_match_the_node_adapter() {
    let mut data_and_error_cases = 0;
    for case in contract_cases() {
        let id = case["id"].as_str().unwrap();
        let expected = &case["expected"];
        let config = case.get("config").cloned().unwrap_or_else(default_config);
        if case.get("inputStatus").and_then(Value::as_str) == Some("no-output") {
            let inputs: Inputs = [("in".to_string(), OutputValue::NoOutput)].into();
            let outputs = RomajiToHiragana
                .execute(context(), &inputs, &config)
                .unwrap();
            assert_eq!(outputs["out"], OutputValue::NoOutput, "{id}");
            assert_eq!(outputs["pending"], OutputValue::NoOutput, "{id}");
            continue;
        }
        let packet = match case.get("packet") {
            Some(raw) => serde_json::from_value::<Packet>(raw.clone()).unwrap(),
            None => text_packet(case["text"].as_str().unwrap()),
        };
        data_and_error_cases += 1;
        let input = RuntimeInput {
            revision: 1,
            text: case["text"].as_str().unwrap_or_default().into(),
            mode: if case["mode"] == "compose" {
                InputMode::Compose
            } else {
                InputMode::Finalize
            },
        };
        let inputs = [("in".into(), OutputValue::Data(Arc::new(packet)))].into();
        match (
            expected["status"].as_str().unwrap(),
            RomajiToHiragana.execute(
                RunContext {
                    input: Some(&input),
                    ..context()
                },
                &inputs,
                &config,
            ),
        ) {
            ("data", Ok(outputs)) => {
                assert_eq!(
                    out_text(&outputs, "out"),
                    expected["text"].as_str().unwrap(),
                    "{id}"
                );
                assert_eq!(
                    out_text(&outputs, "pending"),
                    expected["pending"].as_str().unwrap(),
                    "{id}"
                );
            }
            ("error", Err(error)) => {
                assert_eq!(error.code, expected["code"].as_str().unwrap(), "{id}")
            }
            (status, result) => panic!("{id}: expected {status}, got {result:?}"),
        }
    }
    assert_eq!(
        data_and_error_cases, 21,
        "all data and error cases must run"
    );
}

#[test]
fn compose_keeps_unfinished_tails_and_finalize_resolves_them() {
    let compose = |t: &str| convert_json(t, Mode::Compose, &default_config()).unwrap();
    for (input, text, pending) in [
        ("", "", ""),
        ("k", "", "k"),
        ("sh", "", "sh"),
        ("ts", "", "ts"),
        ("xts", "", "xts"),
        ("kak", "か", "k"),
        ("kyak", "きゃ", "k"),
        ("kk", "っ", "k"),
        ("kky", "っ", "ky"),
        ("nn", "ん", ""),
        ("nnn", "ん", "n"),
        ("na", "な", ""),
        ("ny", "", "ny"),
        ("kan", "か", "n"),
        ("kann", "かん", ""),
        ("kanna", "かんな", ""),
        ("kinnen", "きんね", "n"),
        ("ka n", "か ", "n"),
        ("n3", "ん3", ""),
        ("n'", "ん", ""),
        ("Kk", "っ", "k"),
        ("KYA", "きゃ", ""),
        ("ShI", "し", ""),
    ] {
        assert_eq!(
            compose(input),
            (text.into(), pending.into()),
            "compose {input:?}"
        );
    }
    for (input, expected) in [
        ("kan", "かん"),
        ("n", "ん"),
        ("kk", "っk"),
        ("sh", "sh"),
        ("ny", "ny"),
        ("Kan", "かん"),
        ("kya", "きゃ"),
        ("xtu", "っ"),
        ("lya", "ゃ"),
        ("tsu", "つ"),
        ("ji", "じ"),
        ("zi", "じ"),
        ("wo", "を"),
        ("xn", "xん"),
    ] {
        assert_eq!(finalize(input), expected, "finalize {input:?}");
    }
}

#[test]
fn gemination_only_applies_to_consonants_with_rules() {
    assert_eq!(finalize("tt"), "っt");
    assert_eq!(finalize("kka"), "っか");
    assert_eq!(finalize("ccha"), "っちゃ");
    assert_eq!(finalize("sshi"), "っし");
    assert_eq!(finalize("ppa"), "っぱ");
    // 規則のない子音（q / v）、母音、n、x / l は促音化しない。
    assert_eq!(finalize("qq"), "qq");
    assert_eq!(finalize("vva"), "vvあ");
    assert_eq!(finalize("aa"), "ああ");
    // x / l の重複は促音にせず、先頭を未知として残して後続を小書き母音にする。
    assert_eq!(finalize("xxa"), "xぁ");
    assert_eq!(finalize("lla"), "lぁ");
}

#[test]
fn passes_digits_symbols_kana_and_unicode_through() {
    assert_eq!(finalize("30%"), "30%");
    assert_eq!(finalize("a b\tc\n"), "あ b\tc\n");
    assert_eq!(finalize("すし"), "すし");
    assert_eq!(finalize("ｋa"), "ｋあ"); // 全角英字は英字として扱わない
    assert_eq!(finalize("é"), "é");
    assert_eq!(finalize("😀ka😀"), "😀か😀");
    assert_eq!(finalize("𠮷ka"), "𠮷か");
    assert_eq!(finalize("it's"), "いt's"); // 規則のないtは未知としてそのまま
    // 記号のアポストロフィはunknown=errorでも失敗させない。
    let strict = json!({"overrides": [], "unknown": "error"});
    assert_eq!(convert_json("'", Mode::Finalize, &strict).unwrap().0, "'");
    // 結合文字・ZWJなど複数コードポイントも落とさない。
    assert_eq!(finalize("ka\u{3099}"), "か\u{3099}");
    assert_eq!(finalize("👨‍👩‍👧ka"), "👨‍👩‍👧か");
}

#[test]
fn unknown_policy_distinguishes_unsupported_from_incomplete() {
    let strict = json!({"overrides": [], "unknown": "error"});
    let code = |text: &str, mode| convert_json(text, mode, &strict).unwrap_err().code;
    assert_eq!(code("q", Mode::Compose), "unsupported-romaji");
    assert_eq!(code("kq", Mode::Compose), "unsupported-romaji");
    assert_eq!(code("v", Mode::Finalize), "unsupported-romaji");
    assert_eq!(code("ka k", Mode::Finalize), "incomplete-romaji");
    assert_eq!(code("sh", Mode::Finalize), "incomplete-romaji");
    // 入力途中の未完はエラーにしない。nは確定時に「ん」。
    assert_eq!(
        convert_json("kak", Mode::Compose, &strict).unwrap(),
        ("か".into(), "k".into())
    );
    assert_eq!(convert_json("n", Mode::Finalize, &strict).unwrap().0, "ん");
    // preserveは原文の大小を保持する。
    let preserve = default_config();
    assert_eq!(convert_json("Q", Mode::Compose, &preserve).unwrap().0, "Q");
    assert_eq!(
        convert_json("KA K", Mode::Finalize, &preserve).unwrap().0,
        "か K"
    );
    // 規則のない k+S は k を未知として残し、末尾の Sh だけを入力途中として保持する。
    assert_eq!(
        convert_json("kSh", Mode::Compose, &preserve).unwrap(),
        ("k".into(), "Sh".into())
    );
}

#[test]
fn overrides_replace_add_and_prefer_longer_matches() {
    let config = |entries: Value| json!({"overrides": entries, "unknown": "preserve"});
    let run =
        |entries: Value, text: &str, mode| convert_json(text, mode, &config(entries)).unwrap();
    // 置換
    assert_eq!(
        run(json!([{"from": "si", "to": "スィ"}]), "si", Mode::Finalize).0,
        "スィ"
    );
    // 追加（既定にない規則）。接頭辞も入力途中として保持される。
    let added = json!([{"from": "qa", "to": "くぁ"}]);
    assert_eq!(
        run(added.clone(), "qa", Mode::Compose),
        ("くぁ".into(), "".into())
    );
    assert_eq!(run(added, "q", Mode::Compose), ("".into(), "q".into()));
    // 長い一致を優先する。
    let longer = json!([{"from": "kaa", "to": "X"}, {"from": "ka", "to": "Y"}]);
    assert_eq!(run(longer.clone(), "kaa", Mode::Finalize).0, "X");
    assert_eq!(run(longer, "ka", Mode::Compose), ("Y".into(), "".into()));
    // アポストロフィを含むfrom、複数コードポイントのto。
    assert_eq!(
        run(
            json!([{"from": "a'b", "to": "あいう"}]),
            "a'b",
            Mode::Finalize
        )
        .0,
        "あいう"
    );
    // 上限ちょうどは有効。
    let max_from = "a'a'a'a'";
    assert_eq!(
        run(
            json!([{"from": max_from, "to": "𠮷".repeat(16)}]),
            max_from,
            Mode::Finalize
        )
        .0,
        "𠮷".repeat(16)
    );
}

#[test]
fn invalid_configs_are_rejected_at_validation() {
    let node = RomajiToHiragana;
    let bad = |value: Value| {
        let error = node.validate_config(&value).expect_err(&value.to_string());
        assert_eq!(error.code, "invalid-config", "{value}");
    };
    assert!(node.validate_config(&default_config()).is_ok());
    let entry = |from: &str, to: &str| json!({"from": from, "to": to});
    let with = |entries: Vec<Value>| json!({"overrides": entries, "unknown": "preserve"});
    bad(json!({}));
    bad(json!({"unknown": "preserve"})); // 部分指定はマージしない
    bad(json!({"overrides": []}));
    bad(json!({"overrides": [], "unknown": "preserve", "extra": 1}));
    bad(json!({"overrides": [], "unknown": "ignore"}));
    bad(json!({"overrides": [], "unknown": null}));
    bad(json!({"overrides": null, "unknown": "preserve"}));
    bad(json!({"overrides": {}, "unknown": "preserve"}));
    bad(json!([]));
    bad(with(vec![json!("ka")]));
    bad(with(vec![json!({"from": "ka"})]));
    bad(with(vec![json!({"from": "ka", "to": "か", "x": 1})]));
    bad(with(vec![json!({"from": 1, "to": "か"})]));
    bad(with(vec![entry("", "か")]));
    bad(with(vec![entry("ka", "")]));
    bad(with(vec![entry("Ka", "か")])); // 大文字
    bad(with(vec![entry("k1", "か")]));
    bad(with(vec![entry("ｋａ", "か")]));
    bad(with(vec![entry("abcdefghi", "か")])); // 9文字
    bad(with(vec![entry("ka", &"あ".repeat(17))]));
    bad(with(vec![entry("ka", "か"), entry("ka", "ヵ")]));
    for reserved in ["n", "nn", "n'"] {
        bad(with(vec![entry(reserved, "ん")]));
    }
    let many: Vec<Value> = (0..257)
        .map(|i| entry(&"a".repeat(1 + i % 8), "あ"))
        .collect();
    bad(with(many));
    let at_limit: Vec<Value> = (0..256)
        .map(|i| {
            entry(
                &format!(
                    "{}{}",
                    char::from(b'a' + (i / 26) as u8 % 26),
                    char::from(b'a' + (i % 26) as u8)
                ),
                "あ",
            )
        })
        .collect();
    assert!(node.validate_config(&with(at_limit)).is_ok());
    // 実行時にも同じ検査で拒否する（計画準備を迂回しても成功しない）。
    let error = run_node(text_packet("ka"), &json!({"overrides": [], "unknown": "x"})).unwrap_err();
    assert_eq!(error.code, "invalid-config");
    // convertへ直接渡したConfigも検査する。
    let broken = Config {
        overrides: vec![],
        unknown: Unknown::Preserve,
    };
    assert!(convert("ka", Mode::Finalize, &broken).is_ok());
}

#[test]
fn wrong_roots_invalid_packets_and_missing_inputs_are_errors() {
    for root in [
        Candidate::Sequence { items: vec![] },
        Candidate::Choice { options: vec![] },
    ] {
        let error = run_node(Packet::new(root), &default_config()).unwrap_err();
        assert_eq!(error.code, "invalid-root");
    }
    let mut broken = text_packet("ka");
    broken.version = 99;
    assert_eq!(
        run_node(broken, &default_config()).unwrap_err().code,
        "invalid-packet"
    );
    let empty = Inputs::new();
    assert_eq!(
        RomajiToHiragana
            .execute(context(), &empty, &default_config())
            .unwrap_err()
            .code,
        "missing-input"
    );
}

#[test]
fn empty_text_is_data_and_input_meta_is_not_copied() {
    let outputs = run_node(text_packet(""), &default_config()).unwrap();
    assert_eq!(out_text(&outputs, "out"), "");
    assert_eq!(out_text(&outputs, "pending"), "");
    let with_meta: Packet = serde_json::from_value(json!({
        "version": 1,
        "root": {"type": "text", "text": "ka", "meta": {"reading": "か"}}
    }))
    .unwrap();
    let outputs = run_node(with_meta, &default_config()).unwrap();
    assert_eq!(out_text(&outputs, "out"), "か"); // out_textがmetaなしを検査する
}

#[test]
fn does_not_mutate_input_and_is_repeatable() {
    let packet = Arc::new(text_packet("honjitunokousuikakurituha30%desu"));
    let before = (*packet).clone();
    let inputs: Inputs = [("in".to_string(), OutputValue::Data(packet.clone()))].into();
    let config = default_config();
    let first = RomajiToHiragana
        .execute(context(), &inputs, &config)
        .unwrap();
    for _ in 0..5 {
        let again = RomajiToHiragana
            .execute(context(), &inputs, &config)
            .unwrap();
        assert_eq!(again, first);
    }
    assert_eq!(*packet, before);
    assert_eq!(config, default_config());
}

/// 小さな決定的乱数でASCII入力を大量に作り、不変条件を確認する。
#[test]
fn properties_hold_for_generated_inputs() {
    let alphabet: Vec<char> = "aiueokstnhmyrwgzdbpfjcxlqvKNYAn''  1%あ𠮷"
        .chars()
        .collect();
    let mut state = 0x2545F4914F6CDD1Du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let preserve = Config::default();
    for _ in 0..3000 {
        let len = (next() % 14) as usize;
        let input: String = (0..len)
            .map(|_| alphabet[(next() % alphabet.len() as u64) as usize])
            .collect();
        let compose = convert(&input, Mode::Compose, &preserve).unwrap();
        let done = convert(&input, Mode::Finalize, &preserve).unwrap();
        // 確定結果は常にpending空。入力途中のpendingは原文の末尾そのもの。
        assert!(done.pending.is_empty(), "{input:?}");
        assert!(
            input.ends_with(&compose.pending),
            "{input:?} -> {compose:?}"
        );
        // composeのtextにpendingを確定して足すと、全文のfinalizeと一致する。
        let tail = convert(&compose.pending, Mode::Finalize, &preserve).unwrap();
        assert_eq!(
            format!("{}{}", compose.text, tail.text),
            done.text,
            "{input:?}"
        );
        // 反復しても同じ。
        assert_eq!(compose, convert(&input, Mode::Compose, &preserve).unwrap());
    }
}

fn instance(id: &str, kind: &str, config: Option<Value>) -> NodeInstance {
    NodeInstance {
        id: id.into(),
        kind: kind.into(),
        version: 1,
        config,
    }
}
fn registry() -> Registry {
    // 統合後の標準Registryを使い、実際の登録と接続も検査する。
    let mut registry = Registry::new();
    crate::test_host::builtins::register_builtin_nodes(&mut registry).unwrap();
    registry
}
fn source(id: &str, text: &str) -> NodeInstance {
    // ja-Latn の固定入力（言語ラベルの接続検査を通すため e2e.input を使う）。
    instance(
        id,
        "e2e.input",
        Some(json!({"mode": "fixed", "text": text})),
    )
}
fn edge(from: &str, port: &str, to: &str, input: &str) -> Edge {
    Edge {
        from: Endpoint::new(from, port),
        to: Endpoint::new(to, input),
    }
}

#[test]
fn engine_runs_multiple_instances_without_interference() {
    let registry = registry();
    let graph = Graph {
        version: 1,
        nodes: vec![
            source("src-a", "ka"),
            source("src-b", "ka"),
            source("src-c", "kak"),
            instance("plain", "text.romaji-to-hiragana", None),
            instance(
                "custom",
                "text.romaji-to-hiragana",
                Some(json!({"overrides": [{"from": "ka", "to": "ヵ"}], "unknown": "preserve"})),
            ),
            instance(
                "strict",
                "text.romaji-to-hiragana",
                Some(json!({"overrides": [], "unknown": "error"})),
            ),
        ],
        edges: vec![
            edge("src-a", "out", "plain", "in"),
            edge("src-b", "out", "custom", "in"),
            edge("src-c", "out", "strict", "in"),
        ],
    };
    let plan = ExecutionPlan::compile(&graph, &registry).unwrap();
    let mut engine = Engine::new(plan);
    for _ in 0..3 {
        let report = engine.run().unwrap();
        let text = |node: &str, port: &str| match &report.nodes[node].outputs[port] {
            ResolvedValue::Data(packet) => match &packet.root {
                Candidate::Text { text, .. } => text.clone(),
                other => panic!("expected Text, got {other:?}"),
            },
            other => panic!("{node}.{port}: {other:?}"),
        };
        assert_eq!(report.nodes["plain"].status, NodeStatus::Executed);
        assert_eq!(text("plain", "out"), "か");
        assert_eq!(text("plain", "pending"), "");
        assert_eq!(text("custom", "out"), "ヵ");
        // 固定グラフはfinalize。未完のkはunknown=errorなのでノードが失敗し、両出力がエラーになる。
        assert_eq!(report.nodes["strict"].status, NodeStatus::Failed);
        let diagnostic = report.nodes["strict"].diagnostic.as_ref().unwrap();
        assert_eq!(diagnostic.error.code, "incomplete-romaji");
        for port in ["out", "pending"] {
            assert!(matches!(
                report.nodes["strict"].outputs[port],
                ResolvedValue::Error(_)
            ));
        }
    }
}

#[test]
fn node_definition_matches_the_contract() {
    let definition = RomajiToHiragana.definition();
    assert_eq!(definition.kind, "text.romaji-to-hiragana");
    assert_eq!(definition.version, 1);
    assert_eq!(definition.default_config, default_config());
    assert_eq!(definition.inputs.len(), 1);
    assert!(definition.inputs[0].required);
    assert_eq!(definition.inputs[0].port.id, "in");
    assert_eq!(
        definition.inputs[0].port.language.as_deref(),
        Some("ja-Latn")
    );
    let outputs: Vec<_> = definition
        .outputs
        .iter()
        .map(|p| (p.id.as_str(), p.language.as_deref().unwrap()))
        .collect();
    assert_eq!(outputs, [("out", "ja"), ("pending", "ja-Latn")]);
    // 完成したノードは標準Registryから利用できる。
    let mut standard = Registry::new();
    crate::test_host::builtins::register_builtin_nodes(&mut standard).unwrap();
    assert!(
        standard
            .definitions()
            .any(|d| d.kind == "text.romaji-to-hiragana")
    );
}

#[test]
fn bundled_examples_are_valid_and_runnable() {
    let configs: Value = serde_json::from_str(include_str!(
        "../examples/nodes/romaji/config-examples.json"
    ))
    .unwrap();
    for name in ["default", "strict-unknown", "with-override"] {
        RomajiToHiragana
            .validate_config(&configs[name])
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let graph =
        Graph::from_json(include_str!("../examples/nodes/romaji/fixed-graph.json")).unwrap();
    let registry = registry();
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    let ResolvedValue::Data(packet) = &report.nodes["romaji"].outputs["out"] else {
        panic!("romaji.out should be Data")
    };
    assert_eq!(
        packet.root,
        Candidate::Text {
            text: "きんえん".into(),
            meta: None
        }
    );
}

fn runtime_graph(config: Option<Value>) -> Graph {
    Graph {
        version: 1,
        nodes: vec![
            instance("input", "input.text", None),
            instance("romaji", "text.romaji-to-hiragana", config),
        ],
        edges: vec![edge("input", "out", "romaji", "in")],
    }
}

#[test]
fn engine_matches_runtime_contract_cases_and_reports_both_failed_ports() {
    let registry = registry();
    for case in contract_cases() {
        let Some(text) = case["text"].as_str() else {
            continue;
        };
        let id = case["id"].as_str().unwrap();
        let graph = runtime_graph(case.get("config").cloned());
        let compiled = ExecutionPlan::compile(&graph, &registry);
        if case["expected"]["code"] == "invalid-config" {
            assert_eq!(compiled.err().unwrap().code, "invalid-config", "{id}");
            continue;
        }
        let mut engine = Engine::new(compiled.unwrap());
        let input = RuntimeInput {
            revision: 1,
            text: text.into(),
            mode: if case["mode"] == "compose" {
                InputMode::Compose
            } else {
                InputMode::Finalize
            },
        };
        let report = engine.run_with_input(&input).unwrap();
        assert_eq!(report.input_revision, Some(1), "{id}");
        if case["expected"]["status"] == "data" {
            assert!(report.succeeded(), "{id}");
            for (port, expected) in [("out", "text"), ("pending", "pending")] {
                assert_eq!(
                    report.packet("romaji", port).unwrap().root,
                    Candidate::Text {
                        text: case["expected"][expected].as_str().unwrap().into(),
                        meta: None,
                    },
                    "{id}: {port}"
                );
            }
        } else {
            assert!(!report.succeeded(), "{id}");
            assert_eq!(report.nodes["romaji"].status, NodeStatus::Failed);
            for port in ["out", "pending"] {
                let ResolvedValue::Error(diagnostic) = &report.nodes["romaji"].outputs[port] else {
                    panic!("{id}: {port} must fail, not return partial data");
                };
                assert_eq!(diagnostic.error.code, case["expected"]["code"], "{id}");
            }
        }
    }
}

#[test]
fn engine_accepts_full_snapshots_backspace_and_mode_changes_without_recompiling() {
    let mut engine =
        Engine::new(ExecutionPlan::compile(&runtime_graph(None), &registry()).unwrap());
    for (index, (text, mode, out, pending)) in [
        ("k", InputMode::Compose, "", "k"),
        ("ky", InputMode::Compose, "", "ky"),
        ("kya", InputMode::Compose, "きゃ", ""),
        ("ky", InputMode::Compose, "", "ky"),
        ("", InputMode::Compose, "", ""),
        ("n", InputMode::Compose, "", "n"),
        ("n", InputMode::Finalize, "ん", ""),
        ("n", InputMode::Compose, "", "n"),
    ]
    .into_iter()
    .enumerate()
    {
        let input = RuntimeInput {
            revision: index as u64 + 1,
            text: text.into(),
            mode,
        };
        let report = engine.run_with_input(&input).unwrap();
        assert!(report.succeeded());
        assert_eq!(report.plan_version, 1);
        assert_eq!(report.input_revision, Some(input.revision));
        assert_eq!(
            report.packet("romaji", "out").unwrap().root,
            text_packet(out).root
        );
        assert_eq!(
            report.packet("romaji", "pending").unwrap().root,
            text_packet(pending).root
        );
        assert_eq!(engine.plan_version(), 1);
    }
}

#[test]
fn adapter_reads_text_from_the_port_and_only_mode_from_the_context() {
    let input = RuntimeInput {
        revision: 1,
        text: "unrelated source text".into(),
        mode: InputMode::Compose,
    };
    let inputs = [("in".into(), OutputValue::Data(Arc::new(text_packet("n"))))].into();
    let outputs = RomajiToHiragana
        .execute(
            RunContext {
                input: Some(&input),
                ..context()
            },
            &inputs,
            &default_config(),
        )
        .unwrap();
    assert_eq!(out_text(&outputs, "out"), "");
    assert_eq!(out_text(&outputs, "pending"), "n");
}

fn check_isolated_table_settings() {
    let plain = Config::default();
    let strict = Config {
        unknown: Unknown::Error,
        ..Config::default()
    };
    let custom_a = Config::from_value(&json!({
        "overrides": [{"from":"ka","to":"ヵ"},{"from":"q","to":"く"}],
        "unknown":"preserve"
    }))
    .unwrap();
    let custom_b = Config::from_value(&json!({
        "overrides": [{"from":"ka","to":"カ"},{"from":"q'","to":"🦀"}],
        "unknown":"preserve"
    }))
    .unwrap();
    let result = |text, mode, config: &Config| {
        let value = convert(text, mode, config).unwrap();
        (value.text, value.pending)
    };
    for _ in 0..4 {
        assert_eq!(
            result("KAK", Mode::Compose, &plain),
            ("か".into(), "K".into())
        );
        assert_eq!(
            result("KAQ", Mode::Finalize, &custom_a),
            ("ヵく".into(), "".into())
        );
        assert_eq!(
            result("KAQ'", Mode::Finalize, &custom_b),
            ("カ🦀".into(), "".into())
        );
        assert_eq!(
            result("KAQ", Mode::Finalize, &plain),
            ("かQ".into(), "".into())
        );
        assert_eq!(
            result("KAK", Mode::Compose, &strict),
            ("か".into(), "K".into())
        );
        assert_eq!(
            convert("Q", Mode::Compose, &strict).unwrap_err().code,
            "unsupported-romaji"
        );
        assert_eq!(
            convert("K", Mode::Finalize, &strict).unwrap_err().code,
            "incomplete-romaji"
        );
        assert_eq!(result("K", Mode::Finalize, &plain), ("K".into(), "".into()));
        assert_eq!(result("n", Mode::Compose, &strict), ("".into(), "n".into()));
        assert_eq!(
            result("n", Mode::Finalize, &plain),
            ("ん".into(), "".into())
        );
        assert_eq!(
            result("𠮷KA n'👩‍💻", Mode::Finalize, &plain),
            ("𠮷か ん👩‍💻".into(), "".into())
        );
    }
}

#[test]
fn default_table_is_not_changed_by_alternating_settings() {
    check_isolated_table_settings();
}

#[test]
fn cached_default_table_is_safe_during_parallel_configurations() {
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                check_isolated_table_settings();
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
}
