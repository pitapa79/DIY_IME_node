# Experimental guest API v1

This API projects connected Packet/NoOutput values, explicit config, optional compose/finalize mode and authorized read snapshots. It does not serialize RuntimeInput.text, Session/plan/run ownership, permission tokens, Core pointers or OS handles.

Request: apiVersion=1, node={kind,version}, operation.method=describe/validate/prepare/execute. Custom service guests additionally support resume; the official pure dispatcher refuses it.
- describe returns the existing NodeDefinition.
- validate takes config and returns valid=true or the existing Node error.
- execute takes config, inputs keyed by declared port, optional mode, and resources keyed by the exact configured reference with byte arrays. Values are state=data with packet, or state=noOutput.
- Reply: apiVersion=1, ok=true/data, or ok=false/error={code,message}. Node messages retain existing diagnostic semantics; this is not automatic secret masking.

Unknown fields, unsupported API/kind/version, undeclared/missing required ports, invalid Packet/root/config, absent/oversize read snapshots and excess message size are rejected. Both messages are bounded to8MiB and the JSON decoder retains its recursion bound. NoOutput, empty Text and empty Choice remain distinct. Required NoOutput skips Node execute after config/prepare validation, matching the existing Engine ordering.

Resources must be selected and authorized by the trusted Host; guest bytes never grant permission or trigger Host filesystem access. A fresh official processing instance is made per invocation. The internal Rust context has local diagnostic IDs=1 and input text empty; only mode is projected. These values confer no authority. Prototype2 and system4 are not guest factories.

## wasm32-unknown-unknown ABI

- diyime_api_version() -> i32
- diyime_alloc(length:i32) -> pointer:i32; zero rejects empty/oversize allocation.
- diyime_call(pointer:i32,length:i32) -> i64; exact pending allocation only. Low32bits are reply pointer, high32bits are byte length. Zero descriptor means invalid allocation identity.
- diyime_release_reply() frees the last reply. Host copies bytes before release/next call and re-reads memory bounds after every guest call.

Guest owns one pending request and one reply buffer. Calls on a Store are serialized. The Host checks exports/types, API version, offset+length, limits and current invocation ownership. This module imports no WASI, filesystem, network or environment APIs. The current C API probe uses finite test memory/fuel. Production policy, OS isolation, GUI consent and broker connections are implemented in the canonical Host repository; official distribution/cutover remain separate work.

## Reproduce on x64 Windows

Install Rust1.90 GNU and its wasm32-unknown-unknown target, then run scripts/check-wasm-runtime.ps1. It fetches only the pinned official Wasmtime50.0.1 C API archive after SHA256 verification, builds the Wasm and a test-only probe imported from a pinned Core commit, and checks54 real-Wasm cases plus6 specifically classified refusals and recovery. Nothing is loaded into the IME/TSF/Core process.

The runtime probe is development-only and does not itself grant GUI execution or network permission. The canonical Host provides isolation and consent; native Node support remains a later milestone. The Core source is canonical for dev/runtime-probe.rs and scripts/check-wasm-guest.ps1; check-runtime-probe verifies their imported blob hashes.
NoOutputはstateだけを持つ厳密なobjectです。trusted/permissions/packet等の余分なfieldはguest-jsonで拒否し、通常のNoOutputや後続の正常変換は維持します（#9）。

## Explicit preparation validation (#11)

The additive prepare operation accepts exactly config and resources (an exact-reference → byte-array map). It validates config and invokes the existing Node.prepare using only those immutable snapshots. A successful reply is data={valid:true,prepared:true|false}; prepared=false means that Node has no special prepared instance. No input Packet or RuntimeInput is supplied, and unknown fields or missing/null resources are refused. The empty describe variant is also strict; its existing wire {method:"describe"} is unchanged.

This allows a trusted Host to detect dictionary encoding/format/size failures before activation, after it has separately obtained resource permission. It does not read any Host path, create an authorization, or retain an instance/resource between guest calls. Execute still receives its explicitly authorized snapshots and constructs the per-call prepared instance. A later call without those bytes still fails. This preserves existing behavior and avoids claiming that a fresh AppContainer process shares a prepared Store.

The additional scripts/check-prepare.ps1 runs in both normal and actual AppContainer paths through check-wasm-runtime.ps1. It tests dictionary/pure preparation, missing/invalid snapshots and unknown input/authority fields, resource non-retention, error non-echo and normal execution recovery. Existing pinned runtime/fixture probes, SDK snapshot, 48 definitions and formal Node fixture meanings remain unchanged. Host roots, approval and broker implementation remain in the Core repository.


## Custom service continuation (#13)

The additive Resume operation and ServiceNode helper/export macro are documented in [Service guest SDK](service-guest-sdk.md). The default processing dispatcher rejects Resume; only an explicitly compiled custom guest handles it. Existing pure operations/definitions and fixed SDK remain unchanged. Actual Host network/secret-use authorization stays outside the guest.
