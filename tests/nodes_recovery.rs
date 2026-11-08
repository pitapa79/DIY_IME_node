#[path = "support/mod.rs"]
mod test_host;
// Load the isolated implementation without editing the standard module/Registry.
pub use crate::test_host::{error_input, node};
#[path = "../src/nodes/recovery/mod.rs"]
mod recovery;
use crate::test_host::{
    Packet,
    error_input::{ErrorAwareNode, ErrorAwareValue as V},
    graph::{Endpoint, ExecutionPlan, Graph},
    node::{
        CoreError, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry, RunContext,
    },
    runtime::{Engine, ResolvedValue},
};
use recovery::{KIND, RecoverOriginal};
use serde_json::{Value, json};
use std::sync::Arc;
struct Source {
    kind: String,
    value: Option<Arc<Packet>>,
    fail: bool,
    language: &'static str,
}
impl Node for Source {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            &self.kind,
            1,
            vec![],
            vec![Port::packet("out").with_language(self.language)],
        )
    }
    fn execute(&self, _: RunContext<'_>, _: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        if self.fail {
            return Err(CoreError::new("test-failure", &self.kind));
        }
        Ok([(
            "out".into(),
            self.value
                .as_ref()
                .map(|p| OutputValue::Data(p.clone()))
                .unwrap_or(OutputValue::NoOutput),
        )]
        .into())
    }
}
fn compile(
    case: &Value,
    config: Option<Value>,
) -> (Engine, std::collections::BTreeMap<String, Arc<Packet>>) {
    let mut registry = Registry::new();
    registry.register_error_aware(RecoverOriginal).unwrap();
    let mut nodes = vec![];
    let mut edges = vec![];
    let mut packets = std::collections::BTreeMap::new();
    for id in ["primary", "original"] {
        let input = &case["inputs"][id];
        if input["status"] == "missing" {
            continue;
        }
        let packet = (input["status"] == "data")
            .then(|| Arc::new(Packet::from_json(&input["packet"].to_string()).unwrap()));
        if let Some(packet) = &packet {
            packets.insert(id.into(), packet.clone());
        }
        registry
            .register(Source {
                kind: id.into(),
                value: packet,
                fail: input["status"] == "error",
                language: "ja",
            })
            .unwrap();
        nodes.push(json!({"id":id,"kind":id,"version":1}));
        edges.push(json!({"from":{"node":id,"port":"out"},"to":{"node":"recover","port":id}}));
    }
    let mut recover = json!({"id":"recover","kind":KIND,"version":1});
    if let Some(config) = config {
        recover["config"] = config;
    }
    nodes.push(recover);
    let graph =
        Graph::from_json(&json!({"version":1,"nodes":nodes,"edges":edges}).to_string()).unwrap();
    (
        Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap()),
        packets,
    )
}
fn fixtures() -> Value {
    serde_json::from_str(include_str!("contracts/recovery.json")).unwrap()
}
#[test]
fn all_nineteen_formal_cases_run_the_actual_node_and_preserve_packet_identity() {
    let fixtures = fixtures();
    assert_eq!(fixtures["cases"].as_array().unwrap().len(), 19);
    for case in fixtures["cases"].as_array().unwrap() {
        let (mut engine, packets) = compile(case, Some(case["config"].clone()));
        let report = engine
            .run_with_targets(&[Endpoint::new("recover", "out")], None)
            .unwrap();
        let node = &report.report.nodes["recover"];
        let expected = &case["expected"];
        match expected["status"].as_str().unwrap() {
            "data" => assert!(
                Arc::ptr_eq(
                    report.report.packet("recover", "out").unwrap(),
                    &packets[expected["sameAs"].as_str().unwrap()]
                ),
                "{}",
                case["id"]
            ),
            "no-output" => assert!(matches!(node.outputs["out"], ResolvedValue::NoOutput)),
            "error" => assert_eq!(
                node.error_outputs["out"].causes().len() as u64,
                expected["causes"].as_u64().unwrap()
            ),
            _ => panic!("unrecognized fixture"),
        }
        assert_eq!(
            json!(report.delivery),
            expected["delivery"],
            "{}",
            case["id"]
        );
        assert_eq!(
            json!(report.report.succeeded()),
            expected["legacySucceeded"],
            "{}",
            case["id"]
        );
        assert_eq!(
            node.recoveries.len() as u64,
            expected["recoveries"].as_u64().unwrap(),
            "{}",
            case["id"]
        );
        for (id, packet) in packets {
            assert!(Arc::ptr_eq(
                report.report.packet(&id, "out").unwrap(),
                &packet
            ));
        }
    }
}
#[test]
fn omitted_configuration_uses_false_and_runs_do_not_keep_previous_selection() {
    let cases = fixtures();
    let case = &cases["cases"][2];
    let (mut engine, _) = compile(case, None);
    for _ in 0..3 {
        let report = engine
            .run_with_targets(&[Endpoint::new("recover", "out")], None)
            .unwrap();
        assert_eq!(json!(report.delivery), "no-output");
    }
    let (mut fallback, _) = compile(case, Some(json!({"onNoOutput":true})));
    assert_eq!(
        json!(
            fallback
                .run_with_targets(&[Endpoint::new("recover", "out")], None)
                .unwrap()
                .delivery
        ),
        "complete"
    );
    assert_eq!(
        json!(
            engine
                .run_with_targets(&[Endpoint::new("recover", "out")], None)
                .unwrap()
                .delivery
        ),
        "no-output"
    );
}
#[test]
fn complete_configuration_is_required_and_language_mismatch_is_not_disguised() {
    for config in [
        json!({}),
        Value::Null,
        json!([]),
        json!(true),
        json!({"onNoOutput":0}),
        json!({"onNoOutput":null}),
        json!({"onNoOutput":false,"extra":true}),
    ] {
        assert_eq!(
            RecoverOriginal.validate_config(&config).unwrap_err().code,
            "invalid-config"
        );
        let mut registry = Registry::new();
        registry.register_error_aware(RecoverOriginal).unwrap();
        let graph=Graph::from_json(&json!({"version":1,"nodes":[{"id":"r","kind":KIND,"version":1,"config":config}],"edges":[]}).to_string());
        match graph {
            Ok(graph) => assert_eq!(
                ExecutionPlan::compile(&graph, &registry)
                    .err()
                    .unwrap()
                    .code,
                "invalid-config"
            ),
            Err(error) => {
                assert!(config.is_null());
                assert_eq!(error.code, "graph-json");
            }
        }
    }
    let mut registry = Registry::new();
    registry.register_error_aware(RecoverOriginal).unwrap();
    registry
        .register(Source {
            kind: "source".into(),
            value: None,
            fail: false,
            language: "ja-Latn",
        })
        .unwrap();
    let graph=Graph::from_json(&json!({"version":1,"nodes":[{"id":"s","kind":"source","version":1},{"id":"r","kind":KIND,"version":1}],"edges":[{"from":{"node":"s","port":"out"},"to":{"node":"r","port":"original"}}]}).to_string()).unwrap();
    assert!(ExecutionPlan::compile(&graph, &registry).is_err());
}
#[test]
fn direct_call_handles_optional_absence_but_rejects_invalid_packets_even_on_unselected_input() {
    let config = json!({"onNoOutput":false});
    let context = RunContext {
        run_id: 1,
        plan_version: 1,
        node_id: "recover",
        input: None,
    };
    assert!(matches!(
        RecoverOriginal
            .execute(context, &Default::default(), &config)
            .unwrap()
            .outputs["out"],
        V::NoOutput
    ));
    let good =
        Arc::new(Packet::from_json(r#"{"version":1,"root":{"type":"text","text":"ok"}}"#).unwrap());
    let mut bad = (*good).clone();
    bad.version = 2;
    let inputs = [
        ("primary".into(), V::Data(good)),
        ("original".into(), V::Data(Arc::new(bad))),
    ]
    .into();
    assert_eq!(
        RecoverOriginal
            .execute(context, &inputs, &config)
            .unwrap_err()
            .code,
        "invalid-packet"
    );
}

#[test]
fn documented_success_and_failure_graphs_run_against_the_isolated_registry() {
    for (source, selected, delivery) in [
        (
            include_str!("../examples/nodes/recovery/primary-fails.json"),
            "original",
            "recovered",
        ),
        (
            include_str!("../examples/nodes/recovery/primary-succeeds.json"),
            "primary",
            "complete",
        ),
    ] {
        let mut registry = Registry::new();
        registry.register_error_aware(RecoverOriginal).unwrap();
        for (kind, text, fail) in [
            ("example.primary-failure", "", true),
            ("example.primary-success", "主結果", false),
            ("example.original", "原文😀", false),
        ] {
            registry
                .register(Source {
                    kind: kind.into(),
                    value: Some(Arc::new(
                        Packet::from_json(
                            &json!({"version":1,"root":{"type":"text","text":text}}).to_string(),
                        )
                        .unwrap(),
                    )),
                    fail,
                    language: "ja",
                })
                .unwrap();
        }
        let graph = Graph::from_json(source).unwrap();
        let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
        let report = engine
            .run_with_targets(&[Endpoint::new("recover", "out")], None)
            .unwrap();
        assert_eq!(json!(report.delivery), delivery);
        assert!(Arc::ptr_eq(
            report.report.packet("recover", "out").unwrap(),
            report.report.packet(selected, "out").unwrap()
        ));
    }
}

#[test]
fn merged_causes_are_all_recovered_and_unselected_recovery_does_not_taint_primary_data() {
    for select_primary in [false, true] {
        let mut registry = Registry::new();
        registry.register_error_aware(RecoverOriginal).unwrap();
        for (kind, fail) in [
            ("a", true),
            ("b", true),
            ("original", false),
            ("plain", false),
        ] {
            registry
                .register(Source {
                    kind: kind.into(),
                    value: Some(Arc::new(
                        Packet::from_json(r#"{"version":1,"root":{"type":"text","text":"ok"}}"#)
                            .unwrap(),
                    )),
                    fail,
                    language: "ja",
                })
                .unwrap();
        }
        let mut nodes = vec![
            json!({"id":"a","kind":"a","version":1}),
            json!({"id":"b","kind":"b","version":1}),
            json!({"id":"original","kind":"original","version":1}),
            json!({"id":"merge","kind":KIND,"version":1}),
            json!({"id":"recover","kind":KIND,"version":1}),
        ];
        let mut edges = vec![];
        for (from, to, port) in [
            ("a", "merge", "primary"),
            ("b", "merge", "original"),
            ("merge", "recover", "primary"),
            ("original", "recover", "original"),
        ] {
            edges.push(json!({"from":{"node":from,"port":"out"},"to":{"node":to,"port":port}}));
        }
        let target = if select_primary {
            nodes.extend([
                json!({"id":"plain","kind":"plain","version":1}),
                json!({"id":"select","kind":KIND,"version":1}),
            ]);
            for (from, port) in [("plain", "primary"), ("recover", "original")] {
                edges.push(
                    json!({"from":{"node":from,"port":"out"},"to":{"node":"select","port":port}}),
                );
            }
            "select"
        } else {
            "recover"
        };
        let graph = Graph::from_json(&json!({"version":1,"nodes":nodes,"edges":edges}).to_string())
            .unwrap();
        let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
        let report = engine
            .run_with_targets(&[Endpoint::new(target, "out")], None)
            .unwrap();
        let recovery = &report.report.nodes["recover"].recoveries;
        assert_eq!(recovery.len(), 1);
        assert_eq!(recovery[0].causes.len(), 2);
        assert_eq!(
            recovery[0]
                .causes
                .iter()
                .map(|c| c.node_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(
            json!(report.delivery),
            if select_primary {
                "complete"
            } else {
                "recovered"
            }
        );
        assert!(!report.report.succeeded());
    }
}
