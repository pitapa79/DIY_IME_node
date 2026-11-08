# Official processing Node package production

Issue #15; prerequisite for the [Main Wasm distribution/cutover](https://github.com/pitapa79/DIY_IME2/issues/257). The producer creates data for the existing `DIY2NODE` format. It does not grant execution, resource access or default adoption of prototypes. End users receive compiled packages and do not need Git, Cargo or a Node rebuild.

## Exact scope

The current registered processing inventory has 48 definitions. Each becomes `official.<kind>` in one `<kind>.diynode` file, retaining exact kind/version/config schema/default/ports and the existing classification/status. The 4 canonical system Nodes remain in Main; 2 unregistered prototypes remain opt-in and are not packaged. Packaging registered experimental/compatibility Nodes does not reclassify them as standard functions.

All packages embed the same default-feature official Wasm module. Each manifest selects only its own Node identity. Only `text.dictionary-exact` requests `dictionary.read` with logical binding `dictionary`; projected input mode does not add filesystem authority. No official package requests network or secret use. A later module rebuild may conservatively change all 48 executable identities, even if only one implementation changed; authorization follows actual bytes.

Version is the crate version plus build metadata `+g<full-source-commit>`. Manifest metadata, such as this source version, does not itself create permission. The Host independently verifies the module's actual isolated definition/default config and the exact runtime configuration before approval/activation.

## Producer and framing

`cargo build --locked --release --example official_packages` builds a Windows development tool using the existing locked Windows BCrypt dependency. It neither loads Wasm nor imports Main implementation code. Invocation is:

```text
official_packages MODULE EXPECTED_SHA256 SOURCE_COMMIT NEW_OUTPUT_DIRECTORY
```

The expected module SHA256 and full lowercase source commit are supplied by the owning build pipeline. The tool captures bounded module bytes once, checks their binary Wasm header/digest, uses actual registered Rust definitions and compares them with the immutable inventory, and emits deterministic packages. It refuses extra/malformed arguments, bad identities, module/digest/metadata/aggregate limits, unsafe output names and any existing output. It does not prove compiler lineage from arbitrary binary bytes; a caller can only make a source claim, so validated build receipts and source pins remain necessary.

Each file is exactly 8 bytes `DIY2NODE`, little-endian u32 format=1, u32 manifest length, u64 module length, strict existing manifest JSON and binary Wasm. Existing limits apply: 1MiB manifest, 64MiB module, 65MiB+24 complete package and 512MiB aggregate package data. No format fields are added to the existing package manifest.

The tool writes a CNG-named sibling staging directory with create-new files and syncs every file before publishing the complete directory with non-replacing `MoveFileW`. Output and staging are checked against their selected canonical parent. On failure it removes only its own recorded files and empty stage, without recursive deletion or overwrite. Atomic publication is not a power-loss durability guarantee. Existing output is never silently replaced.

## Index and build receipt

`official-index.json` formatVersion=1 records source repository/full commit plus hashes of Cargo.lock, migration provenance and legacy inventory; module SHA256/size/guest API; total package bytes; and sorted package basename/id/hash/size/kind/Node version/classification/status. Basenames are simple relative filenames. This index is metadata, not a signature, executable proof or grant. A bundled Host must pin trusted official data and must preserve runtime proof/consent; arbitrary imported indexes cannot declare themselves official.

`scripts/build-official-packages.ps1` requires an explicit build source commit, selects the pinned Rust1.90.0 toolchain/default-feature release module, and uses encoded remap flags for workspace/cargo-registry paths (including spaces). It restores the caller's environment. A separate build target prevents custom-guest feature artifacts from contaminating the official module. The script runs the actual Wasm checks on that exact module, then `check-official-packages.ps1` independently checks all 48 complete Wasm definitions, frames, embedded module/digests, classification, authority, source hashes and no-overwrite behavior. Package generation is not substituted for actual module validation.

A sibling `.build-receipt.json` records toolchain/target/profile, public remapping labels, source commit, module/index/producer digests and proof counts. It never records the native build paths. The module is checked for unremapped workspace/registry strings. This supports provenance inspection and package determinism for captured module bytes; it does not claim bit-for-bit compiler reproducibility across every environment or cryptographic signing.

CI preserves the 48 packages/index and separate receipt as an artifact named with its exact checkout commit. It uses the pinned upstream `actions/upload-artifact` v6 commit. Node CI remains buildable without Main checkout/private repository credentials. The CI timeout covers the extra verified release build; runtime budgets and untrusted import bounds are unchanged.

## Validation and integration

Native tests verify the known SHA256 vector, exact framing, deterministic registered-only metadata, dictionary-only capability, bad module/source/digest rejection, completed-directory publication and existing data retention. The independent PowerShell checker compares complete actual-Wasm definitions before publication and independently parses every frame and digest afterwards.

The existing immutable SDK/246 imported assets/formal fixture checks and Node contracts remain authoritative. Main's existing isolated package verifier/catalog and official processing graphs must also accept these exact generated files before default cutover. Its untrusted import review has a 16-package batch bound: importing all 48 uses three bounded batches or a separately designed trusted bootstrap, not a larger untrusted limit. No source copies are removed from Main until compiled official distribution/cutover is verified. The complete Windows application bundle and release performance/combined QA remain subsequent work.
The optional independent check "scripts/check-official-host.mjs" takes explicit MANAGER HOST WORKER DLL OFFICIAL_DIRECTORY RESOURCE_CONTRACT arguments. It does not fetch Main or require cross-repository credentials. Against the production Main it imports all 48 packages in three existing bounded reviews, verifies exactly the four system plus 48 processing definitions, and runs the registered romaji/dictionary/select/stringify graph through explicit input-flow and resource approval. It checks Unicode, stable dictionary ties, unknown words, empty Data and saved-permission reopening. Pass the exact compiled release package directory being evaluated; this check does not replace either repository's own contract suites.
