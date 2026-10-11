# Frontend decisions

The references inform layout and interaction, not user data. Comfortable is intentionally larger than the original compact screenshots; users can choose Compact or Large. Local installed fonts are preferred, with a system fallback. No proprietary or other font files are distributed.

Main chat is a durable domain identity (`main`), not a magic title. Side chats have their own drafts, scroll and unread state. The header is a location cue; Back explicitly returns to Main. Search opens over the current workspace. Chat-list pinning, visibility, resizing and section collapse are separate states.

The optional right panel separates Activity, Approvals, Schedule and Identity with a persistent sliding selection pill. Trust-related controls remain accessible there. The portrait can collapse to the conversation center without pausing work. A stable profile prevents animation restart on unrelated snapshots; only changed words transition. Reduced motion is immediate and cancels pending text/portrait motion.

Activity uses independently scrolling timeline and Markdown evidence panes, resizable split/window geometry, explicit pending rows, retained failures and child-lane ownership. Missing times are not inferred. Completed work opens its summary; following live is optional and reading an older step does not lose selection.

Artifacts use a right-hand workspace, not a desktop blocking modal. Identity/SOUL/MEMORY can expand to a centered document with the rail retained. Markdown editing is source-based; other formats are read-only or use their documented slide editor. About notices are outside file contents. Drafts survive navigation; stale saves are rejected. Files from an activity restore its context when closed.

Browser cards and the large right-hand browser workspace share adapter task state. The demonstration uses synthetic local frames and controls, never an actual search or remote Chromium session. Stop, Close, takeover and return control are separate. Unknown payments never offer a retry; challenge permission is explicit. A real interactive browser input transport is not expressed sufficiently for implementation yet and requires a reviewed backend mapping/extension.

Goals separate user-owned goals and assistant tracking; category-first creation asks one question at a time. Ideas have scoped offers and explicit outcomes. Feed posts have provenance and share/discuss actions; this release's posts are fictional editorial examples, not real news. Native file-tree rows expand inline. Library trash is recoverable.

Channels, connectors, providers, devices, schedules and notification delivery are deterministic local state machines. No secure-store secrets, real account login, provider card data or live device commands are accepted. Memory import is reviewed; privacy exports are local downloads. Publishing is a consent-gated local snapshot, not public hosting.

A semantic agent-control interface is preferable to a duplicate UI. It shares the same command vocabulary and view methods, is off by default, and preserves human review. The minimal development CLI/MCP companion is not a backend authorization boundary.

## Shared loading and style guide · 1.11.0

The existing app is the design baseline, documented in STYLE_GUIDE.md and referenced by AGENTS.md. New family CSS uses semantic tokens and passes design:check; existing pixel values are not mass-rewritten. Both builds use index.html's stylesheet order. Mobile reference images are directional only; navigation redesign is deferred.

A skeleton means missing content, not an unconditional transition. Cached content stays visible. Real browser image events drive media placeholders; full-view Retry does not nest inside a thumbnail button. Content operation state is view-local; the existing frozen snapshot has no remote collection-readiness fields. Ideas/library/first-feed delays are explicitly labeled developer demonstrations until an authoritative readiness mapping is reviewed. The reusable document skeleton is a specimen, not an excuse to remount an editor.

## 1.12.0 — one action-menu owner and host-independent preview rendering

The user requested the existing More menu as the shared style, not a redesign. Cosmetic menu rules were migrated out of the historical feature cascade into a single `menus.css` family; the layout/position rules and reaction-pill exception remain. This is a shared-component consolidation, not another per-screen override layer. The ownership guard applies to legacy CSS as well as new files.

A real opaque-origin srcdoc frame reproduced a History API SecurityError on route clicks. Navigation already updates UI state before writing history; guarding those optional URL writes lets the same route render without inventing a live backend, navigating the parent, relaxing sandbox/CSP rules or forcing external hosting. The no-script startup explanation is fallback content only.
