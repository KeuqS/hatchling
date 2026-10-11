import type { AppContext } from '../../app/context.js';
import { h } from '../../components/dom.js';
import { button, markdown } from '../../components/ui.js';
export type MarkdownFormat = 'bold' | 'italic' | 'h1' | 'h2' | 'h3' | 'bullet' | 'number';
export interface TextEdit {
    value: string;
    start: number;
    end: number;
}
/** Selection-preserving Markdown formatting. No HTML editing or document.execCommand. */
export function formatMarkdown(value: string, start: number, end: number, format: MarkdownFormat): TextEdit {
    const from = Math.max(0, Math.min(start, value.length));
    const to = Math.max(from, Math.min(end, value.length));
    if (format === 'bold' || format === 'italic') {
        const token = format === 'bold' ? '**' : '*';
        const selected = value.slice(from, to);
        if (from >= token.length && value.slice(from - token.length, from) === token && value.slice(to, to + token.length) === token)
            return { value: value.slice(0, from - token.length) + selected + value.slice(to + token.length), start: from - token.length, end: to - token.length };
        return { value: value.slice(0, from) + token + selected + token + value.slice(to), start: from + token.length, end: to + token.length };
    }
    const lineStart = from === 0 ? 0 : value.lastIndexOf('\n', from - 1) + 1;
    const nextBreak = value.indexOf('\n', to);
    const lineEnd = nextBreak === -1 ? value.length : nextBreak;
    const prefix = format === 'h1' ? '# ' : format === 'h2' ? '## ' : format === 'h3' ? '### ' : format === 'bullet' ? '- ' : '1. ';
    const lines = value.slice(lineStart, lineEnd).split('\n');
    const remove = lines.every((line) => format === 'number' ? /^\d+\. /u.test(line) : line.startsWith(prefix));
    const result = lines.map((line, index) => remove ? (format === 'number' ? line.replace(/^\d+\. /u, '') : line.slice(prefix.length)) : (format === 'number' ? `${index + 1}. ` : prefix) + line.replace(/^(?:#{1,6}|[-*]|\d+\.) /u, '')).join('\n');
    return { value: value.slice(0, lineStart) + result + value.slice(lineEnd), start: lineStart, end: lineStart + result.length };
}
interface Draft {
    value: string;
    original: string;
    editing: boolean;
    start: number;
    end: number;
}
const drafts = new WeakMap<AppContext, Map<string, Draft>>();
/** Called only after a confirmed workspace reset. Never carry deleted drafts into new fixtures. */
export function clearDocumentDrafts(ctx: AppContext): void { drafts.delete(ctx); }
export interface MarkdownDocument {
    key: string;
    compactToolbar?: boolean;
    label?: string;
    content(): string;
    about?: string;
    save(content: string, original: string): Promise<boolean>;
}
/** Direct Markdown editing with a read view. Unsaved drafts survive closing/switching in this session. */
export function createMarkdownEditor(ctx: AppContext, document: MarkdownDocument): {
    root: HTMLElement;
    toolbar: HTMLElement;
    edit(): void;
} {
    let cache = drafts.get(ctx);
    if (!cache) {
        cache = new Map();
        drafts.set(ctx, cache);
    }
    let savedDraft = cache.get(document.key);
    if (!savedDraft || savedDraft.value === savedDraft.original) {
        savedDraft = { value: document.content(), original: document.content(), editing: false, start: 0, end: 0 };
        cache.set(document.key, savedDraft);
    }
    const draft = savedDraft;
    const root = h('div', { class: 'document-workspace' });
    const toolbar = h('div', { class: 'document-format-toolbar', role: 'toolbar', 'aria-label': 'Document formatting' });
    let input: HTMLTextAreaElement | null = null;
    let saving = false;
    const remember = (): void => {
        if (input) {
            draft.value = input.value;
            draft.start = input.selectionStart;
            draft.end = input.selectionEnd;
        }
    };
    const draw = (): void => {
        toolbar.replaceChildren();
        const formats: readonly {
            id: MarkdownFormat;
            text: string;
            label: string;
        }[] = [
            { id: 'bold', text: 'B', label: 'Bold' }, { id: 'italic', text: 'I', label: 'Italic' },
            { id: 'h1', text: 'H1', label: 'Heading 1' }, { id: 'h2', text: 'H2', label: 'Heading 2' }, { id: 'h3', text: 'H3', label: 'Heading 3' },
            { id: 'bullet', text: '• ≡', label: 'Bulleted list' }, { id: 'number', text: '1 ≡', label: 'Numbered list' },
        ];
        const tools = h('div', { class: 'format-buttons' });
        for (const format of formats) {
            const control = button(format.text, () => {
                remember();
                draft.editing = true;
                const next = formatMarkdown(draft.value, draft.start, draft.end, format.id);
                draft.value = next.value;
                draft.start = next.start;
                draft.end = next.end;
                draw();
                input?.focus({ preventScroll: true });
                input?.setSelectionRange(next.start, next.end);
            }, { variant: 'ghost', className: `format-${format.id}`, disabled: saving });
            control.setAttribute('aria-label', format.label);
            control.dataset['tooltip'] = format.label;
            control.addEventListener('pointerdown', (event) => { remember(); event.preventDefault(); });
            tools.append(control);
        }
        toolbar.append(tools);
        if (draft.editing) {
            toolbar.append(button('Cancel edits', () => { draft.value = document.content(); draft.original = draft.value; draft.editing = false; draw(); }, { variant: 'ghost' }), button(saving ? 'Saving…' : 'Save document', () => {
                if (saving)
                    return;
                remember();
                saving = true;
                const value = draft.value;
                draw();
                void document.save(value, draft.original).then((ok) => {
                    saving = false;
                    if (ok) {
                        draft.original = value;
                        draft.value = value;
                        draft.editing = false;
                    }
                    draw();
                }).catch(() => { saving = false; ctx.toast('The document could not be saved. Your draft is still here.', true); draw(); });
            }, { variant: 'primary', disabled: saving }));
        }
        else if (!document.compactToolbar)
            toolbar.append(button('Edit document', () => { draft.editing = true; draw(); input?.focus({ preventScroll: true }); }, { variant: 'ghost', icon: 'edit' }));
        const paper = h('div', { class: 'editor-reading-column' });
        if (document.about)
            paper.append(h('aside', { class: 'document-about' }, markdown(document.about)));
        if (draft.editing) {
            input = h('textarea', { class: 'markdown-editor direct-markdown', 'aria-label': document.label ?? 'Markdown content', spellcheck: false, disabled: saving });
            input.value = draft.value;
            const status = h('p', { class: 'draft-status', role: 'status' }, draft.value === draft.original ? 'Markdown · private local document' : 'Unsaved draft · kept for this session');
            input.addEventListener('input', () => { remember(); status.textContent = draft.value === draft.original ? 'Markdown · private local document' : 'Unsaved draft · kept for this session'; });
            input.addEventListener('select', remember);
            input.addEventListener('keydown', (event) => {
                if ((event.metaKey || event.ctrlKey) && ['b', 'i', 's'].includes(event.key.toLowerCase())) {
                    event.preventDefault();
                    if (event.key.toLowerCase() === 's')
                        toolbar.querySelector<HTMLButtonElement>('.primary')?.click();
                    else
                        tools.querySelector<HTMLButtonElement>(event.key.toLowerCase() === 'b' ? '.format-bold' : '.format-italic')?.click();
                }
            });
            paper.append(status, input);
        }
        else {
            input = null;
            paper.append(markdown(draft.value, true, { openArtifact: (id) => ctx.openArtifact(id) }));
        }
        root.replaceChildren(paper);
    };
    draw();
    return { root, toolbar, edit: () => { draft.editing = true; draw(); input?.focus({ preventScroll: true }); } };
}
