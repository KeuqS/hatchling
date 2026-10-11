import type { AppContext } from '../../app/context.js';
import type { DeepReadonly, BrowserTask, Purchase } from '../../protocol/models.js';
import { h } from '../../components/dom.js';
import { icon } from '../../components/icons.js';
import { button } from '../../components/ui.js';
import { currency } from '../../presentation/format.js';
import { avatarSource } from '../context/presence.js';
function purchaseChat(ctx: AppContext, id: string): string | undefined { return ctx.state.messages.find(message => message.kind === 'purchase' && message.purchaseId === id)?.chatId; }
function browserChat(ctx: AppContext, id: string): string | undefined { return ctx.state.messages.find(message => message.kind === 'browser' && message.taskId === id)?.chatId; }
function challengePending(task: DeepReadonly<BrowserTask>): boolean { return task.challenge === 'waiting' && task.controller === 'agent' && !['completed', 'cancelled', 'failed'].includes(task.state); }
function reviewPurchase(ctx: AppContext, purchase: DeepReadonly<Purchase>): void {
    const chatId = purchaseChat(ctx, purchase.id);
    if (!chatId)
        return;
    ctx.ui.contextOpen = innerWidth > 1100 && ctx.ui.contextOpen;
    ctx.navigate('chat', chatId);
    const message = ctx.state.messages.find(item => item.kind === 'purchase' && item.purchaseId === purchase.id);
    const row = [...document.querySelectorAll<HTMLElement>('[data-message-id]')].find(item => item.dataset['messageId'] === message?.id);
    row?.scrollIntoView({ block: 'center', behavior: 'instant' });
    const card = row?.querySelector<HTMLElement>('.purchase-card');
    if (card) {
        card.tabIndex = -1;
        card.focus({ preventScroll: true });
    }
}
/** Existing, frozen-contract approvals retain their original commands and scope.
 * Purchases always open the full exact-quote review; never a generic Allow button. */
export function domainApprovalNotice(ctx: AppContext): HTMLElement | null {
    const purchase = ctx.state.purchases.find(item => item.state === 'proposed' && purchaseChat(ctx, item.id) === ctx.ui.chatId);
    const task = ctx.state.browserTasks.find(item => challengePending(item) && browserChat(ctx, item.id) === ctx.ui.chatId);
    if (!purchase && !task)
        return null;
    const title = purchase ? `Review purchase · ${currency(purchase.total)}` : `Site challenge · ${task?.site ?? ''}`;
    const detail = purchase ? `${purchase.merchant} · Review items, payment, delivery, and the exact total before deciding.` : `${task?.step ?? ''}. This permission is for one challenge attempt only.`;
    const card = h('section', { class: 'task-approval-card domain-approval-card', 'aria-label': 'Task approval needed', 'data-domain-approval': purchase?.id ?? task?.id ?? '' }, h('header', { class: 'task-approval-heading' }, h('img', { src: avatarSource(ctx), alt: '', width: 28, height: 28 }), h('span', {}, icon('shield', 14), 'Task approval needed')), h('div', { class: 'task-approval-request' }, h('span', { class: 'task-approval-icon' }, icon(purchase ? 'cart' : 'globe', 20)), h('div', { class: 'task-approval-copy' }, h('strong', {}, title), h('p', {}, detail))));
    const offline = ctx.delivery.connection.kind !== 'ready';
    const actions = h('div', { class: 'task-approval-actions' });
    if (purchase)
        actions.append(button('Review purchase', () => reviewPurchase(ctx, purchase), { variant: 'primary', id: 'review-composer-purchase' }));
    else if (task) {
        const decide = (grant: boolean): void => {
            if (ctx.delivery.connection.kind !== 'ready')
                return;
            void ctx.run({ type: grant ? 'browser.grant' : 'browser.cancel', id: task.id });
        };
        actions.append(button('Deny and stop', () => decide(false), { disabled: offline, id: 'deny-composer-challenge' }), button('View browser', () => ctx.openBrowser(task.id), { id: 'view-composer-challenge' }));
        if (task.attemptsRemaining > 0)
            actions.append(button('Allow challenge once', () => decide(true), { variant: 'primary', disabled: offline, id: 'allow-composer-challenge' }));
    }
    card.append(actions);
    return h('div', { class: 'task-approval-dock' }, card);
}
export function domainPendingCount(ctx: AppContext): number {
    return ctx.state.purchases.filter(item => item.state === 'proposed' && purchaseChat(ctx, item.id)).length + ctx.state.browserTasks.filter(item => challengePending(item) && browserChat(ctx, item.id)).length;
}
export function domainApprovalRecords(ctx: AppContext): HTMLElement[] {
    const records: HTMLElement[] = [];
    for (const purchase of ctx.state.purchases) {
        if (!purchaseChat(ctx, purchase.id))
            continue;
        const status = purchase.state === 'proposed' ? 'Pending approval' : purchase.state === 'approved' ? 'Exact quote approved · not submitted' : purchase.state === 'denied' ? 'Declined' : purchase.state === 'unknown' ? 'Payment outcome unknown' : purchase.state === 'submitted' ? 'Submitted · awaiting outcome' : purchase.state === 'confirmed' ? 'Order confirmed' : 'Payment declined';
        records.push(h('details', { class: 'approval-record', 'data-domain-history': purchase.id }, h('summary', {}, h('span', { class: 'approval-icon' }, icon('cart', 17)), h('span', { class: 'approval-copy' }, h('strong', {}, `Purchase from ${purchase.merchant}`), h('span', { class: 'approval-description' }, `${currency(purchase.total)} · exact quote revision ${purchase.revision}`), h('span', { class: 'approval-meta' }, status)), icon('chevron', 12)), h('div', { class: 'approval-report' }, h('p', {}, 'Approval applies only to the reviewed quote, not future purchases. Approval is not proof of submission or payment.'), button('View purchase review', () => reviewPurchase(ctx, purchase), { variant: 'link' }))));
    }
    for (const task of ctx.state.browserTasks) {
        if (!browserChat(ctx, task.id) || (!challengePending(task) && !task.granted.length && task.challenge !== 'waiting'))
            continue;
        const status = challengePending(task) ? 'Pending approval' : task.state === 'cancelled' ? 'Stopped' : task.granted.length ? 'Allowed once' : task.controller === 'user' ? 'User has control' : 'No pending approval';
        records.push(h('details', { class: 'approval-record', 'data-domain-history': task.id }, h('summary', {}, h('span', { class: 'approval-icon' }, icon('globe', 17)), h('span', { class: 'approval-copy' }, h('strong', {}, `Site challenge · ${task.site}`), h('span', { class: 'approval-description' }, task.step), h('span', { class: 'approval-meta' }, status)), icon('chevron', 12)), h('div', { class: 'approval-report' }, h('p', {}, task.lastConfirmedState), button('View browser', () => ctx.openBrowser(task.id), { variant: 'link' }))));
    }
    return records;
}
