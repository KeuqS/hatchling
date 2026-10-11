import { resizeHandle } from '../../components/resize.js';
import type { AppContext } from '../../app/context.js';
import { h, replacePreservingFocus } from '../../components/dom.js';
import { icon } from '../../components/icons.js';
import { iconButton, markdown, menu, pill } from '../../components/ui.js';
import { durationLabel, reportTitleAndBody, stepOutcome, timelineGroups, timeLabel } from './timeline.js';
import type { ReadRow, ReadThread } from './timeline.js';
interface ActivityView {
    selected: string;
    navTop: number;
    scrolls: Map<string, number>;
    showTimes: boolean;
    laneWidth: number;
    laneHeight: number;
    windowWidth: number;
    windowHeight: number;
    followLive: boolean;
}
const sessions = new WeakMap<AppContext, Map<string, ActivityView>>();
function viewFor(ctx: AppContext, thread: ReadThread): ActivityView {
    let map = sessions.get(ctx);
    if (!map) {
        map = new Map();
        sessions.set(ctx, map);
    }
    let view = map.get(thread.id);
    if (!view) {
        view = { selected: thread.state === 'completed' || thread.state === 'failed' ? 'summary' : thread.pendingSteps?.find(item => !thread.lanes.find(lane => lane.id === item.laneId)?.parentId)?.id ?? 'started', navTop: 0, scrolls: new Map(), showTimes: false, laneWidth: 333, laneHeight: 200, windowWidth: 1320, windowHeight: 880, followLive: false };
        map.set(thread.id, view);
    }
    return view;
}
function files(ctx: AppContext, ids: readonly string[]): HTMLElement {
    const root = h('div', { class: 'activity-file-list' });
    for (const id of new Set(ids)) {
        const artifact = ctx.state.artifacts.find((item) => item.id === id && !item.trashUntil);
        if (!artifact) {
            root.append(h('p', { class: 'muted text-small' }, 'An evidence attachment is no longer available.'));
            continue;
        }
        const ext = artifact.title.includes('.') ? artifact.title.split('.').at(-1)?.toUpperCase() ?? 'FILE' : artifact.format.toUpperCase();
        const preview = h('button', { type: 'button', class: `activity-file-preview preview-${artifact.format}`, 'aria-label': `Open ${artifact.title}` });
        if (artifact.format === 'image' && artifact.asset)
            preview.append(h('img', { src: artifact.asset, alt: artifact.title }));
        else if (artifact.content)
            preview.append(h('pre', { 'aria-hidden': 'true' }, artifact.content));
        else
            preview.append(icon('file', 42), h('span', {}, artifact.description));
        preview.addEventListener('click', () => ctx.openArtifact(id));
        const title = h('button', { class: 'activity-file-name', type: 'button' }, h('strong', {}, artifact.title), h('span', {}, ext));
        title.addEventListener('click', () => ctx.openArtifact(id));
        root.append(h('article', { class: 'activity-file-card', 'aria-label': artifact.title }, preview, h('div', { class: 'activity-file-footer' }, h('span', { class: 'activity-file-glyph' }, icon(artifact.format === 'code' ? 'code' : 'file', 24)), title, menu(`Options for ${artifact.title}`, [
            { label: 'Open file', icon: 'file', action: () => ctx.openArtifact(id) },
            { label: 'Download', icon: 'download', action: () => { void ctx.run({ type: 'artifact.download', id }, true); } },
        ]))));
    }
    return root;
}
function timing(startedAt: string | null | undefined, endedAt: string | null | undefined, duration: number | null | undefined, lane: string): HTMLElement {
    const details = h('details', { class: 'execution-details' }, h('summary', {}, icon('clock', 13), 'Execution details'), h('dl', {}, h('dt', {}, 'Agent'), h('dd', {}, lane), h('dt', {}, 'Started (UTC)'), h('dd', {}, timeLabel(startedAt)), h('dt', {}, 'Finished (UTC)'), h('dd', {}, timeLabel(endedAt)), h('dt', {}, 'Duration'), h('dd', {}, durationLabel(duration))));
    return details;
}
function reportPane(ctx: AppContext, thread: ReadThread, selected: string): HTMLElement {
    const report = h('section', { class: 'activity-report-pane', id: 'activity-selected-report', 'data-selected': selected, 'aria-label': 'Selected activity report', tabindex: 0 });
    const options = { openArtifact: (id: string): void => ctx.openArtifact(id) };
    const pending = thread.pendingSteps?.find(item => item.id === selected);
    if (pending) {
        report.append(h('h3', {}, pending.title), markdown(pending.report, true, options), h('p', { class: 'pending-report-status' }, thread.state === 'running' ? 'In progress — no result has been reported yet.' : 'Paused — this step has no recorded outcome.'), timing(pending.startedAt, null, null, thread.lanes.find(lane => lane.id === pending.laneId)?.name ?? 'Agent'));
    }
    else if (selected === 'summary') {
        report.append(h('h3', {}, 'Summary'), markdown(thread.summary || thread.finishMessage || thread.subtitle, true, options));
        const ids = thread.evidenceArtifactIds ?? (thread.evidenceArtifactId ? [thread.evidenceArtifactId] : []);
        if (ids.length)
            report.append(files(ctx, ids));
        if (thread.verdict)
            report.append(h('p', { class: `finish-verdict ${thread.verdict}` }, `Finish verdict: ${thread.verdict}`));
        report.append(timing(thread.startedAt, thread.endedAt, thread.startedAt && thread.endedAt ? Date.parse(thread.endedAt) - Date.parse(thread.startedAt) : null, 'Main agent'));
    }
    else if (selected === 'started') {
        report.append(h('h3', {}, 'Started'), h('p', {}, thread.name), h('p', { class: 'muted' }, 'The task began with the recorded scope. Select a step in the timeline to read its evidence.'), timing(thread.startedAt, null, null, 'Main agent'));
    }
    else {
        const row = thread.lanes.flatMap((lane) => lane.rows).find((item) => item.id === selected);
        if (!row)
            report.append(h('p', {}, 'This step is no longer available. Select another entry.'));
        else {
            const copy = reportTitleAndBody(row);
            report.append(h('h3', {}, copy.title), markdown(copy.body, true, options));
            if (row.artifactIds?.length)
                report.append(files(ctx, row.artifactIds));
            const outcome = stepOutcome(row.status);
            report.append(h('p', { class: `activity-report-outcome ${row.status}` }, icon(outcome.reportIcon, 14), outcome.sentence ?? row.subtitle), timing(row.startedAt, row.endedAt, row.durationMs, thread.lanes.find((lane) => lane.id === row.laneId)?.name ?? 'Agent'));
        }
    }
    if (ctx.state.mode === 'scripted')
        report.append(h('p', { class: 'activity-provenance' }, thread.id === 'dynamic04' ? 'Scripted sample trace. Times are illustrative; attachments are synthetic local examples, not results from a live agent.' : 'Scripted sample evidence · no live tools or external services ran. Missing timestamps are not inferred.'));
    return report;
}
/** One selected record, two independent scroll panes. Streaming updates preserve selection and scroll. */
export function openActivity(ctx: AppContext, id: string): void {
    const initial = ctx.state.threads.find((item) => item.id === id);
    if (!initial) {
        ctx.toast('This activity is no longer available.', true);
        return;
    }
    const view = viewFor(ctx, initial);
    const content = h('div', { class: 'activity-split' });
    const ref = ctx.dialogs.open(initial.name, content, 'thread-dialog activity-dialog');
    ref.element.dataset['threadId'] = id;
    ref.element.style.setProperty('--activity-nav-width', `${view.laneWidth}px`);
    const setWindow = (): void => { ref.element.style.setProperty('--activity-window-width', `${view.windowWidth}px`); ref.element.style.setProperty('--activity-window-height', `${view.windowHeight}px`); };
    setWindow();
    ref.element.append(resizeHandle({ label: 'Activity window width', className: 'activity-window-width', value: () => ref.element.clientWidth, bounds: () => [640, Math.min(1600, innerWidth - 64)], change: value => { view.windowWidth = value; setWindow(); } }), resizeHandle({ label: 'Activity window height', axis: 'y', className: 'activity-window-height', value: () => ref.element.clientHeight, bounds: () => [420, innerHeight - 64], change: value => { view.windowHeight = value; setWindow(); } }));
    const header = ref.element.querySelector<HTMLElement>('.dialog-header');
    const originalTitle = header?.querySelector('h2');
    const labelId = originalTitle?.id ?? 'activity-title';
    let lastSnapshot = JSON.stringify(initial);
    const render = (): void => {
        const priorNav = content.querySelector<HTMLElement>('.activity-timeline');
        const priorReport = content.querySelector<HTMLElement>('.activity-report-pane');
        if (priorNav)
            view.navTop = priorNav.scrollTop;
        if (priorReport?.dataset['selected'])
            view.scrolls.set(priorReport.dataset['selected'], priorReport.scrollTop);
        const thread = ctx.state.threads.find((item) => item.id === id);
        if (!thread) {
            ref.close();
            return;
        }
        const groups = timelineGroups(thread);
        const allRows = groups.flatMap((group) => group.rows);
        if (!['started', 'summary'].includes(view.selected) && !allRows.some((row) => row.id === view.selected) && !thread.pendingSteps?.some(step => step.id === view.selected))
            view.selected = 'summary';
        const badge = thread.state === 'completed' ? 'Complete' : thread.state === 'failed' ? 'Failed' : thread.state === 'interrupted' ? 'Paused' : 'In Progress';
        const showTimes = iconButton('clock', view.showTimes ? 'Hide step timestamps' : 'Show step timestamps', () => { view.showTimes = !view.showTimes; render(); }, 'activity-times-toggle');
        showTimes.setAttribute('aria-pressed', String(view.showTimes));
        const follow = h('button', { type: 'button', class: 'activity-live-follow', 'aria-pressed': view.followLive }, 'Follow live');
        follow.addEventListener('click', () => { view.followLive = !view.followLive; render(); });
        follow.hidden = thread.state !== 'running';
        if (view.followLive)
            view.selected = thread.state === 'completed' ? 'summary' : thread.pendingSteps?.at(-1)?.id ?? view.selected;
        if (header)
            replacePreservingFocus(header, h('div', { class: 'activity-title-group' }, pill(badge, thread.state === 'failed' ? 'bad' : thread.state === 'completed' ? 'good' : thread.state === 'running' ? 'in-progress' : ''), h('h2', { id: labelId }, thread.name)), h('div', { class: 'activity-header-actions' }, follow, showTimes, iconButton('close', `Close ${thread.name}`, ref.close)));
        const nav = h('nav', { class: `activity-timeline ${view.showTimes ? 'with-times' : ''}`, 'aria-label': 'Activity timeline' });
        const select = (selection: string): void => { view.followLive = false; view.selected = selection; render(); ref.element.querySelector<HTMLElement>(`#${CSS.escape(`timeline-${selection}`)}`)?.scrollIntoView({ block: 'nearest' }); };
        const entry = (selection: string, title: string, row?: ReadRow): HTMLButtonElement => {
            const outcome = stepOutcome(row?.status ?? 'success');
            const node = h('button', { type: 'button', class: `timeline-entry action-record ${selection === view.selected ? 'selected' : ''} ${selection === 'started' ? 'start-entry' : selection === 'summary' ? 'finish-entry' : ''}`, id: `timeline-${selection}`, 'data-work': row?.workId ?? selection, 'data-status': row?.status ?? (selection === 'summary' ? thread.verdict ?? 'success' : 'success'), 'aria-pressed': selection === view.selected, 'aria-controls': 'activity-selected-report', title: row ? `${row.subtitle}\n${timeLabel(row.startedAt)} · ${durationLabel(row.durationMs)}` : title, tabindex: selection === view.selected ? 0 : -1 }, h('span', { class: `timeline-mark ${outcome.mark}` }, selection === 'started' ? h('span', { class: 'timeline-start-dot' }) : icon(outcome.icon, 12)), h('span', { class: 'timeline-entry-copy' }, h('span', {}, title), h('time', { class: 'step-time', datetime: row?.startedAt ?? (selection === 'summary' ? thread.endedAt : thread.startedAt) ?? '' }, timeLabel(row?.startedAt ?? (selection === 'summary' ? thread.endedAt : thread.startedAt)), row?.durationMs !== undefined ? ` · ${durationLabel(row.durationMs)}` : '')));
            node.addEventListener('click', () => select(selection));
            return node;
        };
        const appendPending = (section: HTMLElement, laneId: string): void => {
            for (const step of thread.pendingSteps?.filter(item => item.laneId === laneId) ?? []) {
                const running = thread.state === 'running';
                const node = h('button', { type: 'button', id: `timeline-${step.id}`, class: `timeline-entry pending-entry ${view.selected === step.id ? 'selected' : ''}`, 'data-state': running ? 'running' : 'paused', 'aria-pressed': view.selected === step.id, 'aria-controls': 'activity-selected-report', tabindex: view.selected === step.id ? 0 : -1 }, h('span', { class: running ? 'activity-spinner' : 'pending-paused', 'aria-hidden': 'true' }, icon(running ? 'run' : 'pause', 17)), h('span', { class: 'timeline-entry-copy' }, h('span', {}, step.title), h('time', { class: 'step-time', datetime: step.startedAt ?? '' }, timeLabel(step.startedAt))));
                node.addEventListener('click', () => select(step.id));
                section.append(node);
            }
        };
        const primary = h('section', { class: 'activity-lane', 'data-lane': groups.find((group) => !group.lane.parentId)?.lane.id ?? 'main', 'aria-label': 'Main agent' }, entry('started', 'Started'));
        for (const group of groups.filter((item) => !item.lane.parentId))
            for (const row of group.rows)
                primary.append(entry(row.id, row.title, row));
        for (const group of groups.filter(item => !item.lane.parentId))
            appendPending(primary, group.lane.id);
        if (thread.state !== 'running')
            primary.append(entry('summary', thread.state === 'completed' ? 'Completed' : thread.state === 'failed' ? 'Failed' : 'Paused'));
        nav.append(primary);
        for (const group of groups.filter((item) => item.lane.parentId)) {
            const lane = h('section', { class: 'activity-lane child-lane', 'data-lane': group.lane.id, 'aria-label': group.lane.name }, h('h3', { title: group.lane.name }, `SUBAGENT ${String(group.ordinal).padStart(2, '0')}`));
            for (const row of group.rows)
                lane.append(entry(row.id, row.title, row));
            appendPending(lane, group.lane.id);
            if (thread.state === 'running' && thread.pendingSteps?.some(step => step.laneId === group.lane.id))
                lane.querySelector('h3')?.append(h('span', { class: 'activity-spinner', 'aria-label': 'Subagent working' }, icon('run', 14)));
            nav.append(lane);
        }
        if (thread.state === 'running')
            nav.append(h('div', { class: 'timeline-working', role: 'status' }, h('span', { class: 'activity-spinner', 'aria-hidden': 'true' }, icon('run', 17)), 'Working…'));
        const report = reportPane(ctx, thread, view.selected);
        nav.addEventListener('scroll', () => { view.navTop = nav.scrollTop; }, { passive: true });
        const selected = view.selected;
        report.addEventListener('scroll', () => view.scrolls.set(selected, report.scrollTop), { passive: true });
        nav.addEventListener('keydown', (event) => {
            if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key))
                return;
            const buttons = [...nav.querySelectorAll<HTMLButtonElement>('.timeline-entry')];
            const index = buttons.findIndex((node) => node === document.activeElement);
            const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : Math.max(0, Math.min(buttons.length - 1, index + (event.key === 'ArrowDown' ? 1 : -1)));
            const target = buttons[next];
            if (!target)
                return;
            event.preventDefault();
            const targetId = target.id;
            target.click();
            ref.element.querySelector<HTMLElement>(`#${CSS.escape(targetId)}`)?.focus({ preventScroll: true });
            ref.element.querySelector<HTMLElement>(`#${CSS.escape(targetId)}`)?.scrollIntoView({ block: 'nearest' });
        });
        const mobile = innerWidth <= 700;
        ref.element.style.setProperty('--activity-nav-height', `${view.laneHeight}px`);
        const divider = resizeHandle({ label: mobile ? 'Activity timeline height' : 'Activity timeline width', axis: mobile ? 'y' : 'x', className: 'activity-divider', value: () => mobile ? view.laneHeight : view.laneWidth, bounds: () => mobile ? [100, Math.max(110, content.clientHeight * .65)] : [210, Math.min(580, ref.element.clientWidth * .55)], change: value => {
                if (mobile) {
                    view.laneHeight = value;
                    ref.element.style.setProperty('--activity-nav-height', `${value}px`);
                }
                else {
                    view.laneWidth = value;
                    ref.element.style.setProperty('--activity-nav-width', `${value}px`);
                }
            } });
        replacePreservingFocus(content, nav, divider, report);
        for (const spinner of content.querySelectorAll<HTMLElement>('.activity-spinner'))
            spinner.style.animationDelay = `${-(performance.now() % 900)}ms`;
        nav.scrollTop = view.followLive ? nav.scrollHeight : view.navTop;
        report.scrollTop = view.scrolls.get(view.selected) ?? 0;
    };
    render();
    const unsubscribe = ctx.adapter.subscribe(() => {
        const snapshot = JSON.stringify(ctx.state.threads.find((item) => item.id === id));
        if (snapshot !== lastSnapshot) {
            lastSnapshot = snapshot;
            render();
        }
    });
    ref.element.addEventListener('close', unsubscribe, { once: true });
}
