# Official Node development

## Read first

Before starting, read [the Node development and issue governance policy](docs/node-development-governance.md), [README.md](README.md), [Node classification](docs/node-classification.md), [Node contracts](docs/node-contracts-v0.1.md), and the assigned Issue (including comments and dependencies). The governance policy is authoritative for Node-repository Issue/branch/PR/approval/adoption workflow; Core API and security contracts remain authoritative in the Main repository.

## Approval and task boundaries

- AI may suggest and create Issues, but an Issue is not permission to implement it. Start only explicitly user-approved work, a clearly delegated portion of an approved parent Issue, or a narrowly preapproved category of minor fixes.
- One Node kind has one long-lived management Issue (created gradually). Work is tracked by separate short-lived Issues, normally one branch and one PR per work Issue; explicit multi-PR splits are allowed for large tasks. Never maintain an ongoing branch per Node.
- Only the user decides external-facing feature/spec changes, official adoption, default bundling, Core API contract changes, and security/permission policy changes. Technical implementation and conditionally delegated merges are described in the governance policy.
- Do not invent unresolved requirements, mark experimental Nodes as official, register opt-in prototypes, or start work outside the authorized scope. Do not spawn other agents unless explicitly requested.

## Ownership and compatibility

- Individual Node owners edit their own implementation, tests and examples. Common registration, pinned SDK imports, Cargo manifests/lockfiles, packaging/API and CI are integration-owned unless the assigned Issue explicitly delegates them.
- Keep existing kind/version/ports/schema/default, Unicode, Data/NoOutput/Error, and formal fixture meaning. Never weaken fixtures to pass an implementation. A requirement for new Core APIs goes to a Main-repository Issue for user approval before a reviewed SDK import.
- Canonical Core/OS/system Nodes belong to [DIY_IME Main](https://github.com/pitapa79/DIY_IME). `sdk/runtime` is a pinned dependency snapshot, not a second development source. Do not edit it directly or relabel modified sources as unchanged. `tests/support` contains test-only system adapters.
- Rust's internal Node trait is not the external Wasm ABI. Other languages are allowed when they implement the supported Host contract; language support is verified separately from official adoption.

## Execution, review and handoff

- Use a dedicated branch and worktree/clone; do not overwrite other agents' work. An agent handoff preserves branch, commit SHA, tests and remaining work.
- Run targeted tests and relevant README/CI integration checks. Explain any update to migration-provenance hashes and document changes to source bytes. Source separation alone is not proof of sandboxing or completed product integration.
- PRs reference their work Issue, changes, tests and limits. Seek AI code review where practical, and do not call a self-review independent. An integration agent can merge only approved, reviewed changes passing required CI without new unapproved behavior or permissions.
- Report actual results, unknowns, and blocked decisions. The user's explicit instructions in the active session take precedence over repository workflow defaults.

## Migration

The Node governance policy applies to new work. Existing Issues/PRs/branches are not retroactively invalidated or renamed. Existing Node management Issues are added progressively, not in a 48-Issue bulk operation.
