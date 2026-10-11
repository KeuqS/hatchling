import type { AppContext } from '../../app/context.js';
import type { BrowserTask, DeepReadonly } from '../../protocol/models.js';
import { h, replacePreservingFocus } from '../../components/dom.js';
import { icon } from '../../components/icons.js';
import { button, iconButton } from '../../components/ui.js';
import { resizeHandle } from '../../components/resize.js';
type Task = DeepReadonly<BrowserTask>;
export function browserLabel(task: Task): string {
    if (task.state === 'completed')
        return 'Completed';
    if (task.state === 'failed')
        return 'Failed';
    if (task.state === 'cancelled')
        return 'Stopped';
    if (task.state === 'queued' || task.state === 'assigned')
        return 'Queued';
    if (task.controller === 'switching')
        return 'Switching control';
    if (task.controller === 'user')
        return 'You have control';
    return task.state === 'parked' ? 'Waiting for you' : 'Working';
}
/** Who is driving, for the browser's control pill: never ambiguous, including while control changes hands. */
export function controlPill(controller: BrowserTask['controller']): {
    label: string;
    tone: string;
} {
    switch (controller) {
        case 'user':
            return { label: 'You have control', tone: 'success' };
        case 'switching':
            return { label: 'Switching control', tone: 'attention' };
        case 'agent':
            return { label: 'Watching hatchling', tone: '' };
    }
}
/** While control changes hands nobody drives: the take and return controls are one disabled control . */
export function switchingControl(): HTMLButtonElement {
    return button('Switching control…', () => undefined, { icon: 'refresh', disabled: true });
}
function terminal(task: Task): boolean { return ['cancelled', 'completed', 'failed'].includes(task.state); }
export function browserFrame(task: Task, thumbnail = false, scripted = true): HTMLElement {
    const frame = task.frame;
    const view = h('div', { class: `browser-frame ${thumbnail ? 'is-thumbnail' : ''}`, 'data-sequence': frame?.sequence ?? 0 });
    if (frame?.image)
        view.append(h('img', { class: 'recorded-browser-frame', src: frame.image, alt: scripted ? 'Synthetic site-challenge example; no live website is connected' : `Browser page: ${frame.title || task.site}`, draggable: false }));
    else if (!scripted) {
        view.append(h('p', { class: 'sample-page-caption', role: 'status' }, 'Waiting for the adapter’s browser preview. No page state is inferred.'));
    }
    else {
        view.append(h('div', { class: 'sample-tabbar' }, icon('globe', 12), task.site, h('span', {}, '×')), h('div', { class: 'sample-address' }, icon('arrow', 12), icon('refresh', 12), h('span', {}, icon('lock', 10), task.site)), h('div', { class: 'sample-search-page' }, h('div', { class: 'sample-search-logo' }, frame?.page === 'results' ? 'Search results' : 'Sample search'), h('div', { class: 'sample-searchbox' }, icon('search', 18), h('span', {}, frame?.page === 'home' ? '' : frame?.query ?? ''), h('i', { class: 'sample-caret' })), frame?.page === 'results' ? h('div', { class: 'sample-result' }, h('strong', {}, 'Local browser demonstration'), h('p', {}, 'The search, permission handoff and return-control flow completed. This is an illustrative result, not a web search.')) : h('span', { class: 'sample-page-caption' }, 'Local sample page · no external request')));
    }
    return view;
}
export function browserMessageCard(ctx: AppContext, task: Task): HTMLElement {
    const card = h('article', { class: `chat-browser-card state-${task.state}`, 'aria-label': 'Browser session card', 'data-task-id': task.id }, h('header', {}, h('span', { class: `browser-card-glyph ${task.state === 'running' && task.controller === 'agent' ? 'is-live' : ''}` }, icon('globe', 22)), h('div', {}, h('strong', {}, 'Browser'), h('p', {}, browserLabel(task)))));
    const preview = h('button', { type: 'button', class: 'browser-thumbnail', 'aria-label': 'Expand browser preview' }, browserFrame(task, true, ctx.state.mode === 'scripted'));
    preview.addEventListener('click', () => ctx.openBrowser(task.id));
    card.append(preview, button(terminal(task) ? 'Open preview' : 'Open browser', () => ctx.openBrowser(task.id), { className: 'open-browser-button' }));
    return card;
}
/** Uses the same persistent split workspace as artifacts. Closing never cancels the task. */
export function openBrowserSession(ctx: AppContext, id: string): void {
    if (!ctx.state.browserTasks.some(item => item.id === id)) {
        ctx.toast('This browser session is no longer available.', true);
        return;
    }
    const content = h('section', { class: 'browser-session-content', 'aria-label': 'Browser session viewport' });
    const ref = ctx.dock.open('Browser session', content, 'browser-session-dock');
    const header = ref.element.querySelector<HTMLElement>('.dock-header');
    let width = innerWidth * .70;
    const resize = resizeHandle({ label: 'Browser workspace width', className: 'browser-workspace-resizer', direction: -1,
        value: () => width, bounds: () => [Math.min(480, innerWidth - 320), innerWidth - 320],
        change: value => { width = value; document.body.style.setProperty('--browser-workspace-width', `${value}px`); } });
    ref.element.append(resize);
    let signature = '';
    const draw = (): void => {
        const task = ctx.state.browserTasks.find(item => item.id === id);
        if (!task) {
            ref.close();
            return;
        }
        const next = JSON.stringify(task);
        if (next === signature)
            return;
        signature = next;
        const stopped = terminal(task);
        const status = h('div', { class: 'browser-session-title' }, h('h2', { id: 'artifact-dock-title' }, 'Browser session'), h('p', { role: 'status' }, `${browserLabel(task)} · ${task.site}`));
        const actions = h('div', { class: 'browser-session-actions' });
        if (!stopped) {
            if (task.state === 'queued' || task.state === 'assigned')
                actions.append(button('Start sample task', () => { void ctx.run({ type: 'browser.start', id }, true); }, { variant: 'primary' }));
            else
                actions.append(button('Stop', () => { void ctx.run({ type: 'browser.cancel', id }, true); }, { icon: 'stop', className: 'browser-stop', variant: 'ghost' }), task.controller === 'switching' ? switchingControl() : task.controller === 'agent' ? button('Take control of the browser', () => { void ctx.run({ type: 'browser.takeover', id }, true); }, { variant: 'primary' }) : button('Return control to agent', () => { void ctx.run({ type: 'browser.resume', id }, true); }, { variant: 'primary', disabled: task.challenge === 'waiting' || task.challenge === 'persistent' }));
        }
        const close = iconButton('close', 'Close browser session', ref.close);
        close.removeAttribute('data-tooltip');
        close.removeAttribute('title');
        actions.append(close);
        if (header)
            replacePreservingFocus(header, status, actions);
        const tabs = h('div', { class: 'browser-session-tabs' }, h('span', {}, icon('globe', 14), task.site), h('small', {}, ctx.state.mode === 'scripted' ? 'Scripted preview' : 'Adapter preview'));
        const viewport = h('div', { class: 'browser-session-stage', tabindex: 0, 'aria-label': 'Browser preview screen' }, browserFrame(task, false, ctx.state.mode === 'scripted'));
        if (!stopped && task.controller === 'user' && ctx.state.mode === 'scripted') {
            const tools = h('form', { class: 'local-takeover-controls', 'aria-label': 'Local takeover controls' }, h('strong', {}, 'You have control · local demo'));
            if (task.challenge === 'waiting' || task.challenge === 'persistent')
                tools.append(button('Complete sample challenge', () => { void ctx.run({ type: 'browser.solve', id }, true); }, { icon: 'check', variant: 'primary' }));
            else {
                const input = h('input', { name: 'browser-query', type: 'search', 'aria-label': 'Sample browser search', value: task.frame?.query ?? '', maxlength: 240 });
                tools.append(input, h('button', { type: 'submit', class: 'button primary' }, 'Search sample page'));
                tools.addEventListener('submit', event => { event.preventDefault(); void ctx.run({ type: 'browser.input', id, text: input.value }, true); });
            }
            viewport.append(tools);
        }
        const controls = h('div', { class: 'browser-session-footer' }, h('span', {}, ctx.state.mode !== 'scripted' ? 'Adapter-supplied preview. Direct interactive input requires an approved live-session integration.' : stopped ? 'Last recorded preview · read-only' : task.controller === 'user' ? 'The agent is paused while you control the sample.' : 'Watching the local sample session. No external browser is connected.'));
        if (task.challenge === 'waiting' && task.controller === 'agent' && !stopped)
            controls.append(button('Allow this challenge once', () => { void ctx.run({ type: 'browser.grant', id }, true); }, { variant: 'link' }));
        replacePreservingFocus(content, tabs, viewport, controls);
    };
    draw();
    const unsubscribe = ctx.adapter.subscribe(draw);
    ref.element.addEventListener('close', unsubscribe, { once: true });
}
