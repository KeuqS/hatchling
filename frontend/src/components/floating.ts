import { h } from './dom.js';
export interface Rectangle {
    left: number;
    top: number;
    width: number;
    height: number;
}
/** Viewport-clamped placement, independent of ancestor overflow or modal clipping. */
export function floatingPosition(anchor: Rectangle, size: {
    width: number;
    height: number;
}, viewport: {
    width: number;
    height: number;
}, prefer: 'above' | 'right' = 'above'): {
    left: number;
    top: number;
} {
    const gap = 8;
    const clamp = (value: number, max: number): number => Math.max(gap, Math.min(value, Math.max(gap, max)));
    let left = anchor.left + (anchor.width - size.width) / 2;
    let top = anchor.top - size.height - gap;
    if (prefer === 'right' && anchor.left + anchor.width + gap + size.width < viewport.width - gap) {
        left = anchor.left + anchor.width + gap;
        top = anchor.top + (anchor.height - size.height) / 2;
    }
    else if (top < gap)
        top = anchor.top + anchor.height + gap;
    return { left: clamp(left, viewport.width - size.width - gap), top: clamp(top, viewport.height - size.height - gap) };
}
/** One delegated tooltip in the top layer. It also works inside clipped native dialogs. */
export function installTooltips(): () => void {
    const tip = h('div', { id: 'workspace-tooltip', class: 'floating-tooltip', role: 'tooltip', popover: 'manual' });
    document.body.append(tip);
    document.documentElement.classList.add('portal-tooltips');
    let owner: HTMLElement | null = null;
    let timer = 0;
    let keyboard = false;
    const hide = (): void => {
        window.clearTimeout(timer);
        if (owner) {
            const described = (owner.getAttribute('aria-describedby') ?? '').split(' ').filter((id) => id && id !== tip.id);
            if (described.length)
                owner.setAttribute('aria-describedby', described.join(' '));
            else
                owner.removeAttribute('aria-describedby');
        }
        owner = null;
        if (tip.matches(':popover-open'))
            tip.hidePopover();
        tip.classList.remove('fallback-visible');
    };
    const show = (node: HTMLElement): void => {
        hide();
        owner = node;
        timer = window.setTimeout(() => {
            if (owner !== node || !node.isConnected || node.closest('details[open] > summary'))
                return;
            tip.textContent = node.dataset['tooltip'] ?? '';
            if (!tip.textContent)
                return;
            // Top-layer children retain the active dialog's accessibility subtree.
            (node.closest('dialog') ?? document.body).append(tip);
            if (typeof tip.showPopover === 'function')
                tip.showPopover();
            else
                tip.classList.add('fallback-visible');
            const position = floatingPosition(node.getBoundingClientRect(), tip.getBoundingClientRect(), { width: innerWidth, height: innerHeight }, node.closest('.icon-rail') ? 'right' : 'above');
            tip.style.left = `${position.left}px`;
            tip.style.top = `${position.top}px`;
            const ids = new Set((node.getAttribute('aria-describedby') ?? '').split(' ').filter(Boolean));
            ids.add(tip.id);
            node.setAttribute('aria-describedby', [...ids].join(' '));
        }, 350);
    };
    const enter = (event: Event): void => {
        if (event instanceof FocusEvent && !keyboard)
            return;
        if (event instanceof PointerEvent && event.pointerType === 'touch')
            return;
        const target = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-tooltip]') : null;
        if (target && target !== owner)
            show(target);
    };
    const leave = (event: Event): void => {
        const related = event instanceof FocusEvent || event instanceof MouseEvent ? event.relatedTarget : null;
        if (owner && related instanceof Node && owner.contains(related))
            return;
        hide();
    };
    const keydown = (event: KeyboardEvent): void => {
        keyboard = true;
        if (event.key === 'Escape')
            hide();
    };
    const pointerdown = (): void => { keyboard = false; hide(); };
    document.addEventListener('pointerover', enter);
    document.addEventListener('focusin', enter);
    document.addEventListener('pointerout', leave);
    document.addEventListener('focusout', leave);
    document.addEventListener('pointerdown', pointerdown, true);
    document.addEventListener('scroll', hide, true);
    document.addEventListener('keydown', keydown, true);
    window.addEventListener('resize', hide);
    // UI refreshes may remove a hovered control without dispatching pointerout.
    const observer = new MutationObserver(() => {
        if (owner && !owner.isConnected)
            hide();
    });
    observer.observe(document.body, { childList: true, subtree: true });
    return () => {
        hide();
        observer.disconnect();
        tip.remove();
        document.removeEventListener('pointerover', enter);
        document.removeEventListener('focusin', enter);
        document.removeEventListener('pointerout', leave);
        document.removeEventListener('focusout', leave);
        document.removeEventListener('pointerdown', pointerdown, true);
        document.removeEventListener('scroll', hide, true);
        document.removeEventListener('keydown', keydown, true);
        window.removeEventListener('resize', hide);
    };
}
/** Keep action menus in the top layer without detaching them from their owner. */
function placeMenu(details: HTMLDetailsElement, content: HTMLElement): void {
    const trigger = details.querySelector('summary');
    if (!trigger)
        return;
    const anchor = trigger.getBoundingClientRect();
    const rect = content.getBoundingClientRect();
    const gap = 8;
    const preferredLeft = details.closest('.icon-rail') ? anchor.right + gap : anchor.right - rect.width;
    const left = Math.max(gap, Math.min(preferredLeft, innerWidth - rect.width - gap));
    const below = anchor.bottom + gap;
    const top = below + rect.height <= innerHeight - gap ? below : Math.max(gap, anchor.top - rect.height - gap);
    content.style.setProperty('--floating-left', `${left}px`);
    content.style.setProperty('--floating-top', `${top}px`);
}
/** Synchronous opening lets the first keyboard action focus a visible item. */
export function setFloatingMenuOpen(details: HTMLDetailsElement, open: boolean): void {
    const content = details.querySelector<HTMLElement>('.menu-popover');
    const summary = details.querySelector('summary');
    details.open = open;
    summary?.setAttribute('aria-expanded', String(open));
    if (!content)
        return;
    content.dataset['floating'] = 'true';
    if (open && details.isConnected) {
        for (const other of document.querySelectorAll<HTMLDetailsElement>('details.menu[open]'))
            if (other !== details && !other.contains(details))
                setFloatingMenuOpen(other, false);
        if (typeof content.showPopover === 'function') {
            content.setAttribute('popover', 'manual');
            if (!content.matches(':popover-open'))
                content.showPopover();
        }
        placeMenu(details, content);
    }
    else if (typeof content.hidePopover === 'function' && content.matches(':popover-open')) {
        content.hidePopover();
    }
}
export function installFloatingMenus(): () => void {
    // Labels and asynchronously populated actions can change an open menu's size.
    // Re-anchor after layout, not just when opening, so it stays inside the viewport.
    const observed = new Map<HTMLElement, HTMLDetailsElement>();
    const sizes = typeof ResizeObserver === 'function' ? new ResizeObserver((entries) => {
        for (const entry of entries) {
            if (!(entry.target instanceof HTMLElement))
                continue;
            const details = observed.get(entry.target);
            if (details?.isConnected && details.open)
                placeMenu(details, entry.target);
        }
    }) : null;
    const prune = (): void => {
        for (const [content, details] of observed) {
            if (!details.isConnected || !details.open || !content.isConnected) {
                sizes?.unobserve(content);
                observed.delete(content);
            }
        }
    };
    const removals = new MutationObserver(prune);
    removals.observe(document.body, { childList: true, subtree: true });
    const toggle = (event: Event): void => {
        const details = event.target;
        if (details instanceof HTMLDetailsElement && details.classList.contains('menu')) {
            setFloatingMenuOpen(details, details.open);
            prune();
            const content = details.querySelector<HTMLElement>('.menu-popover');
            if (sizes && content && details.open && details.isConnected && !observed.has(content)) {
                observed.set(content, details);
                sizes.observe(content);
            }
        }
    };
    const close = (): void => {
        for (const details of document.querySelectorAll<HTMLDetailsElement>('details.menu[open]'))
            setFloatingMenuOpen(details, false);
    };
    const scroll = (event: Event): void => {
        for (const details of document.querySelectorAll<HTMLDetailsElement>('details.menu[open]')) {
            const content = details.querySelector<HTMLElement>('.menu-popover');
            if (!content || (event.target instanceof Node && content.contains(event.target)))
                continue;
            const anchor = details.querySelector('summary')?.getBoundingClientRect();
            if (!anchor || anchor.bottom <= 0 || anchor.top >= innerHeight || anchor.right <= 0 || anchor.left >= innerWidth)
                setFloatingMenuOpen(details, false);
            else
                placeMenu(details, content);
        }
    };
    document.addEventListener('toggle', toggle, true);
    document.addEventListener('scroll', scroll, true);
    window.addEventListener('resize', close);
    return () => {
        close();
        sizes?.disconnect();
        removals.disconnect();
        observed.clear();
        document.removeEventListener('toggle', toggle, true);
        document.removeEventListener('scroll', scroll, true);
        window.removeEventListener('resize', close);
    };
}
