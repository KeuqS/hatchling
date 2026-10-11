export type Child = Node | string | number | null | undefined | false;
export type Attributes = Record<string, string | number | boolean | null | undefined>;
/** Safe DOM construction. Strings are always text nodes, never HTML. */
export function h<K extends keyof HTMLElementTagNameMap>(tag: K, attributes: Attributes = {}, ...children: Child[]): HTMLElementTagNameMap[K] {
    const node = document.createElement(tag);
    for (const [key, value] of Object.entries(attributes)) {
        if (typeof value === 'boolean' && (key.startsWith('aria-') || key.startsWith('data-'))) {
            node.setAttribute(key, String(value));
            continue;
        }
        if (value === false || value === null || value === undefined)
            continue;
        node.setAttribute(key === 'className' ? 'class' : key, value === true ? '' : String(value));
    }
    for (const child of children)
        if (child !== null && child !== undefined && child !== false)
            node.append(child instanceof Node ? child : document.createTextNode(String(child)));
    return node;
}
export function replacePreservingFocus(root: HTMLElement, ...content: HTMLElement[]): void {
    const active = document.activeElement;
    const activeId = active instanceof HTMLElement && root.contains(active) ? active.id : '';
    const selection = active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement ? [active.selectionStart, active.selectionEnd] : null;
    const scrolls = [...root.querySelectorAll<HTMLElement>('[data-scroll]')].map((element) => ({ key: element.dataset['scroll'], top: element.scrollTop, left: element.scrollLeft }));
    root.replaceChildren(...content);
    for (const saved of scrolls) {
        const target = [...root.querySelectorAll<HTMLElement>('[data-scroll]')].find((element) => element.dataset['scroll'] === saved.key);
        if (target) {
            target.scrollTop = saved.top;
            target.scrollLeft = saved.left;
        }
    }
    if (activeId) {
        const target = document.getElementById(activeId);
        target?.focus({ preventScroll: true });
        if (selection && (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) && target.type !== 'number') {
            try {
                target.setSelectionRange(selection[0] ?? 0, selection[1] ?? 0);
            }
            catch { /* Some native input types do not expose a text selection. */ }
        }
    }
}
export function valueOf(form: HTMLFormElement, name: string): string {
    const value = new FormData(form).get(name);
    return typeof value === 'string' ? value : '';
}
