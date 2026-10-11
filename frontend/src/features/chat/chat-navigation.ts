import type { AppContext } from '../../app/context.js';
import { h } from '../../components/dom.js';
import { icon } from '../../components/icons.js';
import { resizeHandle } from '../../components/resize.js';
/** View-only navigation state; pinning is still a separate adapter preference. */
export function setChatNavigationOpen(ctx: AppContext, open: boolean, moveFocus = false): void {
    ctx.ui.navOpen = open;
    ctx.render();
    if (moveFocus) {
        const target = document.getElementById(open ? 'chat-search' : 'chat-navigation-toggle');
        target?.focus({ preventScroll: true });
    }
}
/** Adapter clock, never Date.now(): fixture recency stays deterministic and does not invent missing dates. */
export function relativeChatTime(updatedAt: string | undefined, clock: string): string {
    if (!updatedAt)
        return '';
    const elapsed = Date.parse(clock) - Date.parse(updatedAt);
    if (!Number.isFinite(elapsed) || elapsed < 0)
        return '';
    const minutes = Math.floor(elapsed / 60000);
    return minutes < 1 ? 'Now' : minutes < 60 ? `${minutes}m` : minutes < 1440 ? `${Math.floor(minutes / 60)}h` : `${Math.floor(minutes / 1440)}d`;
}
export function setSideChatsOpen(ctx: AppContext, open: boolean): void {
    ctx.ui.sideChatsOpen = open;
    document.querySelector('.side-chat-disclosure')?.setAttribute('aria-expanded', String(open));
    const accordion = document.getElementById('side-chat-accordion');
    accordion?.classList.toggle('is-collapsed', !open);
    if (accordion)
        accordion.inert = !open;
}
export function sideChatDisclosure(ctx: AppContext): HTMLElement {
    const node = h('button', { type: 'button', class: 'side-chat-disclosure', 'aria-label': 'Side chats', 'aria-expanded': ctx.ui.sideChatsOpen, 'aria-controls': 'side-chat-accordion' }, h('span', {}, ctx.ui.showArchived ? 'Archived chats' : 'Side chats'), h('span', { class: 'disclosure-arrow', 'aria-hidden': 'true' }, icon('chevron', 13)));
    node.addEventListener('click', () => setSideChatsOpen(ctx, !ctx.ui.sideChatsOpen));
    return node;
}
export function navigationResizer(ctx: AppContext): HTMLElement {
    const apply = (width: number): void => { ctx.ui.navigationWidth = width; document.documentElement.style.setProperty('--nav-width', `${width}px`); };
    const close = (): void => setChatNavigationOpen(ctx, false, true);
    const handle = resizeHandle({ label: 'Side chats width', className: 'navigation-resizer', value: () => document.querySelector('.navigation-panel')?.getBoundingClientRect().width ?? ctx.ui.navigationWidth,
        bounds: () => [150, Math.min(420, innerWidth * .4)], change: apply,
        commit: () => {
            if (ctx.ui.navigationWidth < 185) {
                apply(240);
                close();
            }
        }, click: close });
    handle.dataset['tooltip'] = 'Side chats\nClick or drag to close';
    return handle;
}
