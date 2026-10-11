import type { BezierShape, LottieAnimation, LottieLayer, Property } from './lottie-model.js';
const NS = 'http://www.w3.org/2000/svg';
function svgElement<K extends keyof SVGElementTagNameMap>(tag: K, attributes: Record<string, string> = {}): SVGElementTagNameMap[K] {
    const element = document.createElementNS(NS, tag);
    for (const [key, value] of Object.entries(attributes))
        element.setAttribute(key, value);
    return element;
}
function ease(progress: number, x1: number, y1: number, x2: number, y2: number): number {
    const curve = (t: number, a: number, b: number): number => 3 * (1 - t) ** 2 * t * a + 3 * (1 - t) * t * t * b + t ** 3;
    let low = 0, high = 1;
    for (let index = 0; index < 12; index++) {
        const mid = (low + high) / 2;
        if (curve(mid, x1, x2) < progress)
            low = mid;
        else
            high = mid;
    }
    return curve((low + high) / 2, y1, y2);
}
export function sampleProperty(property: Property, frame: number): number[] {
    if (property.a === 0)
        return typeof property.k === 'number' ? [property.k] : property.k;
    let current = property.k[0];
    let next = property.k[1];
    for (const [index, item] of property.k.entries())
        if (item.t <= frame) {
            current = item;
            next = property.k[index + 1];
        }
    if (!current)
        return [0];
    if (!next || current.h === 1 || frame <= current.t)
        return current.s;
    const progress = Math.min(1, Math.max(0, (frame - current.t) / (next.t - current.t)));
    const amount = current.i && current.o ? ease(progress, current.o.x[0] ?? .33, current.o.y[0] ?? 0, current.i.x[0] ?? .67, current.i.y[0] ?? 1) : progress;
    return current.s.map((value, index) => value + ((current.e?.[index] ?? next.s[index] ?? value) - value) * amount);
}
function shapePath(shape: BezierShape): string {
    const first = shape.v[0];
    if (!first)
        return '';
    let d = `M${first[0] ?? 0},${first[1] ?? 0}`;
    const segments = shape.c ? shape.v.length : shape.v.length - 1;
    for (let index = 0; index < segments; index++) {
        const point = shape.v[index] ?? [0, 0], nextIndex = (index + 1) % shape.v.length;
        const next = shape.v[nextIndex] ?? [0, 0], out = shape.o[index] ?? [0, 0], into = shape.i[nextIndex] ?? [0, 0];
        d += ` C${(point[0] ?? 0) + (out[0] ?? 0)},${(point[1] ?? 0) + (out[1] ?? 0)} ${(next[0] ?? 0) + (into[0] ?? 0)},${(next[1] ?? 0) + (into[1] ?? 0)} ${next[0] ?? 0},${next[1] ?? 0}`;
    }
    return d + (shape.c ? ' Z' : '');
}
interface LayerView {
    data: LottieLayer;
    element: SVGGElement;
    paths: SVGGeometryElement[];
}
/** Renders only the authored shape/transform/trim subset. Not a general player for untrusted Lottie files. */
export class IconAnimation {
    readonly element: SVGSVGElement;
    private readonly layers: LayerView[] = [];
    private request = 0;
    private started = 0;
    private disposed = false;
    private readonly reduced = window.matchMedia('(prefers-reduced-motion: reduce)');
    constructor(readonly data: LottieAnimation, name: string) {
        this.element = svgElement('svg', { viewBox: '0 0 24 24', width: '24', height: '24', 'aria-hidden': 'true', focusable: 'false', class: `glyph glyph-${name} lottie-glyph`, 'data-renderer': 'lottie-svg', 'data-frames': String(data.op), 'data-frame': String(data.op - 1), fill: 'none' });
        for (const layer of [...data.layers].reverse()) {
            const group = svgElement('g', { 'data-layer': layer.nm });
            const paths: SVGGeometryElement[] = [];
            const stroke = layer.shapes.find((shape) => shape.ty === 'st');
            const fill = layer.shapes.some((shape) => shape.ty === 'fl');
            for (const shape of layer.shapes) {
                let node: SVGGeometryElement | null = null;
                if (shape.ty === 'sh')
                    node = svgElement('path', { d: shapePath(shape.ks.k) });
                if (shape.ty === 'el') {
                    const p = sampleProperty(shape.p, 0), size = sampleProperty(shape.s, 0);
                    node = svgElement('ellipse', { cx: String(p[0]), cy: String(p[1]), rx: String((size[0] ?? 0) / 2), ry: String((size[1] ?? 0) / 2) });
                }
                if (shape.ty === 'rc') {
                    const p = sampleProperty(shape.p, 0), size = sampleProperty(shape.s, 0);
                    node = svgElement('rect', { x: String((p[0] ?? 0) - (size[0] ?? 0) / 2), y: String((p[1] ?? 0) - (size[1] ?? 0) / 2), width: String(size[0]), height: String(size[1]), rx: String(sampleProperty(shape.r, 0)[0]) });
                }
                if (!node)
                    continue;
                node.setAttribute('fill', fill ? 'currentColor' : 'none');
                if (stroke?.ty === 'st') {
                    node.setAttribute('stroke', 'currentColor');
                    node.setAttribute('stroke-width', String(sampleProperty(stroke.w, 0)[0]));
                    node.setAttribute('stroke-linecap', 'round');
                    node.setAttribute('stroke-linejoin', 'round');
                }
                node.setAttribute('pathLength', '100');
                paths.push(node);
                group.append(node);
            }
            this.layers.push({ data: layer, element: group, paths });
            this.element.append(group);
        }
        this.render(data.op - 1);
        this.reduced.addEventListener('change', this.onReducedMotion);
    }
    private onReducedMotion = (): void => {
        if (this.reduced.matches)
            this.stop();
    };
    render(frame: number): void {
        this.element.dataset['frame'] = frame.toFixed(2);
        for (const view of this.layers) {
            const ks = view.data.ks;
            const p = sampleProperty(ks.p, frame), a = sampleProperty(ks.a, frame), s = sampleProperty(ks.s, frame), r = sampleProperty(ks.r, frame)[0] ?? 0;
            view.element.setAttribute('transform', `translate(${p[0] ?? 0} ${p[1] ?? 0}) rotate(${r}) scale(${(s[0] ?? 100) / 100} ${(s[1] ?? 100) / 100}) translate(${-(a[0] ?? 0)} ${-(a[1] ?? 0)})`);
            view.element.setAttribute('opacity', String((sampleProperty(ks.o, frame)[0] ?? 100) / 100));
            const trim = view.data.shapes.find((shape) => shape.ty === 'tm');
            if (trim?.ty === 'tm')
                for (const path of view.paths) {
                    const end = sampleProperty(trim.e, frame)[0] ?? 100;
                    path.setAttribute('stroke-dasharray', '100');
                    path.setAttribute('stroke-dashoffset', String(100 - end));
                }
        }
    }
    play = (): void => {
        if (this.disposed || this.reduced.matches)
            return;
        cancelAnimationFrame(this.request);
        this.started = performance.now();
        this.element.classList.add('icon-playing');
        this.render(0);
        const tick = (now: number): void => {
            if (this.disposed)
                return;
            const frame = (now - this.started) / 1000 * this.data.fr;
            if (frame >= this.data.op) {
                this.stop();
                return;
            }
            this.render(frame);
            this.request = requestAnimationFrame(tick);
        };
        this.request = requestAnimationFrame(tick);
    };
    stop(): void { cancelAnimationFrame(this.request); this.request = 0; this.element.classList.remove('icon-playing'); this.render(this.data.op - 1); }
    destroy(): void { this.stop(); this.disposed = true; this.reduced.removeEventListener('change', this.onReducedMotion); }
}
