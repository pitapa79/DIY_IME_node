#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
use std::sync::Arc;

use crate::test_host::{
    Candidate, ChoiceOption, Packet,
    node::{CoreError, Inputs, Node, OutputValue, PrepareResources, RootConstraint, RunContext},
    opt_in::dictionary_reverse::{DictionaryReverse, KIND, ReverseDictionary},
};
use serde_json::{Value, json};

const SAMPLE: &[u8] = include_bytes!("../examples/nodes/dictionary_reverse/dictionary.tsv");

fn text(value: &str) -> Packet {
    Packet::new(Candidate::Text {
        text: value.into(),
        meta: None,
    })
}
fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "dictionary-reverse",
        input: None,
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/dictionary_reverse/contract.json"
    ))
    .unwrap()
}
struct MemoryResources(std::collections::BTreeMap<String, Arc<[u8]>>);
impl PrepareResources for MemoryResources {
    fn read(&mut self, reference: &str, maximum: usize) -> Result<Arc<[u8]>, CoreError> {
        assert_eq!(
            maximum,
            crate::test_host::nodes::dictionary_exact::MAX_FILE_BYTES
        );
        self.0
            .get(reference)
            .cloned()
            .ok_or_else(|| CoreError::new("resource-read", "test resource missing"))
    }
}
fn resources(bytes: &[u8]) -> MemoryResources {
    MemoryResources([("dictionary.tsv".into(), Arc::from(bytes))].into())
}
fn prepared(bytes: &[u8], config: &Value) -> Arc<dyn crate::test_host::node::PreparedNode> {
    DictionaryReverse
        .prepare(config, &mut resources(bytes))
        .unwrap()
        .unwrap()
}
fn expected_packet(case: &Value) -> Packet {
    let options = case["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| ChoiceOption {
            value: Candidate::Text {
                text: option["text"].as_str().unwrap().into(),
                meta: None,
            },
            score: Some(option["score"].as_f64().unwrap()),
        })
        .collect();
    Packet::new(Candidate::Choice { options })
}
#[test]
fn fixture_cases_match_the_prepared_node() {
    let fixture = fixture();
    assert_eq!(fixture["kind"], KIND);
    for case in fixture["cases"].as_array().unwrap() {
        let config = case.get("config").unwrap_or(&fixture["defaultConfig"]);
        let node = prepared(SAMPLE, config);
        let input: Inputs = [(
            "in".into(),
            OutputValue::Data(Arc::new(text(case["input"].as_str().unwrap()))),
        )]
        .into();
        let before = input.clone();
        let outputs = node.execute(context(), &input).unwrap();
        assert_eq!(
            outputs["out"],
            OutputValue::Data(Arc::new(expected_packet(case))),
            "case={}",
            case["id"]
        );
        assert_eq!(input, before, "input must not change: case={}", case["id"]);
        let again = node.execute(context(), &input).unwrap();
        assert_eq!(again["out"], outputs["out"], "case={}", case["id"]);
    }
}
#[test]
fn prepared_node_distinguishes_no_output_missing_and_invalid_inputs() {
    let fixture = fixture();
    let node = prepared(SAMPLE, &fixture["defaultConfig"]);
    let no_output: Inputs = [("in".into(), OutputValue::NoOutput)].into();
    assert_eq!(
        node.execute(context(), &no_output).unwrap()["out"],
        OutputValue::NoOutput
    );
    assert_eq!(
        node.execute(context(), &Inputs::new()).unwrap_err().code,
        "missing-input"
    );
    let too_long: Inputs = [(
        "in".into(),
        OutputValue::Data(Arc::new(text(&"あ".repeat(4097)))),
    )]
    .into();
    assert_eq!(
        node.execute(context(), &too_long).unwrap_err().code,
        "dictionary-input-limit"
    );
    let wrong_root: Inputs = [(
        "in".into(),
        OutputValue::Data(Arc::new(Packet::new(Candidate::Sequence { items: vec![] }))),
    )]
    .into();
    assert_eq!(
        node.execute(context(), &wrong_root).unwrap_err().code,
        "invalid-root"
    );
}
#[test]
fn definition_config_and_prepare_failures_match_the_family() {
    let node = DictionaryReverse;
    let definition = node.definition();
    assert_eq!(definition.kind, KIND);
    assert_eq!(definition.version, 1);
    assert_eq!(definition.inputs[0].port.root, RootConstraint::Text);
    assert_eq!(definition.outputs[0].root, RootConstraint::Choice);
    assert_eq!(
        definition.default_config,
        json!({ "path": "dictionary.tsv", "limit": 64 })
    );
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            node.validate_config(invalid).unwrap_err().code,
            "invalid-config",
            "{invalid}"
        );
    }
    assert_eq!(
        node.execute(context(), &Inputs::new(), &definition.default_config)
            .unwrap_err()
            .code,
        "dictionary-not-prepared"
    );
    let config = &definition.default_config;
    let mut empty = MemoryResources(Default::default());
    assert_eq!(
        node.prepare(config, &mut empty)
            .err()
            .expect("prepare must fail")
            .code,
        "dictionary-read"
    );
    let duplicate = "reading\tsurface\tweight\nあ\t亜\t1\nあ\t亜\t2\n";
    assert_eq!(
        node.prepare(config, &mut resources(duplicate.as_bytes()))
            .err()
            .expect("prepare must fail")
            .code,
        "dictionary-format"
    );
}
#[test]
fn pure_api_agrees_with_the_prepared_node() {
    let dictionary = ReverseDictionary::from_tsv_bytes(SAMPLE).unwrap();
    let fixture = fixture();
    let case = &fixture["cases"][2];
    assert_eq!(
        dictionary.lookup_packet(&text("今日"), 64).unwrap(),
        expected_packet(case)
    );
    assert_eq!(
        dictionary
            .lookup_output(&OutputValue::NoOutput, 64)
            .unwrap(),
        OutputValue::NoOutput
    );
}
