import { animationLibrary } from './animations.js';
import { IconAnimation } from './lottie-player.js';
import type { IconName } from '../protocol/models.js';
const players = new Map<SVGSVGElement, IconAnimation>();
/** Original Lottie-format assets, with a small typed SVG player. Never uses the reference vector data. */
export function bindIconMotion(button: HTMLButtonElement, playOnMount = false): void {
    const fallback = button.querySelector('svg');
    const name = (['chat', 'feed', 'goal', 'idea', 'library', 'search', 'arrow', 'spark'] satisfies IconName[]).find((key) => fallback?.classList.contains(`glyph-${key}`));
    const data = name ? animationLibrary[name] : undefined;
    if (!fallback || !name || !data)
        return;
    const player = new IconAnimation(data, name);
    fallback.replaceWith(player.element);
    players.set(player.element, player);
    button.addEventListener('pointerenter', player.play);
    button.addEventListener('focus', player.play);
    if (playOnMount)
        player.play();
}
export function disposeIconMotion(root: HTMLElement): void {
    for (const [element, player] of players)
        if (root.contains(element)) {
            player.destroy();
            players.delete(element);
        }
}
