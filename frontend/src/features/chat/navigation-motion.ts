/** Only route/disclosure changes animate. Streaming snapshots do not replay entrance motion. */
export function animateChatNavigation(root: HTMLElement, previousChat: string | undefined, previousSidebar: boolean): void {
    if (matchMedia('(prefers-reduced-motion: reduce)').matches)
        return;
    const toolbar = root.querySelector<HTMLElement>('.conversation-toolbar');
    if (previousChat !== undefined && toolbar && toolbar.dataset['chatId'] !== previousChat) {
        toolbar.querySelector('.conversation-location')?.animate([
            { opacity: .35, transform: 'translateY(-3px)' },
            { opacity: 1, transform: 'translateY(0)' },
        ], { duration: 160, easing: 'cubic-bezier(.2,.8,.2,1)' });
    }
    if (!previousSidebar) {
        root.querySelector('.route-chat .navigation-panel')?.animate([
            { opacity: .3, transform: 'translateX(-10px)' },
            { opacity: 1, transform: 'translateX(0)' },
        ], { duration: 180, easing: 'cubic-bezier(.2,.8,.2,1)' });
    }
}
