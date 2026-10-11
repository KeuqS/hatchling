# Hatchling session protocol

**Status:** v12 (current). This doc is the human-readable reference
for the broker session wire protocol. The Rust types live in
`crates/hatchling-core/src/messages.rs` (op families). The
authoritative version history is the doc comment on
`PROTOCOL_VERSION` in `crates/hatchling-core/src/framing.rs` —
if this doc and that comment disagree, the comment wins.

**Versioning:** the protocol is versioned by op-family additions.
Versions must match exactly: a peer speaking any other version is
a protocol violation, not a negotiation. Note: v12 made
`ScheduleSpec`'s `tz` required, so a bump can tighten a
previously-loose field. Peers reject unknown variants loudly —
no silent misinterpretation.

## Transport

Sessions run over the broker's framed transport (`MAX_FRAME_BYTES`
= 64 KiB per frame, length-prefixed). Every request is a
`SessionRequest`, every response a `SessionResponse`. All types are
serde-serializable; the wire format is JSON.

## Op families

| Family | Request | Response | Since |
|---|---|---|---|
| Vault | `VaultOp` | `VaultResult` | v2 |
| Egress | `EgressOp` | `EgressResult` | v3 |
| Channel | `ChannelOp` | `ChannelResult` | v4 |
| Memory | `MemoryOp` | `MemoryResult` | v1 |
| Skill | `SkillOp` | `SkillResult` | v5 |
| Goals | `GoalsOp` | `GoalsResult` | v6 |
| Query | `QueryOp` | `QueryResult` | v6 |
| Schedule | `ScheduleOp` | `ScheduleResult` | v7 |
| Hook | `HookOp` | `HookResult` | v8 |
| Artifact | `ArtifactOp` | `ArtifactResult` | v9 |
| Space | `SpaceOp` | `SpaceResult` | v10 |
| Subagent | `SubagentOp` | `SubagentResult` | v10 |

## v9 — Artifact ops

The artifact family covers builds, delivery, and the idea lifecycle.

**`ArtifactOp` variants:** `Build`, `Deliver`, `Describe`,
`CaptureIdea`, `ListIdeas`, `NoteIdea`, `ProposeActivation`,
`Activate`, `RefineIdea`, `BuildIdea`.

- `build_kind: String` — names the builder kind (e.g.
  `"document"`, `"page"`). Carried on `Build`, `BuildIdea`, and
  `ProposeActivation`.

**`ArtifactResult` variants:** `Built`, `BuildFailed`,
`Delivered`, `Refused`, `Described`, `UnknownBuild`,
`IdeaSaved`, `Ideas`, `ActivationProposed`, `IdeaRefused`,
`Denied`, `ArtifactError`.

**Typed failures** (not strings):

- **`BuildFailure`** (7): `BadRequest`, `UnknownBuilder`,
  `CapabilityDenied`, `BuilderFailed`, `BadOutput`, `Full`,
  `UnknownKind`.
- **`DeliveryRefusal`** (7): `UnknownBuild`, `Incomplete`,
  `HashMismatch`, `Blocked`, `GateError`, `BadTarget`,
  `DeliveryFailed`.

Every failure is typed so callers can match on it. A build that
fails for a reason not in `BuildFailure` is a protocol gap, not a
string to parse.

## v10 — Space and subagent ops

**Space** covers the shared-state primitive: key-value
(`Put`/`Get`/`Remove`), task lists (`AddTask`/`RunTask`), sync
(`SyncPull`/`SyncPush`/`Conflicts`), media (`PutMedia`/
`GetMedia`), and pairing (`Pair`, `Create`, `Join`).

**Subagent** covers bounded child-agent delegation:

**`SubagentOp`:** `Spawn { task, budget, capabilities }`,
`Wait { child }`, `Kill { child, reason }`.

**`SubagentResult`:** `Spawned { child, ... }`,
`Report { ... }` (the typed result envelope),
`Refused` (typed refusal — e.g. capability widening, depth bound),
`SubagentError`.

Rules enforced at the protocol layer:

- Capabilities are a subset of the parent's grants; widening is
  refused before a cell exists.
- Budget is allocated as spend (no refunds); the broker keeps a
  monotonic ledger (max 64 steps/session, 8 children, 32
  steps/child).
- Depth is structural (max 1 — a child cannot spawn).
- A child's death/timeout surfaces as a typed result, never kills
  the parent.

Full subagent lifecycle semantics are in
`docs/activity-monitor-event-contract.md`.

## v11 — Redaction results

`ChannelResult::Redacted`: the output gate's verdict surfaced as
a typed result. When the gate redacts a channel message, the
result carries the verdict instead of the content.

## v12 — Schedule list

`ScheduleOp::List { after }` / `ScheduleResult::Listed`: the
read-only schedule listing. `ScheduleSpec.tz` is required in
the type — a schedule without a timezone is refused, not
defaulted.

## Device handshake

`DeviceHello` / `DeviceWelcome` / `DeviceRefusal` — the pairing
handshake for new devices. Typed refusal, not a string.

## For the frontend

The frontend's generated types must cover every variant listed
here; the drift test fails if the schema and these types
diverge. When a new op family lands, this doc is updated in the
same commit.
