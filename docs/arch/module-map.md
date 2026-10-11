# Module Map

Hatchling is a Rust workspace. This is what each crate does and
how they fit together. If you're looking for where something
lives, start here.

## Core runtime

| Crate | Role |
| --- | --- |
| `hatchling-broker` | Central coordinator. Spawns cells, routes messages, enforces grants, serves the local API. Never executes untrusted code. |
| `hatchling-agent` | The binary that runs inside a cell. It reaches the broker over its one peer-authorized session; all its logic is library code, tested without spawning it. |
| `hatchling-core` | The shared foundation: identity newtypes, protocol framing and message types, grants, kernel peer credentials. The vocabulary everything else speaks. |
| `hatchling-loop` | The agent's tool-calling loop. The model proposes; the host validates and bounds. LLM-agnostic. |

## Isolation and security

| Crate | Role |
| --- | --- |
| `hatchling-isolation` | Cell lifecycle and the `IsolationBackend` trait. Platform-neutral API; Linux backend implements namespaces + seccomp. |
| `hatchling-sys` | Low-level syscall wrappers (seccomp, namespaces, mounts, process and fd handling). The only crate allowed `unsafe`. |
| `hatchling-vault` | Secret storage. Credentials never leave this crate's boundary except via the egress proxy. |
| `hatchling-egress` | The egress proxy: the only component that touches the network for a cell. Allowlist with pinned DNS, surrogate tokens, credential substitution. |
| `hatchling-adversarial` | Attack simulations used in testing. Proves the isolation holds. |

## Capabilities

| Crate | Role |
| --- | --- |
| `hatchling-skill` | Skill loading, verification, and execution in cells. |
| `hatchling-scheduler` | Scheduled tasks: create, list, cancel, fire on time. |
| `hatchling-memory` | Broker-owned durable memory (user and project), paged so results stay bounded. |
| `hatchling-space` | Spaces: named, persistent scopes with members, and their sync, devices and media. |
| `hatchling-feed` | The feed: a scheduled digest of the user's own records (goals, tracking, tagged memory). |
| `hatchling-goals` | Broker-owned goals and an append-only tracking log, with the read-only query the feed uses. |
| `hatchling-prompt` | The authored prompt pack: the system prompt, built from named sections. |
| `hatchling-model` | The model provider: an OpenAI-compatible chat-completions client, reached through the egress proxy. |
| `hatchling-artifact` | The artifact pipeline: where untrusted build output becomes user-facing content, with an inspector at the boundary. |
| `hatchling-fido2` | The FIDO2 transport (CTAP2 over USB HID), one of the vault's unlock factors. |
| `hatchling-channel` | Messaging channels: how the user reaches the agent. The broker owns the endpoints. |
| `hatchling-builders` | The document and page builders, each a signed skill. |
| `hatchling-research` | Deep research runs: plan, gather, synthesize, and deliver a cited report as an artifact. |

## Developer tooling (never shipped)

| Crate | Role |
| --- | --- |
| `hatchling-dev` | **Dev-only.** Test harness, mutation-testing support, conformance checks, delegation verification. Never linked into the runtime. |

## Dependency rules

- `hatchling-core` depends only on `hatchling-sys`. It's the foundation.
- The broker depends on almost everything but never on `hatchling-dev`.
- `hatchling-dev` is never a dependency of any shipped crate. A test enforces this.
- New cross-crate dependencies need justification in the PR.

## Where to start reading

1. `hatchling-core` — the types. Understand the vocabulary first.
2. `hatchling-broker` — the coordinator. Understand the flow.
3. `hatchling-isolation` — the security model. Understand the boundaries.
4. Then follow your interest.
