import type { ArtifactFormat, Channel, DeepReadonly, PaymentMethod } from '../protocol/models.js';
/** Display selectors only. Adapter/backend validation remains authoritative for every command. */
export const identityDocumentId = 'identity-profile';
export function currency(value: number): string { return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(value); }
export function maskCard(method: DeepReadonly<PaymentMethod>): string {
    return /^\d{4}$/.test(method.last4) ? `${method.brand} .... ${method.last4}` : `${method.brand} ....`;
}
export function canPublish(kind: string): boolean { return kind === 'document' || kind === 'page'; }
const formats: readonly ArtifactFormat[] = ['markdown', 'pdf', 'word', 'spreadsheet', 'csv', 'code', 'text', 'slides', 'html', 'app', 'image', 'video', 'audio', 'archive'];
export function isFormat(value: unknown): value is ArtifactFormat { return typeof value === 'string' && formats.some(format => format === value); }
export interface ChannelAction {
    label: string;
    action: 'begin' | 'pending' | 'check' | 'retry-later' | 'open' | 'view';
}
/** The id-specific deep-link capability is an existing domain-model limitation; see the handoff. */
export function channelAction(channel: DeepReadonly<Channel>): ChannelAction {
    switch (channel.state) {
        case 'unlinked': return { label: 'Connect securely', action: 'begin' };
        case 'link_pending': return { label: 'Continue connection', action: 'pending' };
        case 'checking': return { label: 'Re-check status', action: 'check' };
        case 'unavailable': return { label: 'Try again later', action: 'retry-later' };
        case 'linked': return channel.id === 'whatsapp' ? { label: 'Open chat', action: 'open' } : { label: 'View conversation', action: 'view' };
    }
}
