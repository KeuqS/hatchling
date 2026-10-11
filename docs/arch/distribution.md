# Distribution and API Review

Pre-launch decisions about what ships, where, and under what
stability guarantees. Semver is a promise — these choices bind
us from the first public release.

## Crates.io publication

### Publish

These crates are independently useful and have stable APIs:

- `hatchling-core` — protocol types, grants, errors. The
  foundation others build on.
- `hatchling-isolation` — the `IsolationBackend` trait and cell
  lifecycle. Useful standalone.
- `hatchling-vault` — secret storage with the egress-proxy
  boundary. Useful standalone.

### Do NOT publish (yet)

- `hatchling-broker`, `hatchling-agent`, `hatchling-loop` —
  the runtime. Ships as a binary, not a library. Publishing
  would imply API stability we're not ready to promise.
- `hatchling-dev` — **never.** Dev-only tooling. Publishing
  it would be a mistake.
- `hatchling-research` — the agent's research mode. Ships inside
  the agent, not as a library.
- Everything else — evaluate per-crate after launch based on
  actual external demand. Don't publish speculatively.

### Versioning

- All published crates start at 0.x. No 1.0 until the API has
  survived real external use.
- The workspace uses unified versioning — all crates share
  the same version number, bumped together.
- Breaking changes bump the minor version (0.x → 0.y).
  No breaking changes in patch releases, ever.

## Public API surface

The public API is:

1. **The local API** (Unix socket, JSON). Versioned by
   `UI_SCHEMA_VERSION` (currently 4). The broker session
   protocol is at v12. Documented in `docs/local-api.md`.
   Breaking changes bump the version.
2. **The CLIs** (`hatchling-broker`, `hatchling-agent`).
   Subcommand names and flags are the user-facing API. Don't
   rename flags lightly.
3. **Published crate APIs** (the three above). Semver-bound
   from first publish.

Everything else is internal, even if it's technically `pub`
in Rust. We reserve the right to refactor internals without
notice until 1.0.

## Installation paths

Planned. None of these exist yet: today, build from source (see the
README's quickstart).

| Method | Target | Priority |
| --- | --- | --- |
| `cargo install --git https://github.com/undergroundrap/hatchling` | Rust users | First release |
| AUR (`hatchling-bin` / `hatchling`) | Arch users | First release |
| Release binaries (GitHub) | Linux x86_64, ARM64 | First release |
| Homebrew | macOS users | Post-launch |
| Native packages (.deb, .rpm) | Broader Linux | Post-launch |

## System requirements

- **Linux:** kernel 5.11+ with unprivileged user namespaces
  and seccomp-BPF. Isolation is mandatory — without these
  primitives the agent refuses to start; there is no degraded
  or unsandboxed fallback.
- **Windows:** WSL2 (primary). Native Windows backend is
  post-launch.
- **macOS:** post-launch via the isolation trait's platform
  backends.

## What we're NOT promising yet

- Plugin API stability (skills format may evolve).
- Multi-model support (single-model-first by design).
- Enterprise features (post-launch).
