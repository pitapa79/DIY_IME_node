#[path = "support/service.rs"]
mod support;
use diy_ime2_nodes::guest::{self, service};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
fn request(operation: Value) -> Value {
    json!({"apiVersion":1,"node":{"kind":"fixture.service","version":1},"operation":operation})
}
fn text() -> Value {
    json!({"state":"data","packet":{"version":1,"root":{"type":"text","text":"private-input🐈","meta":{"reading":"かな"}}}})
}
fn execute(mode: &str) -> Value {
    request(
        json!({"method":"execute","config":{"mode":mode},"inputs":{"in":text()},"mode":"compose"}),
    )
}
fn invoke(node: &support::Canary, request: Value) -> Value {
    serde_json::from_slice(&service::handle(
        node,
        &serde_json::to_vec(&request).unwrap(),
    ))
    .unwrap()
}
fn resume(call: &Value, mode: &str) -> Value {
    request(
        json!({"method":"resume","config":{"mode":mode},"continuation":call["call"]["continuation"],"response":{"status":201,"body":[0,255,65]}}),
    )
}
#[test]
fn actual_authoring_callbacks_preserve_metadata_and_explicit_multihop_continuation() {
    let node = support::Canary::default();
    let pure = invoke(&node, execute("pure"));
    assert_eq!(pure["data"]["out"], text());
    let call = invoke(&node, execute("twice"));
    assert_eq!(call["call"]["body"], json!("private-input🐈".as_bytes()));
    assert_eq!(call["call"]["continuation"]["mode"], "compose");
    assert!(call.get("data").is_none());
    let second = invoke(&node, resume(&call, "twice"));
    assert_eq!(second["call"]["body"], json!([0, 255, 65]));
    let final_reply = invoke(&node, resume(&second, "twice"));
    let result: Value = serde_json::from_str(
        final_reply["data"]["out"]["packet"]["root"]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["status"], 201);
    assert_eq!(result["body"], json!([0, 255, 65]));
    assert_eq!(result["mode"], Value::Null);
    assert_eq!(node.resumed.load(Ordering::Relaxed), 2);
}
#[test]
fn required_no_output_skips_service_callback_but_keeps_empty_text_distinct() {
    let node = support::Canary::default();
    let mut no = execute("network");
    no["operation"]["inputs"]["in"] = json!({"state":"noOutput"});
    assert_eq!(invoke(&node, no)["data"]["out"]["state"], "noOutput");
    assert_eq!(node.executed.load(Ordering::Relaxed), 0);
    let mut empty = execute("network");
    empty["operation"]["inputs"]["in"]["packet"]["root"]["text"] = "".into();
    assert_eq!(invoke(&node, empty)["call"]["body"], json!([]));
    assert_eq!(node.executed.load(Ordering::Relaxed), 1);
}
#[test]
fn only_explicit_snapshots_are_available_each_prepare_execute_and_resume() {
    let node = support::Canary::default();
    let config = json!({"mode":"network","resource":"dictionary.tsv"});
    let resource = json!({"dictionary.tsv":b"private-snapshot".to_vec()});
    let preparation = request(json!({"method":"prepare","config":config,"resources":resource}));
    assert_eq!(
        invoke(&node, preparation)["data"],
        json!({"valid":true,"prepared":true})
    );
    let mut missing = execute("network");
    missing["operation"]["config"] = config.clone();
    assert_eq!(
        invoke(&node, missing.clone())["error"]["code"],
        "resource-unavailable"
    );
    missing["operation"]["resources"] = resource.clone();
    let call = invoke(&node, missing);
    assert_eq!(call["call"]["continuation"]["resource"], "private-snapshot");
    let mut resumed = resume(&call, "network");
    resumed["operation"]["config"] = config;
    assert_eq!(
        invoke(&node, resumed.clone())["error"]["code"],
        "resource-unavailable"
    );
    resumed["operation"]["resources"] = resource;
    assert_eq!(invoke(&node, resumed)["ok"], true);
}
#[test]
fn strict_resume_rejects_forged_host_fields_missing_state_and_invalid_responses_without_echo() {
    let node = support::Canary::default();
    let call = invoke(&node, execute("network"));
    let good = resume(&call, "network");
    for field in [
        "inputs",
        "mode",
        "runKey",
        "permissions",
        "secret",
        "url",
        "headers",
    ] {
        let mut bad = good.clone();
        bad["operation"][field] = json!("private-request-canary");
        let reply = invoke(&node, bad);
        assert_eq!(reply["error"]["code"], "guest-json");
        assert!(!reply.to_string().contains("private-request-canary"));
    }
    for status in [0, 99, 600, 65535] {
        let mut bad = good.clone();
        bad["operation"]["response"]["status"] = status.into();
        assert_eq!(invoke(&node, bad)["error"]["code"], "guest-response");
    }
    for field in ["continuation", "response", "config"] {
        let mut bad = good.clone();
        bad["operation"].as_object_mut().unwrap().remove(field);
        assert_eq!(invoke(&node, bad)["error"]["code"], "guest-json");
    }
    let mut unknown = good.clone();
    unknown["operation"]["response"]["headers"] = json!({"private":"value"});
    assert_eq!(invoke(&node, unknown)["error"]["code"], "guest-json");
    let mut null = good;
    null["operation"]["continuation"] = Value::Null;
    assert_eq!(invoke(&node, null)["ok"], true);
}
#[test]
fn invalid_identity_config_port_and_outputs_cannot_form_a_successful_call() {
    let node = support::Canary::default();
    let mut cases = Vec::new();
    let mut version = execute("network");
    version["apiVersion"] = 2.into();
    cases.push((version, "guest-version"));
    let mut kind = execute("network");
    kind["node"]["kind"] = "other".into();
    cases.push((kind, "unknown-kind"));
    let mut cfg = execute("network");
    cfg["operation"]["config"]["extra"] = true.into();
    cases.push((cfg, "invalid-config"));
    let mut input = execute("network");
    input["operation"]["inputs"]["other"] = text();
    cases.push((input, "guest-input"));
    cases.push((execute("bad-binding"), "guest-network"));
    cases.push((execute("bad-output"), "guest-output"));
    for (request, code) in cases {
        let reply = invoke(&node, request);
        assert_eq!(reply["error"]["code"], code);
        assert!(reply.get("call").is_none());
    }
    assert_eq!(invoke(&node, execute("network"))["ok"], true);
}
#[test]
fn request_response_and_authored_error_limits_return_complete_small_failure_envelopes() {
    let node = support::Canary::default();
    let oversized = vec![b' '; guest::MAX_MESSAGE_BYTES + 1];
    let reply: Value = serde_json::from_slice(&service::handle(&node, &oversized)).unwrap();
    assert_eq!(reply["error"]["code"], "guest-limit");
    for mode in ["large", "large-error"] {
        let reply = invoke(&node, execute(mode));
        assert_eq!(reply["error"]["code"], "guest-limit");
        assert!(reply.to_string().len() < 1024);
    }
}
#[test]
fn pure_official_dispatcher_never_executes_a_resume_or_exposes_service_nodes() {
    let mut request = request(
        json!({"method":"resume","config":{},"continuation":null,"response":{"status":200,"body":[]}}),
    );
    request["node"]["kind"] = "experimental.packet-passthrough".into();
    let reply: Value =
        serde_json::from_slice(&guest::handle(&serde_json::to_vec(&request).unwrap())).unwrap();
    assert_eq!(reply["error"]["code"], "guest-operation");
    let reply: Value = serde_json::from_slice(&guest::handle(
        &serde_json::to_vec(&execute("network")).unwrap(),
    ))
    .unwrap();
    assert_eq!(reply["error"]["code"], "unknown-kind");
}
