import type { AppState, CommandResult } from '../../protocol/models.js';
import { textMessage } from '../fixtures.js';
export function ok(message: string, id?: string): CommandResult { return id === undefined ? { ok: true, message } : { ok: true, message, id }; }
export function fail(message: string): CommandResult { return { ok: false, message }; }
export function nextId(state: AppState, prefix: string): string { return `${prefix}-${state.revision + 1}`; }
export function say(state: AppState, text: string, chatId = 'main'): void {
    state.messages.push(textMessage(`${nextId(state, 'reply')}-${state.messages.length}`, text, 'agent', chatId));
}
