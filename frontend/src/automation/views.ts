import type { AppContext } from '../app/context.js';
import { isRecord } from '../protocol/codec.js';
import { containsSensitiveValue } from '../protocol/rules.js';
import { setContextOpen } from '../features/context/presence.js';
import { selectContextTab } from '../features/context/panel.js';
import { setSideChatsOpen } from '../features/chat/chat-navigation.js';
import { routes } from './catalog.js';
/** Semantic UI operations shared with controls; no coordinate clicking, DOM-id API or eval. */
export function executeView(ctx: AppContext, value: unknown): string | null {
    if (!isRecord(value))
        return 'Invalid view action.';
    const id = typeof value['id'] === 'string' ? value['id'] : '';
    switch (value['type']) {
        case 'ui.navigate': {
            const route = routes.find(item => item === value['route']);
            if (!route)
                return 'Unknown workspace area.';
            const detail = typeof value['detail'] === 'string' ? value['detail'] : undefined;
            if (route === 'chat' && detail && !ctx.state.chats.some(chat => chat.id === detail))
                return 'Conversation not found.';
            ctx.navigate(route, detail);
            return null;
        }
        case 'ui.settings':
            ctx.openSettings(typeof value['section'] === 'string' ? value['section'] : undefined);
            return null;
        case 'ui.context': {
            const tab = value['tab'];
            if (tab !== 'activity' && tab !== 'approvals' && tab !== 'schedule' && tab !== 'identity')
                return 'Unknown context tab.';
            setContextOpen(ctx, value['open'] === true);
            selectContextTab(ctx, tab);
            return null;
        }
        case 'ui.side-chats':
            setSideChatsOpen(ctx, value['open'] === true);
            return null;
        case 'ui.prefill': {
            const text = value['text'];
            if (typeof text !== 'string' || text.length > 10000 || containsSensitiveValue(text))
                return 'Use a short draft without sensitive values.';
            ctx.prefillComposer(text);
            return null;
        }
        case 'ui.document':
            if (!ctx.state.memory.some(item => item.id === id))
                return 'Document not found.';
            ctx.openMemoryDocument(id);
            return null;
        case 'ui.artifact':
            if (!ctx.state.artifacts.some(item => item.id === id && !item.trashUntil))
                return 'Artifact not found.';
            ctx.openArtifact(id);
            return null;
        case 'ui.activity':
            if (!ctx.state.threads.some(item => item.id === id))
                return 'Activity not found.';
            ctx.openThread(id);
            return null;
        case 'ui.browser':
            if (!ctx.state.browserTasks.some(item => item.id === id))
                return 'Browser task not found.';
            ctx.openBrowser(id);
            return null;
        case 'ui.close-workspace':
            ctx.dock.close();
            return null;
        case 'ui.search':
            ctx.openSearch();
            return null;
        default: return 'Unknown view action.';
    }
}
