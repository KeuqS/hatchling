/** Fixed, hand-authored product pictograms. No emoji, font glyphs, copied paths or Lottie data. */
import type { IconName } from '../protocol/models.js';
import { icon } from './icons.js';
let serial = 0;
function svgNode<K extends keyof SVGElementTagNameMap>(tag: K, attributes: Record<string, string | number> = {}, ...children: SVGElement[]): SVGElementTagNameMap[K] {
    const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [key, value] of Object.entries(attributes))
        node.setAttribute(key, String(value));
    node.append(...children);
    return node;
}
export function pictogram(name: IconName): SVGSVGElement {
    const id = `pictogram-${serial++}`;
    const svg = svgNode('svg', { viewBox: '0 0 48 48', width: 44, height: 44, fill: 'none', 'aria-hidden': 'true', focusable: 'false' });
    const metal = `${id}-metal`;
    const paper = `${id}-paper`;
    svg.append(svgNode('defs', {}, svgNode('linearGradient', { id: metal, x1: '0%', y1: '0%', x2: '100%', y2: '100%' }, svgNode('stop', { offset: 0, 'stop-color': '#e0e3e5' }), svgNode('stop', { offset: .45, 'stop-color': '#8997a4' }), svgNode('stop', { offset: 1, 'stop-color': '#4c5964' })), svgNode('linearGradient', { id: paper, x1: '0%', y1: '0%', x2: '90%', y2: '100%' }, svgNode('stop', { offset: 0, 'stop-color': '#f5e8d4' }), svgNode('stop', { offset: 1, 'stop-color': '#bca485' }))));
    const path = (d: string, fill: string, stroke = 'none', width = 1): SVGPathElement => svgNode('path', { d, fill, stroke, 'stroke-width': width, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' });
    if (name === 'clipboard') {
        svg.append(svgNode('g', { transform: 'rotate(-7 24 24)' }, svgNode('rect', { x: 12, y: 8, width: 26, height: 36, rx: 3, fill: `url(#${paper})`, stroke: '#bba68d' }), path('M18 10v-4h5c0-4 6-4 6 0h5v5Z', `url(#${metal})`, '#627785'), path('M22 7h7', 'none', '#dce2e7', 1.3), path('M17 17h4v4h-4Zm0 8h4v4h-4Zm0 8h4v4h-4Z', 'none', '#9d8b76'), path('M25 18h8m-8 3h6m-6 5h8m-8 3h6m-6 5h8m-8 3h6', 'none', '#8e8374', 1.1)));
    }
    else if (name === 'lock') {
        svg.append(path('M15 23V12c0-13 20-13 20 0v11h-5V12c0-6-10-6-10 0v11Z', `url(#${metal})`, '#657078'), path('M10 22 35 18l3 5v19l-24 4-5-5Z', '#ab823f', '#83622e'), path('M10 22 32 19v24l-18 3-4-4Z', '#d4b16e'), path('M23 30v5', 'none', '#6b522a', 2));
    }
    else if (name === 'folder') {
        svg.append(path('M5 14 19 9l6 4 16-4 3 4-1 26-33 8-5-5Z', '#b58a54'), path('M6 15 20 11l6 4 14-3v24L9 44Z', '#ebcda1'), path('M10 23 45 14l-3 27-31 7Z', `url(#${paper})`, '#d0ad7a'));
    }
    else if (name === 'calendar') {
        svg.append(path('M4 39 35 32l10 6-32 8Z', '#8a7661'), path('M10 8 36 3l5 33-26 6Z', `url(#${paper})`, '#a69481'), path('M10 8 36 3l2 9-26 5Z', '#6385a5'), path('M15 18 38 14l2 18-23 6Z', '#f4eddf'), path('M20 22c6-4 8 2 3 3 7-1 7 7 1 7m7-12v10', 'none', '#6d849b', 1.9));
    }
    else if (name === 'compass') {
        svg.append(svgNode('ellipse', { cx: 24, cy: 25, rx: 19, ry: 21, fill: `url(#${metal})`, stroke: '#c9cacc', 'stroke-width': 1.4 }), svgNode('ellipse', { cx: 24, cy: 25, rx: 14, ry: 17, fill: '#525e69', stroke: '#9da8b3', 'stroke-width': 2 }), svgNode('ellipse', { cx: 24, cy: 25, rx: 9, ry: 11, stroke: '#e6bf75', 'stroke-width': 1.5 }), path('M24 14v22m-9-11h18m-15-8 12 16m0-16L18 33', 'none', '#caab6d', 1.2), svgNode('circle', { cx: 24, cy: 25, r: 2.5, fill: '#f0d59e' }));
    }
    else if (name === 'microscope') {
        svg.append(path('M10 41 30 36l11 6-21 5Z', `url(#${metal})`), path('M27 13c18 6 12 25-2 26v-5c10-2 10-11 0-16Z', '#8b9a9f', '#627278'), path('M20 31h14v4H20Z', '#303942'), path('M25 5 34 9l-9 23-10-5Z', `url(#${metal})`), path('M26 1 33 4l-2 7-7-3Z', '#d7dce0'), path('M15 25 25 29l-2 4-10-4Z', '#b5a06e'), path('M12 34 20 37l-2 3-9-3Z', '#7c8d95'));
    }
    else if (name === 'shield') {
        svg.append(path('M23 3 40 9l-2 17c-2 9-8 14-15 18C16 40 9 33 8 26L6 9Z', `url(#${paper})`, '#879198', 1.5), path('M23 8 35 13l-2 15c-2 5-6 9-10 12-5-4-9-9-10-14l-1-13Z', '#8a9daa'), path('M18 22h11v11H18Z', '#405c71'), path('M20 22v-4c0-5 7-5 7 0v4', 'none', '#d9d8d1', 2));
    }
    else {
        return icon(name, 36);
    }
    return svg;
}
