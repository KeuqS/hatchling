import { profileReturn, profileVisibility } from './profile-motion.js';
import type { AppContext } from '../../app/context.js';
import { h } from '../../components/dom.js';
import { agentStatus } from './agent-status.js';
let flight: Animation | null = null;
let ghost: HTMLElement | null = null;
/** Local view transition; neither pausing the agent nor altering its work. */
export function setContextOpen(ctx: AppContext, open: boolean): void {
    if (ctx.ui.contextOpen === open)
        return;
    const source = document.querySelector<HTMLElement>(open ? '.floating-agent-toggle img' : '.agent-portrait > img');
    const from = ghost?.getBoundingClientRect() ?? source?.getBoundingClientRect();
    flight?.cancel();
    ghost?.remove();
    ghost = null;
    profileVisibility(ctx, false);
    ctx.ui.contextOpen = open;
    ctx.render();
    const target = document.querySelector<HTMLElement>(open ? '.agent-portrait > img' : '.floating-agent-toggle img');
    if (!target)
        return;
    if (!matchMedia('(prefers-reduced-motion: reduce)').matches) {
        const to = target.getBoundingClientRect();
        const dx = from ? from.left + from.width / 2 - to.left - to.width / 2 : 0;
        const dy = from ? from.top + from.height / 2 - to.top - to.height / 2 : -8;
        const scale = from && to.width ? from.width / to.width : .85;
        const image = h('img', { class: 'agent-transition-ghost', src: ctx.state.settings.avatar || './media/rail-avatar.png', alt: '', 'aria-hidden': 'true' });
        image.style.left = `${to.left}px`;
        image.style.top = `${to.top}px`;
        image.style.width = `${to.width}px`;
        image.style.height = `${to.height}px`;
        target.style.visibility = 'hidden';
        document.body.append(image);
        ghost = image;
        const animation = image.animate([{ transform: `translate(${dx}px, ${dy}px) scale(${scale})`, opacity: .85 }, { transform: 'translate(0,0) scale(1.06)', opacity: 1, offset: .8 }, { transform: 'translate(0,0) scale(1)', opacity: 1 }], { duration: 360, easing: 'cubic-bezier(.2,.8,.2,1)' });
        flight = animation;
        const reduced = matchMedia('(prefers-reduced-motion: reduce)');
        const reduceMotion = (): void => {
            if (reduced.matches)
                animation.cancel();
        };
        reduced.addEventListener('change', reduceMotion);
        const cleanup = (): void => {
            reduced.removeEventListener('change', reduceMotion);
            target.style.visibility = '';
            image.remove();
            if (ghost === image) {
                ghost = null;
                flight = null;
            }
        };
        void animation.finished.then(cleanup, cleanup);
        if (open)
            document.querySelector('.context-panel')?.animate([{ opacity: .45, transform: 'translateX(22px)' }, { opacity: 1, transform: 'none' }], { duration: 280, easing: 'ease-out' });
    }
    if (open)
        profileReturn(ctx);
    if (open)
        document.querySelector<HTMLElement>('.context-close button')?.focus({ preventScroll: true });
    else
        target.closest<HTMLElement>('button')?.focus({ preventScroll: true });
}
export function avatarSource(ctx: AppContext): string { return ctx.state.settings.avatar || './media/hatchling-avatar.png'; }
export function floatingAgent(ctx: AppContext): HTMLElement {
    const status = agentStatus(ctx.state, ctx.delivery.connection, ctx.approvals.pending());
    const control = h('button', { type: 'button', class: `floating-agent-toggle ${status.kind}`, 'aria-label': 'Show agent status', 'aria-expanded': 'false', 'aria-description': status.label, 'data-tooltip': `${ctx.state.settings.name}
${status.label}` }, h('img', { src: ctx.state.settings.avatar || './media/rail-avatar.png', alt: '', width: 54, height: 54 }));
    control.addEventListener('click', () => {
        if (document.body.classList.contains('has-browser-session'))
            ctx.dock.close();
        setContextOpen(ctx, true);
    });
    return control;
}
