//! Build-time helpers for a custom guest; these types confer no Host authority.
use super::{API_VERSION, MAX_MESSAGE_BYTES, Operation, Request, Snapshots, inputs, outputs};
use crate::node::{
    CoreError, InputMode, Inputs, NodeDefinition, OutputValue, Outputs, PrepareResources,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, Write};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}
/// A request for the Host to consider, not permission to make that request.
#[derive(Debug, Serialize)]
pub struct NetworkCall {
    pub binding: String,
    pub body: Vec<u8>,
    pub continuation: Value,
}
pub enum ServiceStep {
    Outputs(Outputs),
    Network(NetworkCall),
}
pub struct ServiceContext<'a> {
    pub mode: Option<InputMode>,
    pub resources: &'a mut dyn PrepareResources,
}
/// Rust authoring interface inside a guest. The external boundary is JSON/Wasm.
/// The factory is recreated in each fresh Worker; persist state in continuation.
pub trait ServiceNode {
    fn definition(&self) -> NodeDefinition;
    fn validate_config(&self, config: &Value) -> Result<(), CoreError>;
    fn prepare(
        &self,
        _config: &Value,
        _resources: &mut dyn PrepareResources,
    ) -> Result<bool, CoreError> {
        Ok(false)
    }
    fn execute(
        &self,
        config: &Value,
        inputs: &Inputs,
        context: ServiceContext<'_>,
    ) -> Result<ServiceStep, CoreError>;
    fn resume(
        &self,
        _config: &Value,
        _continuation: Value,
        _response: HttpResponse,
        _context: ServiceContext<'_>,
    ) -> Result<ServiceStep, CoreError> {
        Err(CoreError::new(
            "guest-operation",
            "service does not support resume",
        ))
    }
}
enum Reply {
    Data(Value),
    Network(NetworkCall),
}
fn step(definition: &NodeDefinition, step: ServiceStep) -> Result<Reply, CoreError> {
    match step {
        ServiceStep::Outputs(values) => outputs(definition, values).map(Reply::Data),
        ServiceStep::Network(call) => {
            let binding = &call.binding;
            if binding.is_empty()
                || binding.len() > 128
                || binding.starts_with('.')
                || binding.ends_with('.')
                || !binding
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(&c))
            {
                return Err(CoreError::new(
                    "guest-network",
                    "invalid logical network binding",
                ));
            }
            Ok(Reply::Network(call))
        }
    }
}
fn dispatch(node: &dyn ServiceNode, request: Request) -> Result<Reply, CoreError> {
    if request.api_version != API_VERSION {
        return Err(CoreError::new("guest-version", "unsupported guest API"));
    }
    let definition = node.definition();
    if request.node.kind != definition.kind || request.node.version != definition.version {
        return Err(CoreError::new(
            "unknown-kind",
            "service identity differs from request",
        ));
    }
    match request.operation {
        Operation::Describe {} => Ok(Reply::Data(
            serde_json::to_value(definition)
                .map_err(|_| CoreError::new("guest-json", "definition serialization failed"))?,
        )),
        Operation::Validate { config } => {
            node.validate_config(&config)?;
            Ok(Reply::Data(json!({"valid":true})))
        }
        Operation::Prepare { config, resources } => {
            node.validate_config(&config)?;
            let prepared = node.prepare(&config, &mut Snapshots(resources))?;
            Ok(Reply::Data(json!({"valid":true,"prepared":prepared})))
        }
        Operation::Execute {
            config,
            inputs: supplied,
            mode,
            resources,
        } => {
            node.validate_config(&config)?;
            let inputs = inputs(&definition, supplied)?;
            let mut resources = Snapshots(resources);
            node.prepare(&config, &mut resources)?;
            if definition.inputs.iter().any(|p| {
                p.required && matches!(inputs.get(&p.port.id), Some(OutputValue::NoOutput))
            }) {
                return outputs(
                    &definition,
                    definition
                        .outputs
                        .iter()
                        .map(|p| (p.id.clone(), OutputValue::NoOutput))
                        .collect(),
                )
                .map(Reply::Data);
            }
            step(
                &definition,
                node.execute(
                    &config,
                    &inputs,
                    ServiceContext {
                        mode,
                        resources: &mut resources,
                    },
                )?,
            )
        }
        Operation::Resume {
            config,
            continuation,
            response,
            resources,
        } => {
            node.validate_config(&config)?;
            if !(100..=599).contains(&response.status) {
                return Err(CoreError::new(
                    "guest-response",
                    "invalid HTTP response status",
                ));
            }
            let mut resources = Snapshots(resources);
            node.prepare(&config, &mut resources)?;
            step(
                &definition,
                node.resume(
                    &config,
                    continuation,
                    response,
                    ServiceContext {
                        mode: None,
                        resources: &mut resources,
                    },
                )?,
            )
        }
    }
}
struct Bounded(Vec<u8>);
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_MESSAGE_BYTES.saturating_sub(self.0.len()) {
            return Err(io::ErrorKind::WriteZero.into());
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn encode(value: &impl Serialize) -> Vec<u8> {
    let mut output = Bounded(Vec::new());
    if serde_json::to_writer(&mut output, value).is_err() {
        return super::failure(CoreError::new(
            "guest-limit",
            "guest response exceeds limit",
        ));
    }
    output.0
}
/// One bounded request, no OS IO, native paths or authority parameters.
pub fn handle(node: &dyn ServiceNode, bytes: &[u8]) -> Vec<u8> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return super::failure(CoreError::new("guest-limit", "guest message exceeds limit"));
    }
    let result = serde_json::from_slice::<Request>(bytes)
        .map_err(|_| CoreError::new("guest-json", "invalid guest request"))
        .and_then(|request| dispatch(node, request));
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Data {
        api_version: u32,
        ok: bool,
        data: Value,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Call {
        api_version: u32,
        ok: bool,
        call: NetworkCall,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Failure {
        api_version: u32,
        ok: bool,
        error: CoreError,
    }
    match result {
        Ok(Reply::Data(data)) => encode(&Data {
            api_version: API_VERSION,
            ok: true,
            data,
        }),
        Ok(Reply::Network(call)) => encode(&Call {
            api_version: API_VERSION,
            ok: true,
            call,
        }),
        Err(error) => encode(&Failure {
            api_version: API_VERSION,
            ok: false,
            error,
        }),
    }
}
#[doc(hidden)]
pub const CUSTOM_GUEST_ENABLED: bool = cfg!(feature = "custom-guest");
/// Export a custom ServiceNode guest ABI. Enable the dependency's custom-guest
/// feature so the official processing module does not export conflicting names.
#[macro_export]
macro_rules! export_service_guest {
    ($factory:expr) => {
        const _: () = assert!(
            $crate::guest::service::CUSTOM_GUEST_ENABLED,
            "enable diy-ime2-nodes/custom-guest for a custom guest ABI"
        );
        #[cfg(target_arch = "wasm32")]
        fn diyime2_service_guest_factory() -> impl $crate::guest::service::ServiceNode {
            $factory
        }
        #[cfg(target_arch = "wasm32")]
        mod diyime2_service_guest_abi {
            use std::sync::Mutex;
            static REQUEST: Mutex<Option<Box<[u8]>>> = Mutex::new(None);
            static RESPONSE: Mutex<Option<Box<[u8]>>> = Mutex::new(None);
            #[unsafe(no_mangle)]
            pub extern "C" fn diyime_api_version() -> u32 {
                $crate::guest::API_VERSION
            }
            #[unsafe(no_mangle)]
            pub extern "C" fn diyime_alloc(length: u32) -> u32 {
                let length = length as usize;
                if length == 0 || length > $crate::guest::MAX_MESSAGE_BYTES {
                    return 0;
                }
                let bytes = vec![0; length].into_boxed_slice();
                let pointer = bytes.as_ptr() as u32;
                *REQUEST.lock().unwrap() = Some(bytes);
                pointer
            }
            #[unsafe(no_mangle)]
            pub extern "C" fn diyime_call(pointer: u32, length: u32) -> u64 {
                let bytes = {
                    let mut pending = REQUEST.lock().unwrap();
                    if !pending
                        .as_ref()
                        .is_some_and(|b| b.as_ptr() as u32 == pointer && b.len() == length as usize)
                    {
                        return 0;
                    }
                    pending.take().unwrap()
                };
                let node = super::diyime2_service_guest_factory();
                let result = $crate::guest::service::handle(&node, &bytes).into_boxed_slice();
                let descriptor = ((result.len() as u64) << 32) | result.as_ptr() as u64;
                *RESPONSE.lock().unwrap() = Some(result);
                descriptor
            }
            #[unsafe(no_mangle)]
            pub extern "C" fn diyime_release_reply() {
                *RESPONSE.lock().unwrap() = None;
            }
        }
    };
}
