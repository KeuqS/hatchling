import type { AppContext } from '../app/context.js';
import type { ContentResource } from '../app/content-loading.js';
import { h } from './dom.js';
import { icon } from './icons.js';
import { button } from './ui.js';
import { loadingRegion } from './skeleton.js';
const names: Record<ContentResource, string> = { ideas: 'ideas', feed: 'feed', library: 'artifacts' };
/** Returns true only when the not-yet-loaded content must be withheld. Cached rows remain readable. */
export function appendContentState(ctx: AppContext, host: HTMLElement, resource: ContentResource): boolean {
    const state = ctx.contentLoading.read(resource);
    if (state.phase === 'ready' || !state.visible)
        return false;
    const name = names[resource];
    const loading = state.phase === 'loading';
    const title = loading ? state.slow ? 'Taking longer than usual' : state.cached ? `Updating your ${name}` : `Loading your ${name}` : state.cached ? `Couldn’t update your ${name}` : `Your ${name} couldn’t load`;
    const notice = h('section', { class: `content-state ${state.cached ? 'content-state-cached' : ''} ${loading && !state.cached && !state.slow ? 'content-state-initial' : ''}`, 'data-content-resource': resource, 'data-content-phase': state.phase }, h('div', { class: 'content-state-copy', role: 'status', 'aria-live': 'polite' }, icon(loading ? 'clock' : 'warning', 17), h('div', {}, h('strong', {}, title), state.cached || !loading || state.slow ? h('p', {}, state.cached ? 'Your last loaded content stays visible.' : loading ? 'You can keep using the rest of the workspace.' : 'Try again. Your other work is still here.') : null)));
    if (!loading)
        notice.append(button(`Retry loading ${name}`, () => {
            if (state.demo)
                ctx.contentLoading.preview(resource, state.cached ? 'refresh' : 'loading');
            else if (resource === 'feed')
                void ctx.run({ type: 'feed.generate' });
        }, { icon: 'refresh', variant: 'ghost' }));
    if (state.demo)
        notice.append(h('div', { class: 'content-preview-caption' }, h('span', {}, 'Loading preview · local demo'), button('End preview', () => ctx.contentLoading.endPreview(resource), { variant: 'ghost', id: `end-loading-preview-${resource}` })));
    host.append(notice);
    if (loading && !state.cached) {
        const region = loadingRegion(`Loading ${name}`, resource, ctx.ui.libraryLayout);
        // The visible notice already announces this region; don't announce each row twice.
        region.querySelector('[role="status"]')?.remove();
        host.append(region);
    }
    return !state.cached;
}
