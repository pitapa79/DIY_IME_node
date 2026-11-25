#![cfg(feature = "opt-in-nodes")]

#[path = "support/mod.rs"]
mod test_host;
use std::sync::Arc;

use crate::test_host::{
    Candidate, ChoiceOption, Packet,
    node::{CoreError, Inputs, Node, OutputValue, PrepareResources, PreparedNode, RunContext},
    opt_in::text_skk_dictionary::{KIND, TextSkkDictionary},
};
use serde_json::Value;

const SAMPLE: &[u8] = include_bytes!("../examples/nodes/text_skk_dictionary/dictionary.skk");

fn context() -> RunContext<'static> {
    RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "test",
        input: None,
    }
}
fn text(value: &str) -> Packet {
    Packet::new(Candidate::Text {
        text: value.into(),
        meta: None,
    })
}
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../examples/nodes/text_skk_dictionary/contract.json"
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
    MemoryResources([("dictionary.skk".into(), Arc::from(bytes))].into())
}
fn prepared(bytes: &[u8], config: &Value) -> Arc<dyn PreparedNode> {
    TextSkkDictionary
        .prepare(config, &mut resources(bytes))
        .unwrap()
        .unwrap()
}
fn expected_packet(case: &Value) -> Packet {
    let options = case["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| ChoiceOption {
            value: Candidate::Text {
                text: item.as_str().unwrap().into(),
                meta: None,
            },
            score: None,
        })
        .collect();
    Packet::new(Candidate::Choice { options })
}
#[test]
fn fixture_cases_match_the_prepared_node() {
    let fixture = fixture();
    assert_eq!(fixture["kind"], KIND);
    for case in fixture["cases"].as_array().unwrap() {
        let node = prepared(SAMPLE, &fixture["defaultConfig"]);
        let input: Inputs = [(
            "in".into(),
            OutputValue::Data(Arc::new(text(case["input"].as_str().unwrap()))),
        )]
        .into();
        let outputs = node.execute(context(), &input).unwrap();
        assert_eq!(
            outputs["out"],
            OutputValue::Data(Arc::new(expected_packet(case))),
            "case={case}"
        );
        let again = node.execute(context(), &input).unwrap();
        assert_eq!(again["out"], outputs["out"], "case={case}");
    }
}
#[test]
fn prepared_node_distinguishes_no_output_missing_and_invalid_inputs() {
    let node = prepared(SAMPLE, &fixture()["defaultConfig"]);
    let no_output: Inputs = [("in".into(), OutputValue::NoOutput)].into();
    assert_eq!(
        node.execute(context(), &no_output).unwrap()["out"],
        OutputValue::NoOutput
    );
    let missing: Inputs = [].into();
    assert_eq!(
        node.execute(context(), &missing).unwrap_err().code,
        "missing-input"
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
fn plain_execute_reports_not_prepared_and_configs_are_strict() {
    let config = fixture()["defaultConfig"].clone();
    let inputs: Inputs = [("in".into(), OutputValue::Data(Arc::new(text("かんじ"))))].into();
    assert_eq!(
        TextSkkDictionary
            .execute(context(), &inputs, &config)
            .unwrap_err()
            .code,
        "dictionary-not-prepared"
    );
    for invalid in fixture()["invalidConfigs"].as_array().unwrap() {
        assert_eq!(
            TextSkkDictionary.validate_config(invalid).unwrap_err().code,
            "invalid-config",
            "{invalid}"
        );
    }
}
#[test]
fn prepare_failures_keep_their_error_codes() {
    let config = fixture()["defaultConfig"].clone();
    let mut empty = MemoryResources(Default::default());
    assert_eq!(
        TextSkkDictionary
            .prepare(&config, &mut empty)
            .err()
            .expect("prepare must fail")
            .code,
        "dictionary-read"
    );
    assert_eq!(
        TextSkkDictionary
            .prepare(&config, &mut resources(b"bad line without slashes\n"))
            .err()
            .expect("prepare must fail")
            .code,
        "dictionary-format"
    );
    assert_eq!(
        TextSkkDictionary
            .prepare(&config, &mut resources("x /a".as_bytes()))
            .err()
            .expect("prepare must fail")
            .code,
        "dictionary-format"
    );
    assert_eq!(
        TextSkkDictionary
            .prepare(&config, &mut resources(&[0xFF, 0xFE, 0x61]))
            .err()
            .expect("prepare must fail")
            .code,
        "dictionary-read"
    );
}
