#[path = "support/mod.rs"]
mod test_host;
use crate::test_host::{
    Candidate, ChoiceOption, Packet,
    builtins::register_builtin_nodes,
    graph::{Edge, Endpoint, ExecutionPlan, Graph, NodeInstance},
    node::{
        CoreError, InputMode, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port, Registry,
        RootConstraint, RunContext, RuntimeInput,
    },
    runtime::{Engine, NodeStatus},
};
use serde_json::Value;
use std::sync::Arc;

fn registry() -> Registry {
    let mut registry = Registry::new();
    register_builtin_nodes(&mut registry).unwrap();
    registry
}
fn node(id: &str, kind: &str) -> NodeInstance {
    NodeInstance {
        id: id.into(),
        kind: kind.into(),
        version: 1,
        config: None,
    }
}
fn edge(from: &str, to: &str) -> Edge {
    Edge {
        from: Endpoint::new(from, "out"),
        to: Endpoint::new(to, "in"),
    }
}

#[test]
fn experimental_catalog_preserves_legacy_kinds_and_registers_every_verified_node() {
    let registry = registry();
    for kind in [
        "input.text",
        "text.romaji-to-hiragana",
        "text.select-candidates",
        "text.stringify",
        "demo.packet",
        "e2e.input",
    ] {
        assert!(registry.definitions().any(|d| d.kind == kind));
    }
    let nodes: Value =
        serde_json::from_str(include_str!("../examples/experimental-catalog.json")).unwrap();
    let mut kinds = std::collections::BTreeSet::new();
    for node in nodes.as_array().unwrap() {
        let kind = node["kind"].as_str().unwrap();
        assert!(kinds.insert(kind));
        let definition = registry
            .definitions()
            .find(|d| d.kind == kind)
            .expect("catalog node must be registered");
        assert_eq!(definition.version, 1);
        assert!(kind.starts_with("experimental."));
    }
    assert_eq!(
        kinds.len(),
        crate::test_host::nodes::experimental::NODE_COUNT
    );
    assert_eq!(registry.definitions().count(), 17 + kinds.len());
}

#[test]
fn runtime_text_sample_executes_unicode_and_empty_text_without_changing_input() {
    let graph =
        serde_json::from_str::<Graph>(include_str!("../examples/graphs/experimental-text.json"))
            .unwrap();
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry()).unwrap());
    for (index, (text, expected)) in [("  kana  ", "カナ"), ("　😀\t", "😀"), ("", "")]
        .into_iter()
        .enumerate()
    {
        let input = RuntimeInput {
            revision: index as u64 + 1,
            text: text.into(),
            mode: InputMode::Finalize,
        };
        let report = engine.run_with_input(&input).unwrap();
        assert!(report.succeeded());
        assert_eq!(report.input_revision, Some(input.revision));
        assert_eq!(input.text, text);
        assert_eq!(
            report.packet("text", "out").unwrap().root,
            Candidate::Text {
                text: expected.into(),
                meta: None
            }
        );
    }
    let missing_input = engine.run().unwrap();
    assert!(!missing_input.succeeded());
    assert_eq!(missing_input.nodes["trim"].status, NodeStatus::Blocked);
}

struct ChoiceSource;
impl Node for ChoiceSource {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition::new(
            "test.experimental-choice-source",
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
        let options = [("あ", 1.0), ("あ", 3.0), ("い", 2.0)]
            .into_iter()
            .map(|(text, score)| ChoiceOption {
                value: Candidate::Text {
                    text: text.into(),
                    meta: None,
                },
                score: Some(score),
            })
            .collect();
        Ok([(
            "out".into(),
            OutputValue::Data(Arc::new(Packet::new(Candidate::Choice { options }))),
        )]
        .into())
    }
}

#[test]
fn registered_deduplication_keeps_first_score_before_existing_candidate_selection() {
    let mut registry = registry();
    registry.register(ChoiceSource).unwrap();
    let graph = Graph {
        version: 1,
        nodes: vec![
            node("source", "test.experimental-choice-source"),
            node("dedup", "experimental.choice-deduplicate"),
            node("select", "text.select-candidates"),
            node("text", "text.stringify"),
        ],
        edges: vec![
            edge("source", "dedup"),
            edge("dedup", "select"),
            edge("select", "text"),
        ],
    };
    let mut engine = Engine::new(ExecutionPlan::compile(&graph, &registry).unwrap());
    let report = engine.run().unwrap();
    assert!(report.succeeded());
    assert_eq!(
        report.packet("text", "out").unwrap().root,
        Candidate::Text {
            text: "い".into(),
            meta: None
        }
    );
}
