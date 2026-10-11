import type { AppContext } from '../../app/context.js';
import { h } from '../../components/dom.js';
import { icon } from '../../components/icons.js';
import { openGoal } from '../goals/goals.js';
import { searchWorkspace } from './search-model.js';
import type { SearchResult } from './search-model.js';
/** Search is a modal over the current workspace, not a navigation destination. */
export function openSearch(ctx: AppContext): void {
    if (document.querySelector('.search-dialog')) {
        document.getElementById('workspace-search')?.focus();
        return;
    }
    const input = h('input', { id: 'workspace-search', type: 'text', placeholder: 'Search', 'aria-label': 'Search workspace', role: 'combobox', 'aria-autocomplete': 'list', 'aria-controls': 'workspace-search-results', 'aria-expanded': 'true', autocomplete: 'off', autofocus: true });
    const list = h('div', { class: 'search-results', id: 'workspace-search-results', role: 'listbox', 'aria-label': 'Search results' });
    const heading = h('h3', { class: 'search-section-label' }, 'Recents');
    const body = h('div', { class: 'search-overlay-content' }, h('div', { class: 'search-input-row' }, icon('search', 18), input), heading, list, h('p', { class: 'sr-only', id: 'search-instructions' }, 'Use up and down arrows to select a result, Enter to open, and Escape to close.'));
    input.setAttribute('aria-describedby', 'search-instructions');
    const ref = ctx.dialogs.open('Search', body, 'search-dialog');
    const hiddenClose = ref.element.querySelector<HTMLButtonElement>('.dialog-header button');
    if (hiddenClose)
        hiddenClose.hidden = true;
    let results: SearchResult[] = [];
    let selected = 0;
    const activate = (item: SearchResult): void => {
        ref.close();
        switch (item.kind) {
            case 'chat':
                ctx.navigate('chat', item.id);
                break;
            case 'artifact':
                ctx.openArtifact(item.id);
                break;
            case 'goal':
                openGoal(ctx, item.id);
                break;
            case 'memory': {
                const memory = ctx.state.memory.find((entry) => entry.id === item.id);
                if (memory)
                    ctx.ui.memoryFilter = memory.kind;
                ctx.navigate('memory');
                break;
            }
        }
    };
    const select = (index: number): void => {
        selected = index;
        for (const [i, row] of [...list.querySelectorAll<HTMLElement>('[role="option"]')].entries()) {
            row.setAttribute('aria-selected', String(i === selected));
            row.classList.toggle('selected', i === selected);
            if (i === selected) {
                input.setAttribute('aria-activedescendant', row.id);
                row.scrollIntoView({ block: 'nearest' });
            }
        }
    };
    const render = (): void => {
        results = searchWorkspace(ctx.state, input.value, ctx.ui.recentChatIds);
        selected = 0;
        heading.textContent = input.value.trim() ? 'Results' : 'Recents';
        list.replaceChildren();
        for (const [index, item] of results.entries()) {
            const row = h('button', { type: 'button', role: 'option', id: `search-option-${index}`, 'aria-label': item.title, tabindex: -1, 'aria-selected': String(index === selected), class: `search-result ${index === selected ? 'selected' : ''}` }, h('span', { class: 'search-result-icon' }, icon(item.icon, 18)), h('span', { class: 'search-result-copy' }, h('strong', {}, item.title), h('span', { class: 'search-result-summary' }, h('span', {}, item.preview), h('time', {}, item.time ? `· ${item.time}` : ''))));
            row.addEventListener('click', () => activate(item));
            row.addEventListener('pointermove', () => select(index));
            list.append(row);
        }
        if (results.length)
            input.setAttribute('aria-activedescendant', 'search-option-0');
        else {
            input.removeAttribute('aria-activedescendant');
            list.append(h('p', { class: 'search-empty', role: 'status' }, `No results for “${input.value.trim()}”`));
        }
    };
    input.addEventListener('input', render);
    input.addEventListener('keydown', (event) => {
        if (['ArrowDown', 'ArrowUp'].includes(event.key)) {
            event.preventDefault();
            if (results.length)
                select((selected + (event.key === 'ArrowDown' ? 1 : -1) + results.length) % results.length);
        }
        if (event.key === 'Enter') {
            event.preventDefault();
            const item = results[selected];
            if (item)
                activate(item);
        }
    });
    render();
    input.focus({ preventScroll: true });
}
