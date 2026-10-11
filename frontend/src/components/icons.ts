import type { IconName } from '../protocol/models.js';
const paths: Record<IconName, readonly string[]> = {
    smile: ['M22.5 12a10.5 10.5 0 1 1-21 0 10.5 10.5 0 0 1 21 0', 'M8 14s1.3 3 4 3 4-3 4-3M8 9h.01M16 9h.01'],
    reply: ['m9 4-7 7 7 7v-5c6 0 9 2 12 7-1-10-6-12-12-12Z'],
    hand: ['M8 12V5a1.5 1.5 0 0 1 3 0v6-8a1.5 1.5 0 0 1 3 0v8-6a1.5 1.5 0 0 1 3 0v7-3a1.5 1.5 0 0 1 3 0v7c0 4-3 6-7 6-4 0-6-2-8-6l-2-4c-1-2 2-3 3-1l2 3Z'],
    mail: ['M4 5h16a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1Z', 'm3 6 9 7 9-7M3 18l6-6M21 18l-6-6'],
    transfer: ['M7 3v17m-4-4 4 4 4-4M17 21V4m-4 4 4-4 4 4'],
    eye: ['M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12Z', 'M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0'],
    bug: ['M7 9h10v7a5 5 0 0 1-10 0ZM9 9V6a3 3 0 0 1 6 0v3M12 9v12M3 10h4M17 10h4M2 15h5M17 15h5M4 21l4-3M16 18l4 3M8 2l2 2M16 2l-2 2'],
    currency: ['M12 2v20M17 6c-2-3-10-2-10 2s10 3 10 7-8 6-11 2'],
    palette: ['M12 3c-5 0-9 4-9 9s4 9 9 9h2c3 0 2-4 0-4-2-1-1-3 1-3h3c5 0 4-11-6-11Z', 'M7 9h.1M11 6h.1M16 7h.1M6 14h.1'],
    laptop: ['M5 3h14v13H5ZM3 16h18l1 4H2Z'],
    clipboard: ['M9 5H5v17h14V5h-4M9 3h6v4H9ZM8 12h8M8 16h6'],
    microscope: ['m11 2 6 3-5 11-6-3ZM15 9c9 1 8 10-1 11M5 21h15M5 17h8'],
    compass: ['M22.5 12a10.5 10.5 0 1 1-21 0 10.5 10.5 0 0 1 21 0', 'm15 7-2 7-5 3 2-7Z'],
    fingerprint: ['M5 9a7 7 0 0 1 14 0', 'M3 7a10 10 0 0 1 18 0', 'M8 21c-2-3-2-6-2-9a6 6 0 0 1 12 0c0 3 0 6 3 8', 'M11 22c-3-4-3-7-3-10a4 4 0 0 1 8 0c0 4 1 7 3 9', 'M14 21c-3-4-3-6-3-9a1 1 0 0 1 2 0c0 4 1 6 3 9', 'M3 11c0 4 0 6 2 9'],
    menu: ['M5 9h14', 'M5 15h14'],
    calendar: ['M5 3h14a2 2 0 0 1 2 2v15H3V5a2 2 0 0 1 2-2Z', 'M3 8h18M7 2v3M17 2v3M7 12h.1M12 12h.1M17 12h.1M7 16h.1M12 16h.1M17 16h.1'],
    connected: ['M22.5 12a10.5 10.5 0 1 1-21 0 10.5 10.5 0 0 1 21 0', 'm13 5-5 8h4l-1 6 5-9h-4Z'],
    chat: ['M21 11.5a8.5 8.5 0 0 1-8.5 8.5 10 10 0 0 1-4-.8L3 21l1.8-5.5a8.5 8.5 0 1 1 16.2-4Z'],
    search: ['M21 21l-5-5', 'M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0'],
    feed: ['M7 3h11a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z', 'M5 7H3v12M9 7h7M9 11h5'],
    idea: ['M9 18h6M10 21h4M8 14a6 6 0 1 1 8 0c-1.2 1-1.5 2-1.5 4h-5c0-2-.3-3-1.5-4'],
    goal: ['M6 3h12a3 3 0 0 1 3 3v12a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3V6a3 3 0 0 1 3-3Z', 'm7 12 3 3 7-7'],
    library: ['m6 2 4 5-4 5-4-5Z', 'm17 2 4 8h-8Z', 'M2 17a4 4 0 1 0 8 0 4 4 0 0 0-8 0', 'M14 14h7v7h-7Z'],
    browser: ['M4 3h16a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1ZM3 8h18', 'M6 5.5h.1M9 5.5h.1M12 5.5h.1'],
    memory: ['M8 3c-3 0-5 3-4 6-3 3-2 7 1 8-1 3 2 5 5 4V4c0-2-2-2-2-1Zm8 0c3 0 5 3 4 6 3 3 2 7-1 8 1 3-2 5-5 4V4c0-2 2-2 2-1Z', 'M4 9h3M5 17l2-2M20 9h-3M19 17l-2-2'],
    clock: ['M22.5 12a10.5 10.5 0 1 1-21 0 10.5 10.5 0 0 1 21 0', 'M12 7v5l3 2'],
    bell: ['M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4'],
    settings: ['M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.09a2 2 0 0 1 1 1.74v.5a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z', 'M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0'],
    panel: ['M5 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2ZM9 4v16'],
    plus: ['M12 5v14M5 12h14'], close: ['m6 6 12 12M18 6 6 18'], chevron: ['m9 5 7 7-7 7'], arrow: ['M12 20V4m-6 6 6-6 6 6'],
    more: ['M5 12h.1M12 12h.1M19 12h.1'], check: ['m5 12 4 4L19 6'],
    shield: ['M12 2 3 6v6c0 6 9 10 9 10s9-4 9-10V6Z', 'm8 12 3 3 5-6'],
    shieldLock: ['M12 2 3 6v6c0 6 9 10 9 10s9-4 9-10V6Z', 'M9.5 12.4h5v4.1h-5Z', 'M10.55 12.4v-1.05a1.45 1.45 0 0 1 2.9 0v1.05', 'M12 14.2v1.2'],
    key: ['M12 15.5a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z', 'm10.8 12.7 9.2-9.2', 'm16.5 7 2 2', 'm14.3 9.2 2 2'],
    lock: ['M5 10h14v11H5ZM8 10V6a4 4 0 0 1 8 0v4M12 14v3'],
    link: ['m10 13 4-4M8 16l-2 2a4 4 0 0 1-6-6l6-6a4 4 0 0 1 6 0M16 8l2-2a4 4 0 0 1 6 6l-6 6a4 4 0 0 1-6 0'],
    wallet: ['M3 5h15a2 2 0 0 1 2 2v13H5a2 2 0 0 1-2-2V5Zm0 0 14-3v3M15 10h7v6h-7ZM18 13h.1'],
    device: ['M9 3h10a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2ZM11 18h6M3 7H1v14h3'],
    file: ['M5 2h10l5 5v15H5ZM15 2v6h5M8 12h8M8 16h5'],
    folder: ['M3 5h7l2 3h9v12H3ZM3 5V3h7l2 2h9v3'],
    image: ['M3 3h18v18H3Z', 'm3 16 5-5 5 5 4-4 4 4', 'M15 7h.1'],
    video: ['M3 5h12v14H3ZM15 9l6-4v14l-6-4Z'],
    audio: ['M3 10v4M7 5v14M12 2v20M17 7v10M21 10v4'],
    download: ['M12 3v12m-5-5 5 5 5-5M4 14v7h16v-7'],
    edit: ['m15 3 6 6-12 12H3v-6ZM12 6l6 6'],
    trash: ['M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M9 10v7M15 10v7'],
    pin: ['m16 3 5 5-5 2-2 6-2 2-6-6 2-2 6-2ZM9 15l-6 6'],
    share: ['M12 15V2m-5 5 5-5 5 5M5 11H3v11h18V11h-2'],
    grid: ['M3 3h7v7H3ZM14 3h7v7h-7ZM3 14h7v7H3ZM14 14h7v7h-7Z'],
    list: ['M7 5h14M7 12h14M7 19h14M3 5h.1M3 12h.1M3 19h.1'],
    mic: ['M9 5a3 3 0 0 1 6 0v7a3 3 0 0 1-6 0ZM5 10v2a7 7 0 0 0 14 0v-2M12 19v3M8 22h8'],
    phone: ['M3 3h5l2 6-3 2a14 14 0 0 0 6 6l2-3 6 2v5C11 24 0 13 3 3Z'],
    stop: ['M6 6h12v12H6Z'], heart: ['M20 4c-3-3-7-1-8 2-1-3-5-5-8-2-5 5 2 12 8 17C18 16 25 9 20 4Z'],
    refresh: ['M20 7a9 9 0 1 0 1 8M20 2v6h-6'],
    logout: ['M9 3H3v18h6M8 12h14m-5-5 5 5-5 5'],
    gift: ['M3 8h18v4H3ZM5 12v9h14v-9M12 8v13M12 8C3 8 5 0 9 3l3 5Zm0 0c9 0 7-8 3-5Z'],
    help: ['M22.5 12a10.5 10.5 0 1 1-21 0 10.5 10.5 0 0 1 21 0', 'M9 8a3 3 0 0 1 6 0c0 2-3 2-3 5M12 17h.1'],
    sun: ['M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0M12 1v2M12 21v2M1 12h2M21 12h2M4 4l2 2M18 18l2 2M4 20l2-2M18 6l2-2'],
    moon: ['M20 15A9 9 0 0 1 9 4a9 9 0 1 0 11 11Z'],
    globe: ['M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0M3 12h18M12 3c5 5 5 13 0 18-5-5-5-13 0-18Z'],
    run: ['M12 3v4M12 17v4M3 12h4M17 12h4M5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M5.6 18.4l2.8-2.8M15.6 8.4l2.8-2.8'],
    warning: ['m12 2 11 19H1ZM12 8v5M12 17h.1'],
    code: ['m8 6-6 6 6 6M16 6l6 6-6 6M14 3l-4 18'],
    spark: ['m12 2 3 7 7 3-7 3-3 7-3-7-7-3 7-3Z'],
    briefcase: ['M3 7h18v14H3ZM8 7V3h8v4M3 12h18M10 12v3h4v-3'],
    leaf: ['M21 3C9 1 1 8 5 17c8 6 17-2 16-14ZM3 21 17 7'],
    people: ['M8 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8M1 21v-3a7 7 0 0 1 14 0v3M17 4a4 4 0 0 1 0 8M18 15a6 6 0 0 1 5 6'],
    book: ['M12 5C8 2 3 2 1 3v17c4-2 8-1 11 1 3-2 7-3 11-1V3c-2-1-7-1-11 2ZM12 5v16'],
    cart: ['M1 3h3l3 12h13l3-9H5M9 21h.1M18 21h.1'],
    copy: ['M8 8h13v13H8ZM16 4V1H1v15h3'],
    expand: ['M8 3H3v5M16 3h5v5M3 16v5h5M21 16v5h-5'],
    filter: ['M3 5h18M6 12h12M9 19h6'],
    play: ['m8 3 13 9-13 9Z'], pause: ['M6 3h4v18H6ZM14 3h4v18h-4Z'],
    volume: ['M11 3 5 8H1v8h4l6 5ZM16 8a6 6 0 0 1 0 8M19 4a11 11 0 0 1 0 16'],
    paperclip: ['m8 12 7-7a3 3 0 0 1 4 4L9 19a5 5 0 0 1-7-7L13 1M6 14l9-9'],
    keyboard: ['M2 5h20v14H2ZM5 9h.1M9 9h.1M13 9h.1M17 9h.1M5 13h.1M9 13h.1M13 13h.1M17 13h.1M7 16h10'],
};
export function icon(name: IconName, size = 20): SVGSVGElement {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    for (const [key, value] of Object.entries({ viewBox: '0 0 24 24', width: String(size), height: String(size), fill: 'none', stroke: 'currentColor', 'stroke-width': name === 'more' ? '3' : '1.65', 'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': 'true', focusable: 'false' }))
        svg.setAttribute(key, value);
    svg.setAttribute('class', `glyph glyph-${name}`);
    for (const [index, d] of paths[name].entries()) {
        const path = document.createElementNS(svg.namespaceURI, 'path');
        path.setAttribute('d', d);
        path.setAttribute('class', `glyph-part part-${index}`);
        path.setAttribute('pathLength', '1');
        svg.append(path);
    }
    if (name === 'chat') {
        for (const [index, cx] of [8, 12, 16].entries()) {
            const dot = document.createElementNS(svg.namespaceURI, 'circle');
            dot.setAttribute('cx', String(cx));
            dot.setAttribute('cy', '11');
            dot.setAttribute('r', '1');
            dot.setAttribute('fill', 'currentColor');
            dot.setAttribute('stroke', 'none');
            dot.setAttribute('class', `typing-dot typing-dot-${index}`);
            svg.append(dot);
        }
    }
    return svg;
}
