# Frontend protocol

One contract between the frontend and the backend, in one schema, with both sides' types generated from it. A type that changes on one side and not the other fails a gate.

## The source of truth: Rust first

The wire types are written once, in Rust, in `hatchling-core`:

- `crates/hatchling-core/src/ui.rs` holds the frontend protocol: the handshake, the event stream and the user actions.
- `crates/hatchling-core/src/messages.rs` and `crates/hatchling-core/src/subagent.rs` hold the broker session protocol (v12, every op family) and the activity-monitor events.

**Why Rust first.** The backend's types already existed, with their serde attributes (`tag`, `rename`, `flatten`). A hand-written schema beside them would have been a second source, with its own drift. Instead each Rust type derives a JSON Schema through `schemars`, which reads serde's own attributes, so the schema says exactly what serde puts on the wire. The derives sit behind `hatchling-core`'s off-by-default `schema` feature, which only the dev tool turns on, so no runtime binary compiles them.

## Generating both sides

| Step | Command | Output |
| --- | --- | --- |
| Rust → schema | `cargo run -p hatchling-dev -- protocol-schema --write` | `frontend/protocol/schema.json` (this directory) |
| schema → TypeScript | `npm run protocol:generate` (in `frontend/`) | `src/adapters/live/protocol/generated.ts` (the types and the shared constants) and `src/adapters/live/protocol/wire.ts` (the shapes the UI frames reach, as data for the runtime check) |

Both outputs are checked in, so a review shows the contract change itself, and the frontend builds without a Rust toolchain.

**Deterministic.** The same input always gives the same bytes:

- **The schema** is written with every object's keys sorted, a two-space indent and one trailing newline. That is also what the frontend formatter produces, so formatting never changes it.
- **The TypeScript** is emitted in sorted order, whatever order the schema lists its keys in, and goes through the same TypeScript printer as `scripts/format.mjs`.

Both properties are tested.

**Strict.** The TypeScript generator handles only the JSON Schema constructs the Rust types produce. Anything else is refused with its location, never guessed at: a construct it skipped would become a silently wrong type. The same holds for the runtime check (`conform.ts`): every received frame is checked against `wire.ts` before it is read, and a keyword the check does not interpret fails generation instead of being dropped.

## Shared constants

The schema carries what both ends must apply identically, under `x-hatchling`, and the generator emits each as a constant:

- **The mismatch sentences** (`incompatible`), for version skew.
- **The takeover table** (`control.transitions`, v2): every allowed handoff of a shared surface (`agent`, `user`, `to_user`, `to_agent`), from `hatchling-core::ui::control`. The UI's state machine reads `CONTROL_TRANSITIONS`, so the broker and the UI cannot disagree about who drives. A handoff outside the table ends the connection.

## Drift checks

| Check | Where | Fails when |
| --- | --- | --- |
| `cargo run -p hatchling-dev -- protocol-schema`, and a `hatchling-dev` unit test | `bash scripts/check.sh` (Rust gate); CI `protocol drift` job | a Rust wire type changed but `schema.json` was not regenerated, or `schema.json` was edited by hand |
| `npm run protocol:check`, and a unit test | `npm run check` (frontend gate); CI `protocol drift` job | `schema.json` changed but `generated.ts` was not regenerated, or `generated.ts` was edited by hand |

A Rust change therefore lands with its regenerated schema and TypeScript in the same commit. When a new session op family lands, `docs/session-protocol.md` is updated in that commit too.

## Versioning

- **Two versions:** `UI_SCHEMA_VERSION` (`hatchling-core::ui`, emitted as `x-hatchling.ui_schema_version`) versions this protocol. A breaking change bumps it, and the two ends must match exactly. The session protocol keeps its own version (`PROTOCOL_VERSION`, 12).
- **History:** v1: chat, activity, artifact builds, schedules, confirmations. v2: the interactive surfaces — agent status, artifact revisions, files, and the live browser with takeover. v3: the local API (`Hello` token, `not_granted` refusal, `api_call` activity). v4: coding-harness runs (`harness_run` activity).
- **The handshake:** the UI's first frame is `hello` with its version. The backend answers `welcome`, or `incompatible` with a sentence a person can act on, for example:

  > frontend v3 needs broker ≥ v3; broker is v2 — restart the broker to update

  The two sentence templates live in the schema (`x-hatchling.incompatible`). So the backend (`incompatibility_message`) and the frontend (`incompatibilityMessage`) say exactly the same words, and both are pinned by tests.
- **Skew detected on either side:** the frontend also checks a `welcome`'s version itself. An older broker that does not know to refuse still gets the user an explanation instead of a misrendered UI.

## Replay

On reconnect the client resumes rather than starting over or re-reading everything blindly:

- **Ordering:** every event carries `seq`, strictly increasing within a session's stream.
- **Resuming:** the client keeps the highest `seq` it applied (`EventLedger.afterSeq()`) and resumes with `subscribe { after_seq }`.
- **Idempotence:** an event at or below the mark is a duplicate and changes nothing.
- **Confirmations are decided once:**
  - Once the client has applied `confirmation_decided` for a confirmation, replaying its request never presents it again.
  - `EventLedger.decide()` returns no action for anything not pending, so a decision is never sent twice.
  - A decision names the revision the user saw, and the backend rejects a stale one (`decision_rejected: stale_revision`).

## Activity state

Activity events carry the state the feed shows (`ActivityState`). The backend derives it in one place (`activity_state`), per the activity-monitor contract:

| Event | State |
| --- | --- |
| `subagent_spawned`, `subagent_progress` | `pending` |
| `subagent_finished`, verdict `ok` | `success` |
| `subagent_finished`, verdict `partial` | `partial` |
| `subagent_killed` | `interrupted`: a killed child, distinct from a failed one |
| `subagent_failed` | `failed` |

## Transport, to the depth the schema needs

There is no separate HTTP presentation API:

- **The browser:** each frame is one JSON object, a `UiRequest` from the UI or a `UiResponse` to it, over one ordered, reliable, message-framed duplex channel per UI session.
- **The local frontend host** carries that channel. It is a thin transport adapter: it speaks the broker's Unix socket protocol, and the broker authenticates it by peer credentials, like every other caller. The adapter has no routes, no authorization logic and no security properties of its own.
- **Not done yet:** choosing the browser-side channel and hardening it, with the `hatchling-host` crate. The live adapter takes the channel as an injected `UiTransport`.

**Thin is enforced, not just intended:**

- **Rust:** the `arch` check (`hatchling-dev`) holds the future host crate, `hatchling-host`, to the wire-contract dependencies and rejects authorization vocabulary in its code.
- **TypeScript:** the linter rejects it in `src/adapters/live/`.

## Numbers

Rust `u64` fields (`seq`, budgets, sizes) become TypeScript `number`. They are exact below 2^53, far above anything these fields reach.

## Placement

- **The schema:** this directory.
- **The generated and hand-written live-adapter code:** `src/adapters/live/protocol/`, the only place `AGENTS.md` permits new live code.
- **The frozen domain types:** `src/protocol/` and its locks change only by a recorded rebaseline. The live adapter maps the wire types onto them.
