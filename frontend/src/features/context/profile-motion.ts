import type { AppContext } from '../../app/context.js';
interface ProfileState {
    element: HTMLElement;
    name: string;
    label: string;
    kind: string;
    visible: boolean;
    animations: Animation[];
}
const states = new WeakMap<AppContext, ProfileState>();
function play(element: HTMLElement | null, id: string, delay = 0): Animation | null {
    if (!element || matchMedia('(prefers-reduced-motion: reduce)').matches)
        return null;
    for (const previous of element.getAnimations())
        if (previous.id.startsWith('profile-'))
            previous.cancel();
    const animation = element.animate([{ opacity: 0, transform: 'translateY(6px)' }, { opacity: 1, transform: 'translateY(0)' }], { duration: 260, delay, fill: 'backwards', easing: 'cubic-bezier(.2,.75,.25,1)' });
    animation.id = id;
    const preference = matchMedia('(prefers-reduced-motion: reduce)');
    const cancel = (): void => {
        if (preference.matches)
            animation.cancel();
    };
    preference.addEventListener('change', cancel);
    const cleanup = (): void => preference.removeEventListener('change', cancel);
    void animation.finished.then(cleanup, cleanup);
    return animation;
}
export function profileMounted(ctx: AppContext, profile: HTMLElement, label: string, kind: string): HTMLElement {
    const previous = states.get(ctx);
    if (previous?.visible && previous.element.isConnected) {
        const stable = previous.element;
        const name = stable.querySelector<HTMLElement>('.agent-name');
        if (name && previous.name !== ctx.state.settings.name) {
            name.textContent = ctx.state.settings.name;
            name.title = ctx.state.settings.name;
        }
        const status = stable.querySelector<HTMLElement>('.agent-status');
        const nextStatus = profile.querySelector<HTMLElement>('.agent-status');
        if (status && nextStatus) {
            status.className = nextStatus.className;
            status.title = nextStatus.title;
            const words = status.querySelector('.agent-status-label');
            if (words && previous.label !== label)
                words.textContent = label;
            if (previous.kind !== kind) {
                const glyph = nextStatus.querySelector('svg');
                if (glyph)
                    status.querySelector('svg')?.replaceWith(glyph);
            }
        }
        const portrait = stable.querySelector<HTMLImageElement>('.agent-portrait > img');
        const nextPortrait = profile.querySelector<HTMLImageElement>('.agent-portrait > img');
        if (portrait && nextPortrait) {
            if (portrait.getAttribute('src') !== nextPortrait.getAttribute('src'))
                portrait.src = nextPortrait.src;
            portrait.alt = nextPortrait.alt;
        }
        profile = stable;
    }
    const current: ProfileState = { element: profile, name: ctx.state.settings.name, label, kind, visible: true, animations: previous?.animations ?? [] };
    states.set(ctx, current);
    // Local navigation will handle the entrance explicitly. State updates animate only changed words.
    if (!previous?.visible)
        return profile;
    requestAnimationFrame(() => {
        if (!profile.isConnected)
            return;
        if (previous.name !== current.name) {
            const animation = play(profile.querySelector('.agent-name'), 'profile-name-update');
            if (animation)
                current.animations.push(animation);
        }
        if (previous.label !== label || previous.kind !== kind) {
            const animation = play(profile.querySelector('.agent-status-label'), 'profile-status-update');
            if (animation)
                current.animations.push(animation);
        }
        current.animations = current.animations.filter(animation => animation.playState === 'running' || animation.pending);
    });
    return profile;
}
export function profileVisibility(ctx: AppContext, visible: boolean): void {
    const previous = states.get(ctx);
    if (previous) {
        for (const animation of previous.animations)
            animation.cancel();
        previous.animations = [];
        previous.visible = visible;
    }
}
export function profileReturn(ctx: AppContext): void {
    const state = states.get(ctx);
    if (!state)
        return;
    const name = play(document.querySelector('.agent-name'), 'profile-name-return', 90);
    const status = play(document.querySelector('.agent-status'), 'profile-status-return', 145);
    state.animations = [name, status].filter((animation): animation is Animation => animation !== null);
}
