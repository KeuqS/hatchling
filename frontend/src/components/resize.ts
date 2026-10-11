import { h } from './dom.js';
export interface ResizeOptions {
    label: string;
    value(): number;
    bounds(): readonly [
        number,
        number
    ];
    change(value: number): void;
    commit?(): void;
    click?(): void;
    axis?: 'x' | 'y';
    direction?: 1 | -1;
    className?: string;
}
export function clampSize(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(Math.max(min, max), Number.isFinite(value) ? value : min));
}
/** Pointer capture is local to the mounted handle. Keyboard and pointer share the same bounds. */
export function resizeHandle(options: ResizeOptions): HTMLElement {
    const axis = options.axis ?? 'x';
    const node = h('div', { class: `resize-handle ${options.className ?? ''}`, role: 'separator', tabindex: 0,
        'aria-label': options.label, 'aria-orientation': axis === 'x' ? 'vertical' : 'horizontal' });
    let origin = 0;
    let initial = 0;
    let active: number | null = null;
    let travelled = false;
    const sync = (): void => {
        const [min, max] = options.bounds();
        node.setAttribute('aria-valuemin', String(Math.round(min)));
        node.setAttribute('aria-valuemax', String(Math.round(max)));
        node.setAttribute('aria-valuenow', String(Math.round(options.value())));
    };
    const apply = (value: number): void => { const [min, max] = options.bounds(); options.change(clampSize(value, min, max)); sync(); };
    node.addEventListener('pointerdown', event => {
        if (event.button !== 0)
            return;
        event.preventDefault();
        active = event.pointerId;
        initial = options.value();
        origin = axis === 'x' ? event.clientX : event.clientY;
        travelled = false;
        node.setPointerCapture(event.pointerId);
        node.classList.add('is-resizing');
    });
    node.addEventListener('pointermove', event => {
        if (active !== event.pointerId)
            return;
        const delta = (axis === 'x' ? event.clientX : event.clientY) - origin;
        if (Math.abs(delta) > 3)
            travelled = true;
        if (travelled)
            apply(initial + delta * (options.direction ?? 1));
    });
    const finish = (event: PointerEvent): void => {
        if (active !== event.pointerId)
            return;
        active = null;
        node.classList.remove('is-resizing');
        if (node.hasPointerCapture(event.pointerId))
            node.releasePointerCapture(event.pointerId);
        if (event.type === 'pointercancel')
            apply(initial);
        else if (!travelled)
            options.click?.();
        options.commit?.();
    };
    node.addEventListener('pointerup', finish);
    node.addEventListener('pointercancel', finish);
    node.addEventListener('lostpointercapture', () => { active = null; node.classList.remove('is-resizing'); });
    node.addEventListener('keydown', event => {
        if ((event.key === 'Enter' || event.key === ' ') && options.click) {
            event.preventDefault();
            options.click();
            return;
        }
        const [min, max] = options.bounds();
        const positive = axis === 'x' ? 'ArrowRight' : 'ArrowDown';
        const negative = axis === 'x' ? 'ArrowLeft' : 'ArrowUp';
        if (![positive, negative, 'Home', 'End'].includes(event.key))
            return;
        event.preventDefault();
        apply(event.key === 'Home' ? min : event.key === 'End' ? max : options.value() + (event.key === positive ? 1 : -1) * (options.direction ?? 1) * (event.shiftKey ? 40 : 16));
        options.commit?.();
    });
    sync();
    return node;
}
