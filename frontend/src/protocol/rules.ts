import type { ActivityRow, Channel, DeepReadonly, PaymentMethod, Purchase, Verdict } from './models.js';
export const BUBBLE_BREAK = '[[break]]';
export const SECURE_TOKEN = '[[secure-entry]]';
export function splitAgentBubbles(text: string): string[] {
    return text.split(BUBBLE_BREAK).map((part) => part.trim()).filter(Boolean);
}
export function isSecureEmbed(text: string): boolean {
    return text.split('\n').some((line) => line.trim() === SECURE_TOKEN);
}
export function actionStatus(outputFailed: boolean): Verdict { return outputFailed ? 'failed' : 'success'; }
export function normalizeRow(row: ActivityRow): ActivityRow {
    for (const field of [row.title, row.subtitle, row.icon, row.report, row.workId, row.laneId]) {
        if (!field.trim())
            throw new Error('An activity record is missing required evidence.');
    }
    return { ...row, status: actionStatus(row.outputFailed) };
}
export function deduplicateRows(rows: ActivityRow[]): ActivityRow[] {
    const unique = new Map<string, ActivityRow>();
    for (const row of rows)
        unique.set(`${row.laneId}:${row.workId}`, normalizeRow(row));
    return [...unique.values()];
}
export function finishVerdict(facts: {
    produced: boolean;
    launched: boolean;
    scheduled: boolean;
    saved: boolean;
    delivered: boolean;
    handoff: boolean;
}): Verdict {
    return Object.values(facts).some(Boolean) ? 'success' : 'failed';
}
export function maskCard(method: DeepReadonly<PaymentMethod>): string {
    if (!/^\d{4}$/.test(method.last4))
        throw new Error('Only the last four digits belong in a payment reference.');
    return `${method.brand} .... ${method.last4}`;
}
export function wordCount(text: string): number { return text.trim().split(/\s+/u).filter(Boolean).length; }
export function validThreadCopy(name: string, subtitle: string, statusTitle: string): boolean {
    return wordCount(name) <= 8 && wordCount(subtitle) >= 4 && wordCount(subtitle) <= 10 && wordCount(statusTitle) >= 1 && wordCount(statusTitle) <= 3 && subtitle.toLowerCase() !== name.toLowerCase();
}
const forbiddenBrowserWords = /\b(error|failed|failure|retry|retries|retrying|recover(?:y|ing)?|troubleshoot(?:ing)?)\b/iu;
export function sanitizeBrowserNarration(proposed: string, concreteAction: string): string {
    return forbiddenBrowserWords.test(proposed) ? concreteAction : proposed;
}
export function canApprove(purchase: DeepReadonly<Purchase>, revision: number, total: number, available: number): string | null {
    if (purchase.state !== 'proposed')
        return 'This review is no longer awaiting approval.';
    if (purchase.ordersSubmitted > 0)
        return 'An order has already been submitted. Take over to check the merchant.';
    if (revision !== purchase.revision || total !== purchase.total)
        return 'The quote changed. Please review the updated total before approving.';
    if (total > available)
        return `This purchase is $${total.toFixed(2)}; your remaining daily limit is $${available.toFixed(2)}. Nothing was submitted.`;
    return null;
}
export function canSubmit(purchase: DeepReadonly<Purchase>): string | null {
    if (purchase.state === 'unknown')
        return 'The payment outcome is unknown. Do not retry: you could pay twice. Take over to check.';
    if (purchase.ordersSubmitted > 0)
        return 'This browser task already submitted its one allowed order.';
    if (purchase.state !== 'approved')
        return 'Please approve the exact purchase review first.';
    if (purchase.approvedTotal !== purchase.total || purchase.approvedRevision !== purchase.revision)
        return 'Your approval does not cover the changed quote. Review it again.';
    return null;
}
export interface ChannelAction {
    label: string;
    action: 'begin' | 'pending' | 'check' | 'retry-later' | 'open' | 'view';
}
export function channelAction(channel: DeepReadonly<Channel>): ChannelAction {
    switch (channel.state) {
        case 'unlinked': return { label: 'Connect securely', action: 'begin' };
        case 'link_pending': return { label: 'Continue connection', action: 'pending' };
        case 'checking': return { label: 'Re-check status', action: 'check' };
        case 'unavailable': return { label: 'Try again later', action: 'retry-later' };
        case 'linked': return channel.id === 'whatsapp' ? { label: 'Open chat', action: 'open' } : { label: 'View conversation', action: 'view' };
    }
}
export function canPublish(kind: string): boolean { return kind === 'document' || kind === 'page'; }
export function editableFormat(format: string): boolean { return format === 'markdown' || format === 'slides'; }
export function currency(value: number): string { return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(value); }
export function safeFilename(title: string): string { return title.replace(/[^a-zA-Z0-9._ -]/g, '').replace(/\s+/g, '-').toLowerCase() || 'download'; }
export function containsSensitiveValue(text: string): boolean {
    return /\b(?:\d[ -]?){13,19}\b|\b(?:password|api[_ -]?key|access[_ -]?token|security[_ -]?code)\s*[:=]\s*\S+/iu.test(text);
}
export function assertNever(value: never): never { throw new Error(`Unhandled contract variant: ${String(value)}`); }
