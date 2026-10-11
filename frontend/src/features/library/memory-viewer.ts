import { menu } from '../../components/ui.js';
import { identityDocumentId } from '../../presentation/format.js';
import type { AppContext } from '../../app/context.js';
import { createMarkdownEditor } from './markdown-editor.js';
export function openMemoryViewer(ctx: AppContext, id: string): void {
    const entry = ctx.state.memory.find((note) => note.id === id);
    if (!entry) {
        ctx.toast('This note is no longer available.', true);
        return;
    }
    const title = entry.title.endsWith('.md') ? entry.title : `${entry.title}.md`;
    const editor = createMarkdownEditor(ctx, {
        key: `memory:${id}`, label: entry.title, compactToolbar: id === identityDocumentId,
        content: () => ctx.state.memory.find((note) => note.id === id)?.body ?? entry.body,
        about: id === identityDocumentId ? '**About this file.** This is how hatchling describes itself. It records the name you gave hatchling, its character, and how it comes across. hatchling reads it at the start of each conversation so it stays consistent, and updates it when the two of you change these things together. You can edit it at any time. This note is not part of the file.' : id === 'identity-soul' ? '**About this file.** This is hatchling’s persona. It sets out the values and habits hatchling tries to hold to in every conversation. It starts from a template that ships with hatchling. hatchling may refine it over time and tells you when it does. You can edit it at any time. This notice is not part of the file.' : '**About this file.** Your editable local memory notes. Keep passwords, verification codes, tokens, and payment details out of this document. This notice is not part of the file.',
        save: async (body, original) => (await ctx.run({ type: 'memory.save', id, title: entry.title, body, expectedBody: original }, true)).ok,
    });
    const ref = ctx.dock.open(title, editor.root, `memory-dock ${id === identityDocumentId ? 'identity-document-dock' : ''}`, undefined, id === identityDocumentId ? { expanded: true, preserveRail: true } : {});
    ref.element.querySelector('.dock-header-actions')?.before(editor.toolbar);
    if (id === identityDocumentId) {
        const actions = ref.element.querySelector('.dock-header-actions');
        const expand = actions?.querySelector<HTMLButtonElement>('button');
        const options = menu('Document options', [
            { label: 'Edit document', icon: 'edit', action: editor.edit },
            { label: 'Download Markdown', icon: 'download', action: () => { void ctx.run({ type: 'support.download', title: 'IDENTITY', body: ctx.state.memory.find(note => note.id === id)?.body ?? entry.body }); } },
            { label: 'Switch split / full view', icon: 'expand', action: () => expand?.click() },
        ]);
        expand?.remove();
        actions?.prepend(options);
    }
}
