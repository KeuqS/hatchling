import type { AppState, DeepReadonly, IconName } from '../../protocol/models.js';
export interface SearchResult {
    id: string;
    kind: 'chat' | 'artifact' | 'goal' | 'memory';
    title: string;
    preview: string;
    time: string;
    icon: IconName;
}
/** Pure selector: deterministic recents, transcript search, and bounded workspace results. */
export function searchWorkspace(state: DeepReadonly<AppState>, input: string, recentIds: readonly string[] = ['reference', 'chat-0', 'main', 'chat-1', 'chat-2', 'chat-3']): SearchResult[] {
    const query = input.trim().toLocaleLowerCase();
    const results: SearchResult[] = [];
    const chats = query ? state.chats.filter((chat) => !chat.archived) : recentIds.flatMap((id) => state.chats.filter((chat) => chat.id === id && !chat.archived));
    for (const chat of chats) {
        const messages = state.messages.filter((message) => message.chatId === chat.id && 'text' in message);
        const titleMatch = chat.title.toLocaleLowerCase().includes(query);
        const message = query && !titleMatch ? messages.findLast((item) => 'text' in item && item.text.toLocaleLowerCase().includes(query)) : messages.at(-1);
        if (query && !titleMatch && !message)
            continue;
        results.push({ id: chat.id, kind: 'chat', title: chat.title, preview: message && 'text' in message ? message.text.replace(/\*\*|`/g, '').replace(/\s+/g, ' ') : 'No messages yet', time: message?.time ?? '', icon: 'chat' });
    }
    if (!query)
        return results;
    for (const item of state.artifacts)
        if (!item.trashUntil && `${item.title} ${item.content}`.toLocaleLowerCase().includes(query))
            results.push({ id: item.id, kind: 'artifact', title: item.title, preview: item.description, time: item.format, icon: 'file' });
    for (const item of state.goals)
        if (`${item.title} ${item.description}`.toLocaleLowerCase().includes(query))
            results.push({ id: item.id, kind: 'goal', title: item.title, preview: item.pulse, time: 'Goal', icon: 'goal' });
    for (const item of state.memory)
        if (`${item.title} ${item.body}`.toLocaleLowerCase().includes(query))
            results.push({ id: item.id, kind: 'memory', title: item.title, preview: item.body.replace(/\s+/g, ' '), time: 'Memory', icon: 'memory' });
    return results.slice(0, 40);
}
