use diy_ime2_nodes::guest;
use serde_json::{Value, json};
fn invoke(request: Value) -> Value {
    serde_json::from_slice(&guest::handle(&serde_json::to_vec(&request).unwrap())).unwrap()
}
fn request(kind: &str, operation: Value) -> Value {
    json!({"apiVersion":1,"node":{"kind":kind,"version":1},"operation":operation})
}
fn text(text: &str) -> Value {
    json!({"state":"data","packet":{"version":1,"root":{"type":"text","text":text}}})
}
fn convert(input: Value, mode: &str) -> Value {
    request(
        "text.romaji-to-hiragana",
        json!({"method":"execute","config":{"overrides":[],"unknown":"preserve"},"inputs":{"in":input},"mode":mode}),
    )
}
#[test]
fn descriptions_and_default_validation_match_all_48_registered_definitions() {
    let mut registry = diy_ime2_nodes::node::Registry::new();
    diy_ime2_nodes::register_nodes(&mut registry).unwrap();
    for definition in registry.definitions() {
        let reply = invoke(request(&definition.kind, json!({"method":"describe"})));
        assert_eq!(reply["data"], serde_json::to_value(definition).unwrap());
        let valid = invoke(request(
            &definition.kind,
            json!({"method":"validate","config":definition.default_config}),
        ));
        assert_eq!(valid["ok"], true, "{}: {valid}", definition.kind);
    }
}
#[test]
fn compose_finalize_empty_and_no_output_remain_distinct() {
    let pending = invoke(convert(text("n"), "compose"));
    assert_eq!(pending["data"]["out"], text(""));
    assert_eq!(pending["data"]["pending"], text("n"));
    let final_text = invoke(convert(text("n"), "finalize"));
    assert_eq!(final_text["data"]["out"], text("ん"));
    assert_eq!(final_text["data"]["pending"], text(""));
    let none = invoke(convert(json!({"state":"noOutput"}), "compose"));
    assert_eq!(none["data"]["out"]["state"], "noOutput");
    assert_eq!(none["data"]["pending"]["state"], "noOutput");
    assert_eq!(
        invoke(convert(text(""), "compose"))["data"]["out"],
        text("")
    );
}
#[test]
fn unicode_and_metadata_are_not_replaced_with_host_input() {
    let mut input = text("か🐈");
    input["packet"]["root"]["meta"] = json!({"reading":"かな","sourceSpan":[0,2]});
    let reply = invoke(request(
        "experimental.packet-passthrough",
        json!({"method":"execute","config":{},"inputs":{"in":input}}),
    ));
    assert_eq!(reply["data"]["out"], input);
}
#[test]
fn host_identity_permissions_and_raw_input_fields_are_rejected() {
    for field in [
        "session",
        "runKey",
        "permissions",
        "trusted",
        "input",
        "text",
    ] {
        let mut value = convert(text("kya"), "compose");
        value[field] = json!("private-canary-never-echo");
        let reply = invoke(value);
        assert_eq!(reply["error"]["code"], "guest-json");
        assert!(!reply.to_string().contains("private-canary-never-echo"));
    }
}
#[test]
fn malformed_versions_kinds_ports_packets_and_configs_fail_then_normal_execution_recovers() {
    let mut cases = Vec::new();
    let mut version = convert(text("kya"), "compose");
    version["apiVersion"] = json!(2);
    cases.push((version, "guest-version"));
    cases.push((
        request("input.text", json!({"method":"describe"})),
        "unknown-kind",
    ));
    cases.push((
        request("experimental.paragraph-split", json!({"method":"describe"})),
        "unknown-kind",
    ));
    let mut port = convert(text("kya"), "compose");
    port["operation"]["inputs"]["secret"] = text("canary");
    cases.push((port, "guest-input"));
    let mut missing = convert(text("kya"), "compose");
    missing["operation"]["inputs"] = json!({});
    cases.push((missing, "missing-input"));
    let mut packet = convert(text("kya"), "compose");
    packet["operation"]["inputs"]["in"]["packet"]["version"] = json!(2);
    cases.push((packet, "invalid-packet"));
    let mut config = convert(text("kya"), "compose");
    config["operation"]["config"] = json!({"trusted":true});
    cases.push((config, "invalid-config"));
    for (request, code) in cases {
        assert_eq!(invoke(request)["error"]["code"], code);
        assert_eq!(
            invoke(convert(text("kya"), "compose"))["data"]["out"],
            text("きゃ")
        );
    }
}
#[test]
fn dictionary_reads_only_the_explicit_snapshot_and_keeps_node_errors() {
    let mut request = request(
        "text.dictionary-exact",
        json!({"method":"execute","config":{"path":"dictionary.tsv"},"inputs":{"in":text("か")}}),
    );
    assert_eq!(invoke(request.clone())["error"]["code"], "dictionary-path");
    request["operation"]["resources"] =
        json!({"dictionary.tsv":"reading\tsurface\tweight\nか\t蚊\t7\n".as_bytes()});
    let reply = invoke(request);
    assert_eq!(reply["ok"], true);
    assert_eq!(
        reply["data"]["out"]["packet"]["root"]["options"][0]["value"]["text"],
        "蚊"
    );
}
#[test]
fn byte_limit_and_invalid_json_are_bounded_without_echoing_input() {
    let error: Value =
        serde_json::from_slice(&guest::handle(&vec![b'x'; guest::MAX_MESSAGE_BYTES + 1])).unwrap();
    assert_eq!(error["error"]["code"], "guest-limit");
    let error: Value =
        serde_json::from_slice(&guest::handle(b"private-canary-invalid-json")).unwrap();
    assert_eq!(error["error"]["code"], "guest-json");
    assert!(!error.to_string().contains("private-canary"));
}

#[test]
fn no_output_rejects_every_unknown_field_without_echoing_or_poisoning_normal_calls() {
    for field in ["trusted", "permissions", "packet", "extra"] {
        let mut value = json!({"state":"noOutput"});
        value[field] = json!("private-no-output-canary");
        let reply = invoke(convert(value, "compose"));
        assert_eq!(reply["error"]["code"], "guest-json", "{field}");
        assert!(!reply.to_string().contains("private-no-output-canary"));
        assert_eq!(
            invoke(convert(json!({"state":"noOutput"}), "compose"))["data"]["out"]["state"],
            "noOutput"
        );
        assert_eq!(
            invoke(convert(text("kya"), "compose"))["data"]["out"],
            text("きゃ")
        );
    }
}

#[test]
fn prepare_validates_explicit_snapshots_without_inputs_or_retained_state() {
    let dictionary = "reading\tsurface\tweight\nか\t蚊\t7\n".as_bytes();
    let prepare = request(
        "text.dictionary-exact",
        json!({"method":"prepare","config":{"path":"dictionary.tsv"},"resources":{"dictionary.tsv":dictionary}}),
    );
    let result = invoke(prepare.clone());
    assert_eq!(result["data"], json!({"valid":true,"prepared":true}));
    assert!(!result.to_string().contains("蚊"));
    let execution = request(
        "text.dictionary-exact",
        json!({"method":"execute","config":{"path":"dictionary.tsv"},"inputs":{"in":text("か")}}),
    );
    assert_eq!(invoke(execution)["error"]["code"], "dictionary-path");
    assert_eq!(invoke(prepare)["data"]["prepared"], true);
    let pure = invoke(request(
        "text.romaji-to-hiragana",
        json!({"method":"prepare","config":{"overrides":[],"unknown":"preserve"},"resources":{}}),
    ));
    assert_eq!(pure["data"], json!({"valid":true,"prepared":false}));
}
#[test]
fn prepare_refuses_missing_malformed_and_oversized_snapshots_before_execution() {
    for (resources, code) in [
        (json!({}), "dictionary-path"),
        (json!({"dictionary.tsv":[255]}), "dictionary-encoding"),
        (
            json!({"dictionary.tsv":b"private-dictionary-canary"}),
            "dictionary-format",
        ),
        (
            json!({"dictionary.tsv":vec![b'x';1024*1024+1]}),
            "dictionary-limit",
        ),
    ] {
        let result = invoke(request(
            "text.dictionary-exact",
            json!({"method":"prepare","config":{"path":"dictionary.tsv"},"resources":resources}),
        ));
        assert_eq!(result["error"]["code"], code);
        assert!(!result.to_string().contains("private-dictionary-canary"));
    }
}
#[test]
fn describe_and_prepare_reject_unknown_authority_input_or_null_resource_fields() {
    for method in ["describe", "prepare"] {
        for field in [
            "trusted",
            "permissions",
            "inputs",
            "input",
            "mode",
            "runKey",
        ] {
            let mut operation = if method == "describe" {
                json!({"method":"describe"})
            } else {
                json!({"method":"prepare","config":{"overrides":[],"unknown":"preserve"},"resources":{}})
            };
            operation[field] = json!("private-prepare-canary");
            let result = invoke(request("text.romaji-to-hiragana", operation));
            assert_eq!(result["error"]["code"], "guest-json");
            assert!(!result.to_string().contains("private-prepare-canary"));
        }
    }
    for operation in [
        json!({"method":"prepare","config":{},"resources":null}),
        json!({"method":"prepare","config":{}}),
    ] {
        assert_eq!(
            invoke(request("text.dictionary-exact", operation))["error"]["code"],
            "guest-json"
        );
    }
    assert_eq!(
        invoke(request(
            "text.romaji-to-hiragana",
            json!({"method":"describe"})
        ))["ok"],
        true
    );
}
