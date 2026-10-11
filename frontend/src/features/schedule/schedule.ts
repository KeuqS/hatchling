import type { AppContext } from '../../app/context.js';
import { h, valueOf } from '../../components/dom.js';
import { icon } from '../../components/icons.js';
import { button, emptyState, field, infoRow, menu, pill, selectField, tabs, toggle } from '../../components/ui.js';
export function renderSchedule(ctx: AppContext): HTMLElement {
    const page = h('div', { class: 'page schedule-page reading-wide', 'data-scroll': 'schedule' }, h('header', { class: 'page-header' }, h('h1', {}, 'Scheduled work'), button('New schedule', () => createJob(ctx), { icon: 'plus', variant: 'primary' })), h('p', { class: 'page-intro muted' }, 'A clear time, a useful result, and a stopping point.'), tabs([{ id: 'all', label: 'All work' }, { id: 'time', label: 'Time-based' }, { id: 'hook', label: 'Event hooks' }], ctx.ui.scheduleFilter, (id) => { ctx.ui.scheduleFilter = id; ctx.render(); }));
    const jobs = ctx.state.jobs.filter((job) => ctx.ui.scheduleFilter === 'all' || job.kind === ctx.ui.scheduleFilter);
    for (const job of jobs)
        page.append(h('article', { class: 'job-card' }, h('header', {}, h('span', { class: 'feature-icon' }, icon(job.kind === 'hook' ? 'link' : 'clock', 22)), h('div', { class: 'row-copy' }, h('h2', {}, job.title), h('p', { class: 'muted text-small' }, job.schedule)), pill(job.health, job.health === 'ok' ? 'success' : job.health === 'errored' ? 'failed' : ''), menu(`Manage ${job.title}`, [{ label: 'Edit schedule', icon: 'edit', action: () => createJob(ctx, job.id) }, { label: job.health === 'disabled' ? 'Enable schedule' : 'Pause schedule', icon: job.health === 'disabled' ? 'play' : 'pause', action: () => { void ctx.run({ type: 'job.toggle', id: job.id }); } }, { label: 'Run sample now', icon: 'play', disabled: job.health !== 'ok', action: () => ctx.dialogs.confirm('Run this sample job?', 'This creates a local demonstration result and records its delivery. A one-shot job then disables itself.', () => { void ctx.run({ type: 'job.run', id: job.id }); }, { label: 'Run sample' }) }, { label: 'Remove schedule', icon: 'trash', danger: true, action: () => ctx.dialogs.confirm('Remove scheduled work?', 'Future runs stop. Previously delivered results stay in the delivery ledger.', () => { void ctx.run({ type: 'job.delete', id: job.id }); }, { danger: true, label: 'Remove schedule' }) }])), h('p', { class: 'job-limit' }, icon('shield', 14), job.limit), job.kind === 'hook' ? h('p', { class: 'text-small muted' }, `${job.source} → ${job.event}`) : null, h('div', { class: 'job-dates' }, h('div', {}, h('span', { class: 'eyebrow' }, 'Last occurrence'), h('p', {}, job.lastRun)), h('div', {}, h('span', { class: 'eyebrow' }, 'Next'), h('p', {}, job.nextRun)))));
    if (!jobs.length)
        page.append(emptyState('clock', 'No scheduled work in this view.'));
    page.append(h('div', { class: 'notice muted' }, icon('clock', 20), 'Schedules are local fixtures. This frontend does not execute background work when the page is closed.'), button('Notification preferences', () => notificationPreferences(ctx), { icon: 'bell' }));
    return page;
}
/** One editor for the compact context pane and the full workspace. Recurrence is always reapproved. */
export function createJob(ctx: AppContext, id?: string): void {
    const existing = ctx.state.jobs.find((job) => job.id === id);
    let kind: 'time' | 'hook' = existing?.kind ?? 'time';
    let recurring = existing?.recurring ?? false;
    const draft = { title: existing?.title ?? '', schedule: existing?.schedule ?? '', source: existing?.source ?? 'GitHub', event: existing?.event ?? 'Pull request opened' };
    const body = h('div', { class: 'stack' });
    const ref = ctx.dialogs.open(existing ? 'Edit schedule' : 'Arrange a useful result', body);
    const render = (): void => {
        const approvalLabel = (): string => recurring ? (existing ? 'Save and approve this recurrence' : 'Create and approve this recurrence') : (existing ? 'Save one run only' : 'Create one run only');
        const form = h('form', { class: 'stack' }, tabs([{ id: 'time', label: 'At a time' }, { id: 'hook', label: 'When something changes' }], kind, (next) => { kind = next; render(); }), field('What should happen?', 'title', draft.title, { required: true, placeholder: 'Prepare a reading shortlist' }), field(kind === 'time' ? 'When · include time and timezone' : 'Delivery timing', 'schedule', draft.schedule, { required: true, placeholder: kind === 'time' ? 'October 1, 2026 at 9 am America/New_York' : 'When the chosen event arrives' }));
        if (kind === 'hook')
            form.append(selectField('Connected source', 'source', ctx.state.connectors.filter((item) => item.state === 'connected').map((item) => ({ value: item.name, label: item.name })), draft.source), selectField('Event', 'event', [{ value: 'Pull request opened', label: 'Pull request opened' }, { value: 'New message', label: 'New message' }, { value: 'File changed', label: 'File changed' }], draft.event));
        const approval = h('button', { class: 'button primary', type: 'submit' }, approvalLabel());
        form.append(toggle('Repeat this schedule', recurring, (value) => { recurring = value; approval.textContent = approvalLabel(); }, 'Off means exactly one delivery, then the job is disabled.'), h('p', { class: 'notice text-small' }, 'The timing and recurrence below are the only work you authorize. Routine or unchanged results stay silent.'), approval);
        const remember = (): void => {
            draft.title = valueOf(form, 'title');
            draft.schedule = valueOf(form, 'schedule');
            if (kind === 'hook') {
                draft.source = valueOf(form, 'source');
                draft.event = valueOf(form, 'event');
            }
        };
        form.addEventListener('input', remember);
        form.addEventListener('change', remember);
        form.addEventListener('submit', (event) => {
            event.preventDefault();
            remember();
            const submit = (): void => {
                const fields = { title: draft.title, kind, schedule: draft.schedule, recurring, source: kind === 'hook' ? draft.source : '', event: kind === 'hook' ? draft.event : '' };
                void ctx.run(existing ? { type: 'job.update', id: existing.id, ...fields } : { type: 'job.create', ...fields }).then((result) => {
                    if (result.ok)
                        ref.close();
                });
            };
            if (recurring)
                ctx.dialogs.confirm('Approve recurring work?', `Repeat on this exact schedule: ${draft.schedule}. This is separate from approval for a single run.`, submit, { label: 'Approve recurrence' });
            else
                submit();
        });
        body.replaceChildren(form);
    };
    render();
}
export function renderNotifications(ctx: AppContext): HTMLElement {
    const page = h('div', { class: 'page notifications-page reading-wide', 'data-scroll': 'notifications' }, h('header', { class: 'page-header' }, h('h1', {}, 'Your daily digest'), button('Preferences', () => notificationPreferences(ctx), { icon: 'settings' })), h('p', { class: 'page-intro muted' }, `One edition around ${ctx.state.settings.digestTime}. Urgent items arrive on their own.`));
    const delivered = ctx.state.notifications.filter((item) => item.stage === 'delivered');
    page.append(h('h2', { class: 'feed-date' }, 'Wednesday, September 23'));
    for (const item of delivered)
        page.append(h('article', { class: `digest-card ${item.read ? 'read' : ''}` }, h('header', {}, h('span', { class: 'row-icon' }, icon(item.urgent ? 'warning' : 'bell', 20)), h('h3', {}, item.title), h('time', { class: 'tiny muted' }, item.time)), h('p', {}, item.text), h('div', { class: 'action-row' }, item.evidenceId ? button('Open evidence', () => ctx.openThread(item.evidenceId ?? ''), { icon: 'file', variant: 'ghost' }) : null, item.offer ? button(item.offer, () => { void ctx.run({ type: 'notification.answer', id: item.id }); ctx.navigate('chat'); }, { variant: 'primary' }) : null, !item.read ? button('Mark read', () => { void ctx.run({ type: 'notification.read', id: item.id }, true); }, { icon: 'check', variant: 'ghost' }) : pill('Read'))));
    const candidates = ctx.state.notifications.filter((item) => item.stage === 'candidate');
    const staging = h('details', { class: 'digest-staging' }, h('summary', {}, `${candidates.length} verified candidate awaiting the next quiet moment`), ...candidates.map((item) => infoRow(item.title, item.text, 'clock')), h('p', { class: 'muted text-small' }, 'Producer → verified candidate → daily composer → delivery ledger. No duplicate delivery is created from an already delivered item.'));
    page.append(staging, h('h2', { class: 'section-heading' }, 'Delivery ledger'), h('div', { class: 'ledger' }, ...delivered.map((item) => h('div', {}, h('time', {}, item.time), h('span', {}, item.title), pill(item.urgent ? 'Urgent · sent alone' : 'Daily edition')))));
    return page;
}
export function notificationPreferences(ctx: AppContext): void {
    const settings = ctx.state.settings;
    const form = h('form', { class: 'stack' }, field('Daily edition time', 'time', settings.digestTime, { type: 'time', required: true }), h('div', { class: 'field-grid' }, field('Quiet hours begin', 'start', settings.quietStart, { type: 'time' }), field('Waking hours begin', 'end', settings.quietEnd, { type: 'time' })), field('Daily allowance', 'allowance', String(settings.dailyAllowance), { type: 'number', min: 1, max: 10 }), h('p', { class: 'muted text-small' }, 'Regular editions wait for waking hours and a quiet moment. Urgent items go immediately and alone. No-op updates stay silent.'), h('button', { class: 'button primary', type: 'submit' }, 'Save preferences'));
    const ref = ctx.dialogs.open('Notification preferences', form);
    form.addEventListener('submit', (event) => {
        event.preventDefault();
        void ctx.run({ type: 'settings.digest', time: valueOf(form, 'time'), start: valueOf(form, 'start'), end: valueOf(form, 'end'), allowance: Number(valueOf(form, 'allowance')) }).then((result) => {
            if (result.ok)
                ref.close();
        });
    });
}
