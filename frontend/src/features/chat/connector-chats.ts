import type { AppContext } from '../../app/context.js';
import type { Channel, Chat, DeepReadonly } from '../../protocol/models.js';
import { h } from '../../components/dom.js';
import { icon } from '../../components/icons.js';
import { button, iconButton, menu, pill } from '../../components/ui.js';
export function channelStatus(channel: DeepReadonly<Channel> | undefined): {
    label: string;
    active: boolean;
    attention: boolean;
} {
    if (!channel || channel.state === 'unlinked')
        return { label: 'Not connected', active: false, attention: true };
    if (channel.state === 'unavailable')
        return { label: 'Unavailable', active: false, attention: true };
    if (channel.state === 'checking')
        return { label: 'Checking connection', active: false, attention: false };
    if (channel.state === 'link_pending')
        return { label: 'Finish connecting', active: false, attention: true };
    return { label: channel.polling ? 'Listening' : 'Updates paused', active: channel.polling, attention: !channel.polling };
}
export function chatGroups(chats: readonly DeepReadonly<Chat>[], archived: boolean): {
    side: readonly DeepReadonly<Chat>[];
    connectors: readonly DeepReadonly<Chat>[];
} {
    const visible = chats.filter(chat => chat.id !== 'main' && chat.archived === archived);
    return { side: visible.filter(chat => !chat.channelId).toSorted((a, b) => Number(b.pinned) - Number(a.pinned)), connectors: visible.filter(chat => !!chat.channelId).toSorted((a, b) => Number(b.pinned) - Number(a.pinned)) };
}
export function connectorChats(ctx: AppContext): HTMLElement {
    const chats = chatGroups(ctx.state.chats, ctx.ui.showArchived).connectors;
    const unread = chats.filter(chat => chat.unread).length;
    const expanded = ctx.ui.connectorChatsOpen;
    const heading = h('button', { type: 'button', id: 'connector-chat-toggle', class: 'connector-chat-disclosure', 'aria-expanded': expanded, 'aria-controls': 'connector-chat-list', 'aria-label': 'Connector chats' }, h('span', {}, ctx.ui.showArchived ? 'Archived connectors' : 'Connector chats'), unread ? h('span', { class: 'connector-unread-count', 'aria-label': `${unread} unread connector chats` }, unread) : null, h('span', { class: 'disclosure-arrow', 'aria-hidden': 'true' }, icon('chevron', 13)));
    heading.addEventListener('click', () => { ctx.ui.connectorChatsOpen = !ctx.ui.connectorChatsOpen; ctx.render(); document.getElementById('connector-chat-toggle')?.focus({ preventScroll: true }); });
    const list = h('div', { class: 'connector-chat-list', id: 'connector-chat-list', hidden: !expanded, 'data-scroll': 'connector-chats' });
    for (const chat of chats) {
        const channel = ctx.state.channels.find(item => item.id === chat.channelId);
        const status = ctx.delivery.connection.kind === 'ready' ? channelStatus(channel) : { label: 'Waiting for connection', active: false, attention: false };
        const entry = h('button', { type: 'button', id: `chat-link-${encodeURIComponent(chat.id)}`, class: 'connector-chat-link', 'aria-current': chat.id === ctx.ui.chatId ? 'page' : 'false', 'aria-label': `${chat.title}, ${status.label}${chat.unread ? ', unread' : ''}` }, h('span', { class: `connector-chat-symbol ${status.active ? 'is-listening' : ''}`, 'aria-hidden': 'true' }, icon(chat.channelId === 'whatsapp' ? 'phone' : 'chat', 18)), h('span', { class: 'connector-chat-copy' }, h('strong', {}, chat.title), h('span', { class: status.attention ? 'connector-state attention' : 'connector-state' }, h('i', { class: status.active ? 'connector-status-dot active' : 'connector-status-dot', 'aria-hidden': 'true' }), status.label)), chat.unread ? h('span', { class: 'unread-dot', 'aria-label': 'Unread' }) : null);
        entry.addEventListener('click', () => ctx.navigate('chat', chat.id));
        const options = menu(`Manage connector chat ${chat.title}`, [{ label: 'Manage connection', icon: 'link', action: () => ctx.openSettings('channels') }, { label: chat.archived ? 'Restore chat' : 'Archive chat', icon: 'folder', action: () => { void ctx.run({ type: 'chat.archive', id: chat.id }); } }]);
        list.append(h('div', { class: `connector-chat-row ${chat.id === ctx.ui.chatId ? 'active' : ''}` }, entry, options));
    }
    if (!chats.length)
        list.append(h('div', { class: 'connector-empty' }, h('p', {}, ctx.ui.showArchived ? 'No archived connector chats.' : 'Connected app conversations will live here.'), !ctx.ui.showArchived ? button('Connect an app', () => ctx.openSettings('channels'), { variant: 'link' }) : null));
    return h('section', { class: 'connector-chat-section', 'aria-label': 'Connector chats' }, h('div', { class: 'connector-chat-heading' }, heading, iconButton('settings', 'Manage connector chats', () => ctx.openSettings('channels'))), expanded ? h('p', { class: 'connector-section-caption' }, 'Messages & service updates') : null, list);
}
export function connectorBanner(ctx: AppContext, chat: DeepReadonly<Chat>): HTMLElement {
    const channel = ctx.state.channels.find(item => item.id === chat.channelId);
    const status = channelStatus(channel);
    const detail = status.active ? `Replies stay in ${channel?.name ?? chat.title}. This chat is separate from Main and side chats.` : `${status.label}. Earlier messages remain available here; new updates may be missing.`;
    return h('section', { class: `connector-conversation-banner${status.attention ? ' needs-attention' : ''}` }, icon('link', 17), h('div', { class: 'connection-copy' }, h('strong', {}, `${chat.title} · Connector chat`), h('p', {}, detail)), pill('Read-only here'), button('Manage', () => ctx.openSettings('channels'), { variant: 'ghost', id: 'manage-chat-connection' }));
}
