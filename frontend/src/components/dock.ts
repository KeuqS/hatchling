import { h } from './dom.js';
import { iconButton } from './ui.js';
export interface DockOptions {
    expanded?: boolean;
    preserveRail?: boolean;
}
export interface DockRef {
    element: HTMLDialogElement;
    body: HTMLElement;
    close(): void;
}
/** A nonmodal, persistent sibling of the shell: editing never remounts during chat updates. */
export class ArtifactDock {
    private ref: DockRef | null = null;
    private readonly syncInert = (): void => {
        const app = document.getElementById('app');
        if (app) {
            const rail = this.ref?.element.classList.contains('dock-keep-rail') === true && innerWidth > 800;
            app.inert = this.coversConversation && !rail;
            for (const region of app.querySelectorAll<HTMLElement>('.main-region, .navigation-panel, .context-panel'))
                region.inert = this.coversConversation && rail;
            this.ref?.element.setAttribute('aria-modal', String(app.inert));
        }
    };
    private readonly keydown = (event: KeyboardEvent): void => {
        if (event.key === 'Tab' && this.ref && document.getElementById('app')?.inert && !document.querySelector('dialog:modal')) {
            const controls = [...this.ref.element.querySelectorAll<HTMLElement>('button:not([disabled]),input:not([disabled]),textarea:not([disabled]),select:not([disabled]),a[href],summary,[tabindex="0"]')].filter((node) => node.getClientRects().length > 0);
            const first = controls[0];
            const last = controls.at(-1);
            if (first && last && event.shiftKey && document.activeElement === first) {
                event.preventDefault();
                last.focus();
            }
            else if (first && last && !event.shiftKey && document.activeElement === last) {
                event.preventDefault();
                first.focus();
            }
        }
        if (event.key !== 'Escape' || event.defaultPrevented || document.querySelector('dialog:modal') || document.querySelector('details.menu[open]'))
            return;
        if (this.ref) {
            event.preventDefault();
            this.ref.close();
        }
    };
    constructor(private readonly root: HTMLElement, private readonly changed: () => void) { document.addEventListener('keydown', this.keydown); window.addEventListener('resize', this.syncInert); }
    get isOpen(): boolean { return this.ref !== null; }
    get coversConversation(): boolean { return this.isOpen && (innerWidth <= 1000 || this.ref?.element.classList.contains('dock-expanded') === true); }
    get keepsRail(): boolean { return this.ref?.element.classList.contains('dock-keep-rail') === true; }
    syncAccessibility(): void { this.syncInert(); }
    close(): void { this.ref?.close(); }
    open(title: string, content: HTMLElement, className = '', onClose?: () => void, options: DockOptions = {}): DockRef {
        const existing = this.ref;
        const expanded = options.expanded ?? existing?.element.classList.contains('dock-expanded') ?? false;
        const opening = !existing;
        this.remove(false);
        const label = 'artifact-dock-title';
        const dialog = h('dialog', { class: `artifact-dock ${className} ${expanded ? 'dock-expanded' : ''} ${options.preserveRail ? 'dock-keep-rail' : ''}`, 'aria-labelledby': label, 'aria-modal': 'false' });
        const body = h('div', { class: 'dock-body' }, content);
        const trigger = document.activeElement;
        let closed = false;
        const close = (): void => {
            if (closed || this.ref?.element !== dialog)
                return;
            closed = true;
            this.ref = null;
            const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
            const finish = (): void => { dialog.close(); dialog.remove(); };
            if (!reduced) {
                const motion = dialog.animate([{ transform: 'translateX(0)', opacity: 1 }, { transform: 'translateX(32px)', opacity: 0 }], { duration: 180, easing: 'ease-in', fill: 'forwards' });
                motion.addEventListener('finish', finish, { once: true });
            }
            else
                finish();
            document.body.classList.remove('has-artifact-dock', 'artifact-expanded', 'has-browser-session', 'artifact-rail-visible');
            this.syncInert();
            this.changed();
            if (trigger instanceof HTMLElement && trigger.isConnected)
                trigger.focus({ preventScroll: true });
            else
                document.querySelector<HTMLElement>('.composer textarea, #main')?.focus({ preventScroll: true });
            onClose?.();
        };
        const expand = iconButton('expand', expanded ? 'Restore split view' : 'Expand artifact', () => {
            const full = dialog.classList.toggle('dock-expanded');
            document.body.classList.toggle('artifact-expanded', full);
            expand.setAttribute('aria-label', full ? 'Restore split view' : 'Expand artifact');
            expand.dataset['tooltip'] = full ? 'Restore split view' : 'Expand artifact';
            expand.setAttribute('aria-pressed', String(full));
            this.syncInert();
            this.changed();
        });
        expand.setAttribute('aria-pressed', String(expanded));
        dialog.append(h('header', { class: 'dock-header' }, h('div', { class: 'dock-name' }, iconButton('panel', 'Focus conversation', () => {
            if (innerWidth <= 1000) {
                close();
                return;
            }
            if (dialog.classList.contains('dock-expanded'))
                expand.click();
            document.querySelector<HTMLElement>('.composer textarea, #main')?.focus({ preventScroll: true });
        }), h('h2', { id: label }, title)), h('div', { class: 'dock-header-actions' }, expand, iconButton('close', `Close ${title}`, close))), body);
        this.root.append(dialog);
        this.ref = { element: dialog, body, close };
        document.body.classList.add('has-artifact-dock');
        document.body.classList.toggle('has-browser-session', className.includes('browser-session-dock'));
        document.body.classList.toggle('artifact-expanded', expanded);
        document.body.classList.toggle('artifact-rail-visible', options.preserveRail === true);
        this.changed();
        dialog.show();
        this.syncInert();
        dialog.querySelector<HTMLElement>('.dock-header .icon-button')?.focus({ preventScroll: true });
        if (opening && !matchMedia('(prefers-reduced-motion: reduce)').matches)
            dialog.animate([{ transform: 'translateX(48px)', opacity: 0 }, { transform: 'translateX(0)', opacity: 1 }], { duration: 280, easing: 'cubic-bezier(.2,.8,.2,1)' });
        return this.ref;
    }
    /** Replacement does not call the previous origin callback or flash the conversation. */
    private remove(clearClass = true): void {
        if (this.ref) {
            this.ref.element.close();
            this.ref.element.remove();
            this.ref = null;
        }
        if (clearClass)
            document.body.classList.remove('has-artifact-dock', 'artifact-expanded', 'has-browser-session', 'artifact-rail-visible');
    }
    dispose(): void { this.remove(); this.syncInert(); document.removeEventListener('keydown', this.keydown); window.removeEventListener('resize', this.syncInert); }
}
