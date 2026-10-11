import { h } from './dom.js';
import { iconButton, button } from './ui.js';
export interface DialogRef {
    element: HTMLDialogElement;
    body: HTMLElement;
    close(): void;
}
let nextDialogId = 0;
export class DialogManager {
    constructor(private readonly root: HTMLElement, private readonly afterClose?: () => void) { }
    open(title: string, content: HTMLElement, className = ''): DialogRef {
        nextDialogId++;
        const label = `dialog-title-${nextDialogId}`;
        const dialog = h('dialog', { class: `dialog ${className}`, 'aria-labelledby': label });
        const body = h('div', { class: 'dialog-body' }, content);
        let closed = false;
        const close = (): void => {
            if (closed)
                return;
            closed = true;
            dialog.close();
            dialog.remove();
            this.afterClose?.();
        };
        const header = h('header', { class: 'dialog-header' }, h('h2', { id: label }, title), iconButton('close', `Close ${title}`, close));
        dialog.append(header, body);
        this.root.append(dialog);
        dialog.addEventListener('cancel', (event) => { event.preventDefault(); close(); });
        dialog.addEventListener('click', (event) => {
            if (event.target === dialog) {
                const bounds = dialog.getBoundingClientRect();
                if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom)
                    close();
            }
        });
        dialog.addEventListener('keydown', (event) => {
            if (event.key !== 'Tab')
                return;
            const focusable = [...dialog.querySelectorAll<HTMLElement>('button:not([disabled]),input:not([disabled]),textarea:not([disabled]),select:not([disabled]),a[href],summary,[tabindex="0"]')].filter((element) => element.getClientRects().length > 0 && element.tabIndex >= 0);
            const first = focusable[0];
            const last = focusable.at(-1);
            if (!first || !last)
                return;
            if (event.shiftKey && document.activeElement === first) {
                event.preventDefault();
                last.focus();
            }
            else if (!event.shiftKey && document.activeElement === last) {
                event.preventDefault();
                first.focus();
            }
        });
        dialog.showModal();
        return { element: dialog, body, close };
    }
    confirm(title: string, description: string, action: () => void, options: {
        label?: string;
        danger?: boolean;
    } = {}): void {
        const content = h('div', { class: 'confirm-content' }, h('p', {}, description));
        const ref = this.open(title, content, 'confirm-dialog');
        content.append(h('div', { class: 'dialog-actions' }, button('Cancel', ref.close), button(options.label ?? 'Confirm', () => { ref.close(); action(); }, { variant: options.danger ? 'danger' : 'primary' })));
    }
    closeAll(): void {
        for (const dialog of this.root.querySelectorAll<HTMLDialogElement>('dialog.dialog')) {
            dialog.close();
            dialog.remove();
        }
        this.afterClose?.();
    }
}
