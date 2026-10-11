import type { ChatRecovery, SavedDraft, SavedSend } from '../session/chat-session.js';
import { parseCommand } from '../protocol/codec.js';
function object(value: unknown): value is Record<string, unknown> { return value !== null && typeof value === 'object' && !Array.isArray(value); }
/** Validate restored tab data before it can ever reach an adapter command. */
export function parseRecovery(raw: string | null): ChatRecovery | null {
    if (!raw || raw.length > 1000000)
        return null;
    try {
        const value: unknown = JSON.parse(raw);
        if (!object(value) || value['version'] !== 1 || !Array.isArray(value['drafts']) || !Array.isArray(value['outbox']) || value['drafts'].length > 200 || value['outbox'].length > 30)
            return null;
        const drafts: SavedDraft[] = [];
        for (const draft of value['drafts']) {
            if (object(draft) && typeof draft['chatId'] === 'string' && typeof draft['text'] === 'string' && Array.isArray(draft['attachments']) && draft['attachments'].every((id: unknown) => typeof id === 'string'))
                drafts.push({ chatId: draft['chatId'], text: draft['text'], attachments: draft['attachments'].filter((id: unknown): id is string => typeof id === 'string') });
        }
        const outbox: SavedSend[] = [];
        const ids = new Set<string>();
        for (const item of value['outbox']) {
            if (!object(item) || typeof item['id'] !== 'string' || typeof item['batchId'] !== 'string' || typeof item['detail'] !== 'string' || ids.has(item['id']))
                continue;
            const command = parseCommand(item['command']);
            const phase = item['phase'];
            if (!command || (command.type !== 'chat.send' && command.type !== 'chat.attach') || !['queued', 'sending', 'failed', 'unconfirmed'].includes(String(phase)))
                continue;
            ids.add(item['id']);
            // A page disappearing during a request does not establish that it failed.
            outbox.push({ id: item['id'], batchId: item['batchId'], command, phase: phase === 'failed' ? 'failed' : phase === 'queued' ? 'queued' : 'unconfirmed', detail: item['detail'] });
        }
        return { version: 1, drafts, outbox };
    }
    catch {
        return null;
    }
}
