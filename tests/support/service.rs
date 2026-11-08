//! Synthetic SDK fixture, never part of the official registry.
use diy_ime2_nodes::{
    Candidate, Packet,
    guest::service::{HttpResponse, NetworkCall, ServiceContext, ServiceNode, ServiceStep},
    node::{
        CoreError, InputMode, InputPort, Inputs, NodeDefinition, OutputValue, Port,
        PrepareResources, RootConstraint,
    },
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
#[derive(Default)]
pub struct Canary {
    pub executed: AtomicUsize,
    pub resumed: AtomicUsize,
}
fn failure(code: &str) -> CoreError {
    CoreError::new(code, "synthetic service refused")
}
fn resource(config: &Value, resources: &mut dyn PrepareResources) -> Result<Value, CoreError> {
    if let Some(name) = config.get("resource").and_then(Value::as_str) {
        let bytes = resources.read(name, 128)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| failure("fixture-resource"))?;
        return Ok(json!(text));
    }
    Ok(Value::Null)
}
fn data(text: String) -> ServiceStep {
    ServiceStep::Outputs(BTreeMap::from([(
        "out".into(),
        OutputValue::Data(Arc::new(Packet::new(Candidate::Text { text, meta: None }))),
    )]))
}
impl ServiceNode for Canary {
    fn definition(&self) -> NodeDefinition {
        let mut definition = NodeDefinition::new(
            "fixture.service",
            1,
            vec![InputPort::required(
                Port::packet("in").with_root(RootConstraint::Text),
            )],
            vec![Port::packet("out").with_root(RootConstraint::Text)],
        );
        definition.config_schema = json!({"type":"object"});
        definition.default_config = json!({"mode":"network"});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        let mode = config.get("mode").and_then(Value::as_str);
        if !matches!(
            mode,
            Some(
                "network"
                    | "twice"
                    | "pure"
                    | "bad-binding"
                    | "bad-output"
                    | "large"
                    | "large-error"
            )
        ) || !config
            .as_object()
            .is_some_and(|o| o.keys().all(|key| key == "mode" || key == "resource"))
            || config
                .get("resource")
                .is_some_and(|v| v.as_str().is_none_or(|s| s.is_empty()))
        {
            return Err(failure("invalid-config"));
        }
        Ok(())
    }
    fn prepare(
        &self,
        config: &Value,
        resources: &mut dyn PrepareResources,
    ) -> Result<bool, CoreError> {
        resource(config, resources)?;
        Ok(config.get("resource").is_some())
    }
    fn execute(
        &self,
        config: &Value,
        inputs: &Inputs,
        context: ServiceContext<'_>,
    ) -> Result<ServiceStep, CoreError> {
        self.executed.fetch_add(1, Ordering::Relaxed);
        let Some(OutputValue::Data(packet)) = inputs.get("in") else {
            return Err(failure("fixture-input"));
        };
        let Candidate::Text { text, .. } = &packet.root else {
            return Err(failure("fixture-root"));
        };
        let mode = config["mode"].as_str().unwrap();
        if mode == "pure" {
            return Ok(ServiceStep::Outputs(BTreeMap::from([(
                "out".into(),
                OutputValue::Data(packet.clone()),
            )])));
        }
        if mode == "bad-output" {
            return Ok(ServiceStep::Outputs(BTreeMap::from([(
                "other".into(),
                OutputValue::NoOutput,
            )])));
        }
        if mode == "large-error" {
            return Err(CoreError::new(
                "fixture-error",
                "x".repeat(16 * 1024 * 1024),
            ));
        }
        let input_mode = context.mode.map(|m| match m {
            InputMode::Compose => "compose",
            InputMode::Finalize => "finalize",
        });
        Ok(ServiceStep::Network(NetworkCall {
            binding: if mode == "bad-binding" {
                "https://unselected".into()
            } else {
                "api".into()
            },
            body: if mode == "large" {
                vec![255; 4 * 1024 * 1024]
            } else {
                text.as_bytes().to_vec()
            },
            continuation: json!({"text":text,"mode":input_mode,"resource":resource(config,context.resources)?,"hop":1}),
        }))
    }
    fn resume(
        &self,
        config: &Value,
        continuation: Value,
        response: HttpResponse,
        context: ServiceContext<'_>,
    ) -> Result<ServiceStep, CoreError> {
        self.resumed.fetch_add(1, Ordering::Relaxed);
        if config["mode"] == "twice" && continuation["hop"] == 1 {
            return Ok(ServiceStep::Network(NetworkCall {
                binding: "api".into(),
                body: response.body,
                continuation: json!({"text":continuation["text"],"hop":2}),
            }));
        }
        if context.mode.is_some() {
            return Err(failure("fixture-mode"));
        }
        Ok(data(serde_json::to_string(&json!({"status":response.status,"body":response.body,"continuation":continuation,"mode":null,"resource":resource(config,context.resources)?})).unwrap()))
    }
}
