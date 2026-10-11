import { setFloatingMenuOpen } from './floating.js';
import type { IconName } from '../protocol/models.js';
import { h } from './dom.js';
import { icon } from './icons.js';
export function button(label: string, handler: () => void, options: {
    icon?: IconName;
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'link';
    className?: string;
    disabled?: boolean;
    title?: string;
    id?: string;
} = {}): HTMLButtonElement {
    const node = h('button', { type: 'button', class: `button ${options.variant ?? 'secondary'} ${options.className ?? ''}`, disabled: options.disabled, title: options.title, id: options.id }, options.icon ? icon(options.icon, 16) : null, label);
    node.addEventListener('click', handler);
    return node;
}
export function iconButton(name: IconName, label: string, handler: () => void, className = ''): HTMLButtonElement {
    const node = h('button', { type: 'button', class: `icon-button ${className}`, 'aria-label': label, 'data-tooltip': label }, icon(name));
    node.addEventListener('click', handler);
    return node;
}
export function pill(text: string, kind = ''): HTMLElement { return h('span', { class: `pill ${kind}` }, text); }
export function sectionTitle(title: string, suffix?: string): HTMLElement { return h('div', { class: 'section-heading' }, h('h2', {}, title), suffix ? h('span', { class: 'muted text-small' }, suffix) : null); }
export function emptyState(name: IconName, title: string, body?: string): HTMLElement { return h('div', { class: 'empty-state' }, icon(name, 34), h('p', {}, title), body ? h('span', { class: 'muted text-small' }, body) : null); }
export function field(label: string, name: string, value = '', options: {
    type?: string;
    placeholder?: string;
    required?: boolean;
    min?: number;
    max?: number;
    autofocus?: boolean;
} = {}): HTMLElement {
    return h('label', { class: 'form-field' }, h('span', {}, label), h('input', { name, id: `field-${name}`, type: options.type ?? 'text', value, placeholder: options.placeholder, required: options.required ?? false, min: options.min, max: options.max, autofocus: options.autofocus }));
}
export function textareaField(label: string, name: string, value: string, rows = 4): HTMLElement {
    const input = h('textarea', { name, id: `field-${name}`, rows });
    input.value = value;
    return h('label', { class: 'form-field' }, h('span', {}, label), input);
}
export function selectField(label: string, name: string, options: readonly {
    value: string;
    label: string;
}[], current: string): HTMLElement {
    const select = h('select', { name, id: `field-${name}`, 'aria-label': label }, ...options.map((item) => h('option', { value: item.value, selected: item.value === current }, item.label)));
    return h('label', { class: 'form-field' }, h('span', {}, label), select);
}
export function toggle(label: string, checked: boolean, handler: (value: boolean) => void, description?: string): HTMLElement {
    const input = h('input', { type: 'checkbox', role: 'switch', checked, 'aria-label': label });
    input.addEventListener('change', () => handler(input.checked));
    return h('label', { class: 'toggle-row' }, h('span', {}, h('span', { class: 'row-title' }, label), description ? h('span', { class: 'muted text-small row-description' }, description) : null), h('span', { class: 'toggle' }, input, h('span', { class: 'toggle-track', 'aria-hidden': 'true' })));
}
export function infoRow(title: string, subtitle: string, leading: IconName, action?: HTMLElement): HTMLElement {
    return h('div', { class: 'info-row' }, h('span', { class: 'row-icon' }, icon(leading, 20)), h('div', { class: 'row-copy' }, h('span', { class: 'row-title' }, title), subtitle ? h('span', { class: 'row-description muted text-small' }, subtitle) : null), action);
}
let nextMenuId = 0;
export function menu(label: string, actions: readonly {
    label: string;
    icon: IconName;
    action: () => void;
    danger?: boolean;
    disabled?: boolean;
    separatorBefore?: boolean;
}[]): HTMLElement {
    const id = `action-menu-${++nextMenuId}`;
    const details = h('details', { class: 'menu' });
    const summary = h('summary', { class: 'icon-button', 'aria-label': label, 'aria-expanded': 'false', 'aria-controls': id, 'data-tooltip': label, title: label }, icon('more'));
    const content = h('div', { class: 'menu-popover', id, role: 'group', 'aria-label': `${label} actions` });
    for (const [index, item] of actions.entries()) {
        if (index > 0 && item.separatorBefore)
            content.append(h('div', { class: 'menu-separator', role: 'separator' }));
        const control = button('', () => {
            setFloatingMenuOpen(details, false);
            summary.focus({ preventScroll: true });
            item.action();
            // An action may replace its owning row. Restore only lost focus, never
            // take it back from a dialog, input, viewer or explicit navigation target.
            requestAnimationFrame(() => {
                if (document.activeElement === document.body && !summary.isConnected)
                    [...document.querySelectorAll<HTMLElement>('details.menu > summary')].find(node => node.getAttribute('aria-label') === label)?.focus({ preventScroll: true });
            });
        }, { icon: item.icon, variant: item.danger ? 'danger' : 'ghost', disabled: item.disabled ?? false });
        control.append(h('span', { class: 'menu-item-label' }, item.label));
        content.append(control);
    }
    details.append(summary, content);
    summary.addEventListener('click', (event) => {
        event.preventDefault();
        setFloatingMenuOpen(details, !details.open);
    });
    let search = '';
    let lastKeyAt = 0;
    details.addEventListener('keydown', (event) => {
        const horizontal = details.classList.contains('reaction-menu');
        const nextKey = event.key === 'ArrowDown' || (horizontal && event.key === 'ArrowRight');
        const previousKey = event.key === 'ArrowUp' || (horizontal && event.key === 'ArrowLeft');
        if (nextKey || previousKey || event.key === 'Home' || event.key === 'End') {
            event.preventDefault();
            event.stopPropagation();
            setFloatingMenuOpen(details, true);
            const controls = [...content.querySelectorAll<HTMLButtonElement>('button:not([disabled])')];
            const index = controls.findIndex(control => control === document.activeElement);
            const next = event.key === 'Home' ? 0 : event.key === 'End' ? controls.length - 1 : index < 0 ? (previousKey ? controls.length - 1 : 0) : (index + (nextKey ? 1 : -1) + controls.length) % controls.length;
            controls[next]?.focus();
        }
        else if (event.key === 'Escape' && details.open) {
            event.preventDefault();
            event.stopPropagation();
            setFloatingMenuOpen(details, false);
            summary.focus({ preventScroll: true });
        }
        else if (event.key === 'Tab' && details.open) {
            setFloatingMenuOpen(details, false);
            summary.focus({ preventScroll: true });
        }
        else if (details.open && event.key.length === 1 && event.key !== ' ' && !event.ctrlKey && !event.metaKey && !event.altKey) {
            const now = performance.now();
            search = now - lastKeyAt > 700 ? event.key.toLowerCase() : search + event.key.toLowerCase();
            lastKeyAt = now;
            const controls = [...content.querySelectorAll<HTMLButtonElement>('button:not([disabled])')];
            const index = controls.findIndex(control => control === document.activeElement);
            const ordered = [...controls.slice(index + 1), ...controls.slice(0, index + 1)];
            const match = (value: string): HTMLButtonElement | undefined => ordered.find(control => (control.getAttribute('aria-label') ?? control.textContent ?? '').trim().toLowerCase().startsWith(value));
            const target = match(search) ?? match(event.key.toLowerCase());
            if (target) {
                event.preventDefault();
                target.focus();
            }
        }
    });
    details.addEventListener('focusout', (event) => {
        if (event.relatedTarget instanceof Node && details.contains(event.relatedTarget))
            return;
        queueMicrotask(() => {
            if (details.open && !details.contains(document.activeElement))
                setFloatingMenuOpen(details, false);
        });
    });
    return details;
}
export { inlineText, markdown } from './markdown.js';
export function tabs<T extends string>(items: readonly {
    id: T;
    label: string;
}[], active: T, onSelect: (id: T) => void): HTMLElement {
    return h('div', { class: 'tabs', role: 'group', 'aria-label': 'View filter' }, ...items.map((item) => {
        const node = button(item.label, () => onSelect(item.id), { variant: 'ghost', className: active === item.id ? 'active' : '' });
        node.setAttribute('aria-pressed', String(active === item.id));
        return node;
    }));
}
