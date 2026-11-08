# Custom service guest authoring

Node issue #13; Host integration [DIY_IME2 PR295](https://github.com/pitapa79/DIY_IME2/pull/295), merged as `00b1f9a60bf9d5cd1b7ae8f378d323434b05b4f8`. This is an additive authoring API for a custom Wasm guest. It does not register a new official service/LLM Node, change the 48 registered processing definitions, or grant permission to access any service. The fixed Core SDK snapshot is unchanged.

## Authoring and exports

Implement `guest::service::ServiceNode`: definition, exact configuration validation, optional preparation validation, execute and optional resume. Execute returns `ServiceStep::Outputs` or `ServiceStep::Network(NetworkCall {binding,body,continuation})`. Resume receives exact config, the opaque continuation, `HttpResponse {status,body}` and a resource context, and can return another call or final outputs.

The Rust trait is an interface inside your compiled guest. JSON and the existing Wasm ABI are the external boundary; the Host never loads a Rust trait object from a package. Inputs and final outputs retain existing Packet/root/port validation. Required NoOutput skips the authored execute callback after config/resource preparation, while empty Text remains Data.

The context contains only the optional projected input mode during execute and a read interface to the supplied immutable snapshots. Resume has no original input map or mode; put necessary state in the continuation. It receives no RuntimeInput, input revision, RunKey, permission token, native path, header or API credential. Resources must be supplied again on each call and are never retained by the helper between Workers. Preparation can validate a snapshot or cache something in the newly constructed instance for that call; it is not a promise of a persisted Store.

For a custom module, enable `diy-ime2-nodes` feature `custom-guest` and invoke:

```rust,ignore
use my_node::MyService;
diy_ime2_nodes::export_service_guest!(MyService::default());
```

The macro constructs the factory in its caller's scope, creates a fresh authoring instance for each call, and exports the existing version/alloc/call/release functions. It requires the feature at compile time, validates the current pending pointer and exact length before consuming it, and bounds allocations/replies to 8 MiB. Requests/replies are guest-owned until release/next call, with the same serialized Store-call rule as the official ABI.

`custom-guest` suppresses only the official 48-Node ABI exports, preventing duplicate export names in the custom consumer. It is opt-in; the default official Wasm build retains its exports and factory. Build the custom consumer into a separate target directory. A shared Cargo feature build must not be used as the official distribution artifact. The CI explicitly verifies that building the custom fixture does not overwrite the official module.

The normal official dispatcher explicitly refuses Resume and does not create custom service Nodes. The helper and export macro are available to authors; they do not make an unregistered custom Node part of the default registry.

## Wire and limits

Execute and final output envelopes are unchanged. A call is the exact alternative success shape:

```json
{"apiVersion":1,"ok":true,"call":{"binding":"api","body":[104,105],"continuation":{"step":1}}}
```

The Host chooses method/destination/headers/authentication from its fixed binding, validates manifest `network.request` and, where required, same-binding `secret.use`, and asks for separate network/secret-use consent and the source input flow. The binding string is a logical name; the helper refuses URLs/path-like or malformed names. It is a request, not a capability or authorization object. The helper performs no HTTP, filesystem or environment operation and introduces no WASI/network imports.

Resume accepts exactly config, present continuation (including null), response `{status,body}` and optional resources. Status must be 100..599 and body is a byte array. Unknown input/mode/URL/header/permission/credential fields, missing continuation/response/config and invalid responses are refused. Request parsing retains the JSON recursion bound. Complete request and response messages remain bounded to 8 MiB. Response serialization uses a bounded writer and replaces oversize outputs or authored errors with a complete small failure envelope; it never publishes a partial JSON prefix.

Decoder errors do not echo raw requests. Authored Node error messages remain the author's diagnostic values; the helper is not automatic secret masking. The production Host independently validates the raw reply, strips guest error-message text and enforces its own smaller/aggregate limits and current execution lease. It retains the Core physical job through actual HTTP and Worker ending; changing or cancelling a request does not retract an already sent remote side effect.

## Verification

Seven native integration tests cover actual authoring callbacks, Unicode/metadata and pure output, one/two continuation hops, required NoOutput vs empty text, exact snapshot projection/non-retention, strict resume/status/field rejection, invalid identity/config/ports/bindings/output, oversize request/call/error and the official dispatcher's refusal to execute services.

`tests/fixtures/service_guest` is an independent consumer of the public macro, sharing only a synthetic test implementation. It is excluded from the root workspace through its own workspace declaration and builds into a separate target directory. Its locked registry records must all match the existing Node lockfile. `scripts/check-service-guest.ps1` checks 23 actual ABI/call/resume/snapshot/NoOutput/refusal cases in both the normal probe and real AppContainer/Job paths. Existing migration, immutable SDK, formal contracts, 54-case ABI/refusal checks, NoOutput and prepare probes remain in the same CI.

`scripts/check-service-host.mjs` is an optional joint development test with explicit built Host/producer/Worker/DLL/module/policy/probe paths. It performs no cross-repository fetch and requires no GitHub credential. With the production Host from the merge above, it verifies isolated package proof, partial-consent refusal, Host-only synthetic authentication, one/two actual loopback HTTP hops, exact result/continuation and cancellation while Worker count is zero but a physical Core job remains owned. The Node CI remains independently buildable without a Main checkout or private repository token.

All keys and services in tests are synthetic/local. Official package production/default cutover and distribution/performance validation remain separate work.