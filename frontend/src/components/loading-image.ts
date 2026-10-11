import { h } from './dom.js';
import { icon } from './icons.js';
import { skeletonBlock } from './skeleton.js';
export interface LoadingImageOptions {
    src: string;
    alt: string;
    width?: number;
    height?: number;
    lazy?: boolean;
    fit?: 'contain' | 'cover';
    /** Only full viewers may include Retry. A thumbnail is already inside a button. */
    retry?: boolean;
    className?: string;
}
const pending = new Map<HTMLElement, () => void>();
let removalObserver: MutationObserver | null = null;
function trackPending(root: HTMLElement, cleanup: () => void): () => void {
    pending.set(root, cleanup);
    if (!removalObserver) {
        removalObserver = new MutationObserver(() => {
            for (const [element, dispose] of [...pending])
                if (!element.isConnected)
                    dispose();
        });
        removalObserver.observe(document, { subtree: true, childList: true });
    }
    return () => {
        pending.delete(root);
        if (!pending.size) {
            removalObserver?.disconnect();
            removalObserver = null;
        }
    };
}
/** Uses actual browser load/error events. No minimum display time or fake progress. */
export function loadingImage(options: LoadingImageOptions): HTMLElement {
    const width = options.width && Number.isFinite(options.width) && options.width > 0 ? options.width : 1;
    const height = options.height && Number.isFinite(options.height) && options.height > 0 ? options.height : 1;
    const root = h('span', { class: `loading-image ${options.className ?? ''}`, 'data-media-state': 'loading', style: `--media-ratio:${width} / ${height}` });
    const frame = h('span', { class: 'loading-image-frame', 'aria-busy': true });
    const placeholder = skeletonBlock('media');
    const feedback = h('span', { class: 'media-feedback', hidden: true });
    const status = options.retry ? h('span', { class: 'sr-only', role: 'status', 'aria-live': 'polite' }, 'Loading image') : null;
    root.append(frame, feedback);
    if (status)
        root.append(status);
    let previousCleanup: (() => void) | undefined;
    const load = (): void => {
        previousCleanup?.();
        const image = h('img', { src: options.src, alt: options.alt, loading: options.lazy ? 'lazy' : 'eager', decoding: 'async', class: `loading-image-content media-fit-${options.fit ?? 'contain'}` });
        let finished = false;
        let timer: ReturnType<typeof setTimeout> | undefined;
        let visibility: IntersectionObserver | null = null;
        let untrack: (() => void) | undefined;
        const cleanup = (): void => {
            finished = true;
            clearTimeout(timer);
            visibility?.disconnect();
            image.removeEventListener('load', success);
            image.removeEventListener('error', failure);
            untrack?.();
        };
        const settle = (state: 'ready' | 'error', slow = false): void => {
            if (finished)
                return;
            cleanup();
            root.dataset['mediaState'] = state;
            frame.setAttribute('aria-busy', 'false');
            placeholder.hidden = true;
            feedback.hidden = state === 'ready';
            if (status)
                status.textContent = state === 'ready' ? 'Image loaded' : slow ? 'Image is taking too long to load' : 'Image couldn’t load';
            if (state === 'error') {
                feedback.replaceChildren(icon('image', 24), h('strong', {}, slow ? 'Image is taking too long' : 'Image couldn’t load'), h('span', {}, options.retry ? 'Only this preview needs retrying.' : 'Open the preview to retry.'));
                if (options.retry) {
                    const retry = h('button', { type: 'button', class: 'button media-retry' }, icon('refresh', 16), 'Retry image');
                    retry.addEventListener('click', () => { load(); root.focus({ preventScroll: true }); });
                    feedback.append(retry);
                }
            }
        };
        const success = (): void => {
            if (image.naturalWidth > 0)
                settle('ready');
            else
                failure();
        };
        const failure = (): void => settle('error');
        const startDeadline = (): void => {
            if (timer || finished)
                return;
            timer = setTimeout(() => settle('error', true), 20000);
            visibility?.disconnect();
        };
        previousCleanup = cleanup;
        untrack = trackPending(root, cleanup);
        root.dataset['mediaState'] = 'loading';
        if (options.retry) {
            root.setAttribute('tabindex', '-1');
            root.setAttribute('role', 'group');
            root.setAttribute('aria-label', `Image preview: ${options.alt}`);
        }
        frame.setAttribute('aria-busy', 'true');
        placeholder.hidden = false;
        feedback.hidden = true;
        if (status)
            status.textContent = 'Loading image';
        frame.replaceChildren(placeholder, image);
        image.addEventListener('load', success);
        image.addEventListener('error', failure);
        if (options.lazy && typeof IntersectionObserver !== 'undefined') {
            visibility = new IntersectionObserver(entries => {
                if (entries.some(entry => entry.isIntersecting))
                    startDeadline();
            });
            visibility.observe(root);
        }
        else
            startDeadline();
        // Cached success and cached errors can both have complete=true.
        queueMicrotask(() => {
            if (!finished && image.complete)
                success();
        });
    };
    load();
    return root;
}
