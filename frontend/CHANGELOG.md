
## Unreleased UI polish

- Chat navigation hover now uses one row highlight instead of a nested button highlight.
- Browser tabs now follow `Page — character`, using the configured character name (`hatchling` by default).
- Hatchling logo/avatar, favicon, app icons, profile surfaces, and avatar transitions share the white circular identity base.
- The source package includes the compiled click-around preview under `preview/`.
# 1.12.0 — shared popup menus and sandboxed preview navigation

- Unified ordinary action menus with the existing More-menu surface, spacing, typography, SVGs, hover/focus states, separators and semantic destructive actions.
- Removed older feature-specific menu cosmetics and guarded shared ownership in design:check.
- Kept menus inside the viewport, including touch/long-label layouts, keyboard navigation, scroll/resize and the no-Popover fallback; preserved the horizontal reaction picker.
- Made denied URL-history writes nonfatal in script-enabled embedded previews; local route rendering continues without parent navigation or weakened sandbox rules.
- Added startup instructions for script-blocking viewers and documented standalone versus served previews.
- Preserved product data, adapters, protocol/session contracts, bootstrap, mascot/media, approvals, recovery behavior and Connector chats.

# Shared loading and design guide / 1.11.0

- Added a root STYLE_GUIDE.md, an agent read-first instruction, UI review checklist, shared spacing/geometry/type/motion primitives, and a focused design-token/style guard in the normal check command.
- Added reusable text, avatar, media, chat, document and library grid/list skeletons. Dark/light colors and reduced-motion behavior come from one implementation.
- Integrated real image loading/error/cache/timeout/retry across chat, library, feed and image viewers. Frame and metadata remain stable. Full viewers own retry controls; thumbnails contain no nested actions. Lazy images do not time out before visibility; removed pending views release observers/listeners/timers.
- Unified existing chat-history placeholders. Feed generation now tracks the real pending command, keeps cached posts readable, blocks duplicate generations and exposes failure/retry without inventing success.
- Added explicitly labeled collection-loading examples and a document component specimen, not artificial delays on normal routes or a fabricated backend readiness API.
- The standalone builder reads stylesheet order directly from index.html, preserving the served/standalone cascade. Added 16 unit tests and 22 browser cases for loading, media, accessibility semantics and preserved approvals/drafts.
- Made the existing accordion animation test sample actual CSS transition progress rather than unreliable VM wall time; its geometry and reduced-motion assertions remain. No accordion behavior code changed.
- Preserved the supplied mascot and footage, centered reactions, approval flows, reliability UX, separate Connector chats and current mobile layout. No private mobile-reference content was copied. Protocol, session contracts, adapters, fixtures, composition root, assets and original locks are unchanged from the approvals/mascot build.

See docs/DESIGN-SYSTEM.md and docs/DESIGN-SYSTEM-TEST-RESULTS.md for scope, actual checks and environment limitations.

# Approvals and real mascot / 1.10.0

- Fixed the reaction picker's More reactions button with a centered SVG plus, independent of text baselines.
- Replaced all default mascot variants, the full-resolution image sample, the video thumbnail/poster, and the actual sample video with the supplied Hatchling artwork. Existing custom avatars remain user-controlled.
- Added above-composer task approval cards and shared pending/resolved records in Approvals history and Activity. Scope is visible; decisions are Allow once, Allow for task when requested, or Deny. No permanent-grant option was added.
- Added safe pending, submitting, rejected-save, unconfirmed-decision, expired, offline, multiple-request and chat-isolation behavior. Status checks do not resend decisions; deleting a chat is blocked while a task approval is pending.
- Surfaced existing browser challenge and exact-quote purchase approvals above the composer without replacing their domain-specific controls. Purchase review still opens the complete quote.
- Added an optional, review-required taskApprovals capability, local scenarios, 18 unit cases and 22 browser cases. Kept all four frozen protocol modules, main.ts and both original lock files unchanged.
- Preserved reliability/draft/outbox flows, phone layouts, text-size settings and the separate Connector chats section. No live VM, network, permission service or payment backend was added.

# v10 UI touch-up / 1.9.0

## Reliability UX revision (package 1.9.0 unchanged)

- Added inline history loading/error/retry, cached-transcript warnings, VM/offline/session states, and a visible per-chat outbox with honest delivery outcomes.
- Added safe per-file retry, explicit uncertain-delivery reconciliation/dismissal, per-tab draft/outbox recovery, and guards against losing unsent work during navigation or deletion.
- Moved WhatsApp and Messenger into a separate bottom Connector chats section with unread and connection indicators.
- Added a deterministic edge-case preview group, an optional review-required frontend session capability, and dedicated automated coverage. No live backend or provider integration added.
- Kept the original protocol and presentation lock files unchanged; the old presentation audit intentionally reports this requested revision. See docs/RELIABILITY-UX.md.


- Refined neutral dark surfaces, sidebar proportions, grouped messages, composer alignment, and panel/dialog density against the supplied examples.
- Moved dashed choice rows inside response bubbles, corrected reaction-picker direction, and aligned message actions to real cards with narrow-conversation reflow.
- Restyled file, browser and scheduled-message cards; anchored the "See more" control above the current composer height; represented existing unread state in the Chats pill.
- Preserved the actual evidence, local report-download and message commands. Included the same styling in served and standalone builds.
- Added eight browser regression cases and reproducible screenshots. Protocol, adapters, fixtures, composition root, public media, dependencies and original locks are unchanged.

See `docs/UI-TOUCHUP.md` and the current `docs/TEST-RESULTS.md`. The historical v10 notes below are retained for context.

# v10 / 1.9.0

- Matched the profile's 100 px portrait, 23 px name, 18 px Connected line and 14 px visible green mark more closely to the supplied crop. The status has a dedicated muted token; general muted text still meets its existing contrast targets.
- Added staggered name/status return, changed-label transitions and stable profile DOM during streaming. Rapid toggles, focus, long names, mobile layouts and reduced-motion changes are tested.
- Replaced screenshot-derived transcript, research/task data, feed stories/images, browser screenshots and website grants with synthetic examples. No raw source-reference archive is distributed. Old local demo events are not replayed into the new fixture namespace.
- Moved the scripted bubble delimiter out of chat rendering into adapter snapshot normalization. Strengthened type-only presentation imports without changing any of the four protocol files or the main composition root.
- Added asset provenance/hash review, lexical asset regression checks, direct negative AST boundary tests and release capture tooling. Restored the generated sample video asset.
- Renamed the enclosing delivery directory to frontend/ for parent-repository integration. No live adapter was added.

Earlier builds are superseded, not retroactively cleaned. This release contains current synthetic evidence only; old reference-derived screenshots and reports are intentionally omitted.
