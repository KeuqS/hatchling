import type { AppContext } from '../../app/context.js';
import type { AppState, DeepReadonly, SystemEntry } from '../../protocol/models.js';
import { h } from '../../components/dom.js';
import { icon } from '../../components/icons.js';
import { button, emptyState, iconButton, pill } from '../../components/ui.js';
/** Pure selector; search/sort and trash visibility cannot mutate the adapter snapshot. */
export function selectSystemEntries(state: DeepReadonly<AppState>, folder: string, query: string, sort: 'asc' | 'desc'): readonly DeepReadonly<SystemEntry>[] {
    const needle = query.trim().toLocaleLowerCase();
    return state.systemEntries.filter((entry) => entry.parentId === (folder || null) && entry.name.toLocaleLowerCase().includes(needle) && (!entry.artifactId || state.artifacts.some((artifact) => artifact.id === entry.artifactId && !artifact.trashUntil)) && (!entry.memoryId || state.memory.some((note) => note.id === entry.memoryId))).toSorted((a, b) => (sort === 'asc' ? 1 : -1) * a.name.localeCompare(b.name, 'en', { sensitivity: 'base', numeric: true }));
}
export function systemAncestors(entries: readonly DeepReadonly<SystemEntry>[], id: string): readonly DeepReadonly<SystemEntry>[] {
    const result: DeepReadonly<SystemEntry>[] = [];
    const seen = new Set<string>();
    let current = entries.find((entry) => entry.id === id);
    while (current && !seen.has(current.id)) {
        result.unshift(current);
        seen.add(current.id);
        current = entries.find((entry) => entry.id === current?.parentId);
    }
    return result;
}
export interface SystemTreeRow {
    entry: DeepReadonly<SystemEntry>;
    depth: number;
}
/** Folder disclosure is view state; cycle-safe traversal never walks the host filesystem. */
export function visibleSystemRows(state: DeepReadonly<AppState>, folder: string, query: string, sort: 'asc' | 'desc', expanded: ReadonlySet<string>): readonly SystemTreeRow[] {
    const rows: SystemTreeRow[] = [];
    const seen = new Set<string>();
    const walk = (parent: string, depth: number): void => {
        for (const entry of selectSystemEntries(state, parent, query, sort)) {
            if (seen.has(entry.id))
                continue;
            seen.add(entry.id);
            rows.push({ entry, depth });
            if (entry.kind === 'folder' && expanded.has(entry.id))
                walk(entry.id, depth + 1);
        }
    };
    walk(folder, 0);
    return rows;
}
export function renderSystem(ctx: AppContext): HTMLElement {
    const folder = ctx.state.systemEntries.find((entry) => entry.id === ctx.ui.systemFolder);
    const root = ctx.ui.systemFolder === '';
    const page = h('div', { class: 'page system-page', 'data-scroll': `files-${ctx.ui.systemFolder}` }, h('header', { class: 'page-header' }, h('h1', {}, root ? 'System files' : folder?.name ?? 'Folder not found')));
    if (!root) {
        const nav = h('nav', { class: 'system-breadcrumbs', 'aria-label': 'Folder path' }, button('System files', () => ctx.navigate('library', 'system'), { variant: 'ghost' }));
        for (const parent of systemAncestors(ctx.state.systemEntries, ctx.ui.systemFolder))
            nav.append(icon('chevron', 12), button(parent.name, () => ctx.navigate('library', `system/${parent.id}`), { variant: 'ghost' }));
        page.append(nav, h('div', { class: 'system-scope' }, pill(folder?.visibility === 'internal' ? 'Protected internal metadata' : 'User-visible files'), h('p', { class: 'muted text-small' }, folder?.description ?? 'This folder is not present in the local index.')));
    }
    const rows = visibleSystemRows(ctx.state, ctx.ui.systemFolder, ctx.ui.navSearch, ctx.ui.systemSort, ctx.ui.systemExpanded);
    const nameSort = button('Name', () => { ctx.ui.systemSort = ctx.ui.systemSort === 'asc' ? 'desc' : 'asc'; ctx.render(); }, { variant: 'ghost', className: 'system-sort' });
    nameSort.setAttribute('aria-label', `Sort files by name ${ctx.ui.systemSort === 'asc' ? 'descending' : 'ascending'}`);
    const table = h('table', { class: 'system-table', role: 'treegrid', 'aria-label': 'System files', 'aria-colcount': 4, 'aria-rowcount': rows.length + 1 }, h('colgroup', {}, h('col', { class: 'name-column' }), h('col', { class: 'type-column' }), h('col', { class: 'modified-column' }), h('col', { class: 'size-column' })), h('thead', {}, h('tr', {}, h('th', { scope: 'col', 'aria-sort': ctx.ui.systemSort === 'asc' ? 'ascending' : 'descending' }, nameSort), ...['Type', 'Last modified', 'Size'].map((label) => h('th', { scope: 'col' }, label)))));
    const body = h('tbody');
    const focusEntry = (id: string): void => document.getElementById(`file-${id}`)?.focus({ preventScroll: true });
    const toggleFolder = (entry: DeepReadonly<SystemEntry>, force?: boolean): void => {
        const expanded = force ?? !ctx.ui.systemExpanded.has(entry.id);
        if (expanded)
            ctx.ui.systemExpanded.add(entry.id);
        else
            ctx.ui.systemExpanded.delete(entry.id);
        ctx.ui.systemSelected = entry.id;
        ctx.render();
        focusEntry(entry.id);
    };
    for (const [index, row] of rows.entries()) {
        const { entry, depth } = row;
        const folderRow = entry.kind === 'folder';
        const expanded = ctx.ui.systemExpanded.has(entry.id);
        const open = button(entry.name, () => {
            if (folderRow)
                toggleFolder(entry);
            else
                openEntry(ctx, entry);
        }, { variant: 'ghost', className: 'system-entry', id: `file-${entry.id}` });
        open.replaceChildren(h('span', { class: `system-file-icon ${folderRow ? '' : 'document-file-icon'}` }, icon(folderRow ? 'folder' : entry.visibility === 'internal' ? 'lock' : 'file', 18)), h('span', {}, entry.name));
        if (folderRow)
            open.setAttribute('aria-expanded', String(expanded));
        open.title = `${entry.name} · ${entry.visibility === 'internal' ? 'Protected internal metadata' : 'User-visible file'}`;
        const cell = h('div', { class: 'system-cell', style: `--tree-depth:${depth}` }, open);
        if (folderRow)
            cell.append(iconButton('arrow', `Open ${entry.name} folder`, () => ctx.navigate('library', `system/${entry.id}`), 'folder-open-link'));
        const artifact = ctx.state.artifacts.find((item) => item.id === entry.artifactId);
        const memory = ctx.state.memory.find((item) => item.id === entry.memoryId);
        const bytes = artifact ? new TextEncoder().encode(artifact.content).length : memory ? new TextEncoder().encode(memory.body).length : entry.bytes;
        const count = folderRow && expanded ? `${selectSystemEntries(ctx.state, entry.id, '', ctx.ui.systemSort).length} items` : '';
        body.append(h('tr', { 'data-entry': entry.id, 'data-visibility': entry.visibility, 'aria-level': depth + 1, 'aria-rowindex': index + 2, 'aria-selected': ctx.ui.systemSelected === entry.id, 'aria-expanded': folderRow ? String(expanded) : undefined, class: ctx.ui.systemSelected === entry.id ? 'system-selected' : '' }, h('td', { 'data-label': 'Name' }, cell), h('td', { 'data-label': 'Type' }, folderRow ? 'Folder' : entry.reference?.type ?? (entry.kind === 'index' ? 'Protected index' : entry.kind === 'note' ? 'Note' : artifact?.format ?? 'Document')), h('td', { 'data-label': 'Last modified' }, count || entry.reference?.modified || entry.modified || ''), h('td', { 'data-label': 'Size' }, entry.reference?.size ?? (bytes === null ? '' : bytes < 1024 ? `${bytes} B` : `${Math.ceil(bytes / 1024)} KB`))));
        open.addEventListener('keydown', (event) => {
            if (!['ArrowRight', 'ArrowLeft', 'ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key))
                return;
            event.preventDefault();
            if (event.key === 'ArrowRight' && folderRow) {
                if (!expanded)
                    toggleFolder(entry, true);
                else if (rows[index + 1]?.depth === depth + 1)
                    focusEntry(rows[index + 1]?.entry.id ?? entry.id);
            }
            else if (event.key === 'ArrowLeft') {
                if (expanded && folderRow)
                    toggleFolder(entry, false);
                else if (entry.parentId)
                    focusEntry(entry.parentId);
            }
            else {
                const target = event.key === 'Home' ? rows[0] : event.key === 'End' ? rows.at(-1) : rows[index + (event.key === 'ArrowDown' ? 1 : -1)];
                if (target)
                    focusEntry(target.entry.id);
            }
        });
    }
    table.append(body);
    page.append(table);
    if (!rows.length)
        page.append(emptyState('folder', ctx.ui.navSearch ? 'No matching files in this folder.' : folder?.visibility === 'internal' ? 'Internal contents are not exposed.' : 'No sample files in this folder.', 'This is a read-only local index, not access to the host filesystem.'));
    page.append(h('p', { class: 'sample-footnote' }, 'Scripted file index · internal folders expose safe metadata only. Archive names and sizes are synthetic metadata; metadata-only examples have no download.'));
    return page;
}
function openEntry(ctx: AppContext, entry: DeepReadonly<SystemEntry>): void {
    if (entry.artifactId) {
        ctx.openArtifact(entry.artifactId);
        return;
    }
    if (entry.memoryId) {
        ctx.openMemoryDocument(entry.memoryId);
        return;
    }
    ctx.dialogs.open(entry.name, h('div', { class: 'stack' }, pill(entry.reference ? 'Reference listing only' : 'Read-only metadata'), h('p', {}, entry.description)));
}
