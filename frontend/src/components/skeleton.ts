import { h } from './dom.js';
export type SkeletonKind = 'ideas' | 'feed' | 'library' | 'document' | 'chat';
/** Decorative only. Put one named status beside the busy region, never on every line. */
export function skeletonBlock(shape: 'line' | 'title' | 'avatar' | 'media' = 'line', width = 100): HTMLElement {
    const safeWidth = Number.isFinite(width) ? Math.min(100, Math.max(10, width)) : 100;
    return h('span', { class: `skeleton-block skeleton-${shape}`, 'aria-hidden': true, style: `--skeleton-line-width:${safeWidth}%` });
}
export function skeletonText(widths: readonly number[] = [96, 90, 65]): HTMLElement {
    return h('div', { class: 'skeleton-text', 'aria-hidden': true }, ...widths.map(width => skeletonBlock('line', width)));
}
/** Fixed patterns, not random values: dimensions are stable across renders. */
export function skeletonContent(kind: SkeletonKind, layout: 'grid' | 'list' = 'grid'): HTMLElement {
    const body = h('div', { class: `skeleton-content skeleton-content-${kind}`, 'aria-hidden': true });
    if (kind === 'chat') {
        body.append(h('div', { class: 'skeleton-chat-agent' }, skeletonText([90, 100, 64])), h('div', { class: 'skeleton-chat-user' }, skeletonText([100])), h('div', { class: 'skeleton-chat-agent' }, skeletonText([95, 73])));
    }
    else if (kind === 'document') {
        body.append(skeletonBlock('title', 65), skeletonText([100, 97, 100, 76]), skeletonBlock('title', 44), skeletonText([95, 100, 92, 65]), skeletonText([100, 90, 68]));
    }
    else if (kind === 'library') {
        body.classList.add(layout === 'grid' ? 'artifact-grid' : 'artifact-list');
        for (let index = 0; index < 4; index++)
            body.append(h('div', { class: 'skeleton-artifact' }, skeletonBlock('media'), h('div', { class: 'skeleton-artifact-meta' }, skeletonBlock('avatar'), h('div', {}, skeletonBlock('title', 75), skeletonText([58])))));
    }
    else {
        for (let index = 0; index < (kind === 'ideas' ? 4 : 3); index++)
            body.append(h('div', { class: 'skeleton-editorial-row' }, skeletonBlock('avatar'), h('div', { class: 'skeleton-row-copy' }, skeletonBlock('title', index % 2 ? 90 : 80), skeletonText(kind === 'ideas' ? [96, 100, 72] : [100, 97, 100, 88, 64]), kind === 'feed' && index === 0 ? skeletonBlock('media') : null)));
    }
    return body;
}
export function loadingRegion(label: string, kind: SkeletonKind, layout: 'grid' | 'list' = 'grid'): HTMLElement {
    // A busy live region may defer speech. Keep the status OUTSIDE that region.
    return h('div', { class: 'loading-region' }, h('p', { class: 'sr-only', role: 'status', 'aria-live': 'polite' }, label), h('div', { 'aria-busy': true, 'aria-label': label, role: 'region' }, skeletonContent(kind, layout)));
}
