//! Explicit companion operations, never the default all-Node guest dispatcher.
use crate::{
    guest::{self, WireValue},
    node::{CoreError, Node, OutputValue},
    nodes::dictionary_exact::{self, artifact},
};
use diy_ime2_wasm_resource_sdk::{Operation, Request, Resource};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn decode(bytes: &[u8]) -> Result<Request<Value, BTreeMap<String, WireValue>>, CoreError> {
    if bytes.len() > guest::MAX_MESSAGE_BYTES {
        return Err(CoreError::new("guest-limit", "guest message exceeds limit"));
    }
    let request: Request<Value, BTreeMap<String, WireValue>> = serde_json::from_slice(bytes)
        .map_err(|_| CoreError::new("guest-json", "invalid resource guest request"))?;
    request
        .validate()
        .map_err(|_| CoreError::new("guest-json", "invalid resource guest descriptor"))?;
    if request.node.kind != dictionary_exact::KIND || request.node.version != 1 {
        return Err(CoreError::new(
            "unknown-kind",
            "resource guest Node kind/version is not registered",
        ));
    }
    Ok(request)
}
fn reply(result: Result<Value, CoreError>) -> Vec<u8> {
    let value = match result {
        Ok(data) => json!({"apiVersion":1,"ok":true,"data":data}),
        Err(error) => json!({"apiVersion":1,"ok":false,"error":error}),
    };
    match serde_json::to_vec(&value) {
        Ok(bytes) if bytes.len()<=guest::MAX_MESSAGE_BYTES=>bytes,
        _=>serde_json::to_vec(&json!({"apiVersion":1,"ok":false,"error":{"code":"guest-limit","message":"guest response exceeds limit"}})).unwrap(),
    }
}
#[cfg(feature = "dictionary-artifact-producer")]
pub fn handle_prepare(bytes: &[u8]) -> Vec<u8> {
    use diy_ime2_wasm_resource_sdk::{ArtifactWriter, PreparationAcknowledgement};
    use std::io::Write;
    let result = (|| {
        let request = decode(bytes)?;
        let Operation::PrepareArtifact { config, resources } = request.operation else {
            return Err(CoreError::new(
                "guest-operation",
                "expected prepareArtifact",
            ));
        };
        dictionary_exact::DictionaryExact.validate_config(&config)?;
        let path = config["path"].as_str().unwrap();
        let descriptor = resources.0.get(path).ok_or_else(|| {
            CoreError::new("dictionary-path", "snapshot was not supplied by Host")
        })?;
        let resource = Resource::try_from(*descriptor)
            .map_err(|_| CoreError::new("dictionary-read", "invalid original descriptor"))?;
        if resource.len() > dictionary_exact::MAX_FILE_BYTES as u64 {
            return Err(CoreError::new(
                "dictionary-limit",
                "source exceeds Node file limit",
            ));
        }
        let mut original = Vec::with_capacity(resource.len() as usize);
        while original.len() < resource.len() as usize {
            let count = (resource.len() as usize - original.len())
                .min(diy_ime2_wasm_resource_sdk::MAX_CHUNK_BYTES);
            let part = resource
                .read_exact(original.len() as u64, count)
                .map_err(|_| CoreError::new("dictionary-read", "original snapshot read failed"))?;
            original.extend(part);
        }
        let artifact = artifact::prepare(&config, &original)?;
        let mut writer = ArtifactWriter::new();
        writer
            .write_all(&artifact)
            .and_then(|_| writer.flush())
            .map_err(|_| CoreError::new("dictionary-read", "artifact delivery failed"))?;
        let acknowledgement = PreparationAcknowledgement::new(
            artifact::FORMAT.into(),
            artifact::VERSION,
            writer.bytes_written(),
        )
        .map_err(|_| {
            CoreError::new("dictionary-not-prepared", "artifact acknowledgement failed")
        })?;
        serde_json::to_value(acknowledgement)
            .map_err(|_| CoreError::new("guest-json", "artifact acknowledgement encoding failed"))
    })();
    reply(result)
}

pub fn handle_execute(bytes: &[u8]) -> Vec<u8> {
    let result = (|| {
        let request = decode(bytes)?;
        let Operation::ExecutePrepared {
            config,
            artifact: descriptor,
            inputs,
            mode: _,
        } = request.operation
        else {
            return Err(CoreError::new(
                "guest-operation",
                "expected executePrepared",
            ));
        };
        dictionary_exact::DictionaryExact.validate_config(&config)?;
        if descriptor.format != artifact::FORMAT || descriptor.version != artifact::VERSION {
            return Err(CoreError::new(
                "dictionary-not-prepared",
                "unsupported dictionary artifact format",
            ));
        }
        let definition = dictionary_exact::DictionaryExact.definition();
        let inputs = guest::artifact_inputs(&definition, inputs)?;
        let input = inputs
            .get("in")
            .ok_or_else(|| CoreError::new("missing-input", "required input in is missing"))?;
        let mut resource = Resource::try_from(descriptor.resource).map_err(|_| {
            CoreError::new("dictionary-not-prepared", "invalid artifact descriptor")
        })?;
        let output: OutputValue = artifact::lookup(&mut resource, &config, input)?;
        guest::artifact_outputs(&definition, [("out".into(), output)].into())
    })();
    reply(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Value {
        json!({"apiVersion":1,"node":{"kind":"text.dictionary-exact","version":1},
            "operation":{"method":"executePrepared","config":{"path":"dictionary.tsv"},
                "artifact":{"resource":{"id":1,"length":32},"format":artifact::FORMAT,"version":1},
                "inputs":{"in":{"state":"noOutput"}}}})
    }
    fn error(value: &Value) -> String {
        let reply: Value =
            serde_json::from_slice(&handle_execute(&serde_json::to_vec(value).unwrap())).unwrap();
        assert_eq!(reply["ok"], false);
        reply["error"]["code"].as_str().unwrap().into()
    }
    #[test]
    fn companion_validates_protocol_identity_config_ports_and_format_before_read() {
        let mut value = request();
        value["apiVersion"] = 2.into();
        assert_eq!(error(&value), "guest-json");
        let mut value = request();
        value["node"]["kind"] = "other".into();
        assert_eq!(error(&value), "unknown-kind");
        let mut value = request();
        value["operation"]["config"] = json!({});
        assert_eq!(error(&value), "invalid-config");
        let mut value = request();
        value["operation"]["artifact"]["format"] = "wrong".into();
        assert_eq!(error(&value), "dictionary-not-prepared");
        let mut value = request();
        value["operation"]["mode"] = Value::Null;
        assert_eq!(error(&value), "guest-json");
        let mut value = request();
        value["operation"]["inputs"] = json!({});
        assert_eq!(error(&value), "missing-input");
        let mut value = request();
        value["operation"]["inputs"]["extra"] = json!({"state":"noOutput"});
        assert_eq!(error(&value), "guest-input");
        // Native builds have no ambient-file fallback, even for a valid descriptor.
        assert_eq!(error(&request()), "dictionary-read");
    }
    #[cfg(feature = "dictionary-artifact-producer")]
    #[test]
    fn producer_rejects_legacy_or_missing_original_resources() {
        let value = request();
        let output: Value =
            serde_json::from_slice(&handle_prepare(&serde_json::to_vec(&value).unwrap())).unwrap();
        assert_eq!(output["error"]["code"], "guest-operation");
        let value = json!({"apiVersion":1,"node":{"kind":"text.dictionary-exact","version":1},"operation":{"method":"prepareArtifact","config":{"path":"dictionary.tsv"},"resources":{"other.tsv":{"id":1,"length":1}}}});
        let output: Value =
            serde_json::from_slice(&handle_prepare(&serde_json::to_vec(&value).unwrap())).unwrap();
        assert_eq!(output["error"]["code"], "dictionary-path");
    }
}
