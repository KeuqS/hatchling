import type { AppState, CommandResult, DeepReadonly } from '../protocol/models.js';
import type { ChatRecovery, ChatSendCommand, ChatSessionPort, ChatSessionSnapshot, ConnectionState, DeliveryOutcome, HistoryState, ReliabilityScenario, SendRequest } from '../session/chat-session.js';
import type { StoragePort } from './scripted-adapter.js';
import { parseRecovery } from './chat-recovery.js';
const recoveryKey = 'hatchling.chat-recovery.v1';
const ready: ConnectionState = { kind: 'ready', detail: '' };
/** Deterministic fault injection around local fixtures. No VM, network, or service is contacted. */
export class ScriptedChatSession implements ChatSessionPort {
    private connection: ConnectionState = ready;
    private histories: Record<string, HistoryState> = {};
    private readonly listeners = new Set<() => void>();
    private readonly timers = new Map<ReturnType<typeof setTimeout>, () => void>();
    private readonly receipts = new Map<string, DeliveryOutcome>();
    private readonly requests = new Map<string, Promise<DeliveryOutcome>>();
    private readonly uncertain = new Map<string, ChatSendCommand>();
    private next: 'normal' | 'slow' | 'failed' | 'unknown' | 'attachment-failed' = 'normal';
    private disposed = false;
    private revision = 0;
    constructor(private readonly execute: (command: ChatSendCommand) => Promise<CommandResult>, private readonly state: () => DeepReadonly<AppState>, private readonly storage: StoragePort | null = null) { }
    getSnapshot(): ChatSessionSnapshot { return { connection: this.connection, histories: { ...this.histories } }; }
    subscribe(listener: () => void): () => void { this.listeners.add(listener); return () => this.listeners.delete(listener); }
    loadRecovery(): ChatRecovery | null {
        try {
            return parseRecovery(this.storage?.getItem(recoveryKey) ?? null);
        }
        catch {
            return null;
        }
    }
    saveRecovery(recovery: ChatRecovery): boolean {
        try {
            if (!this.storage)
                return false;
            const encoded = JSON.stringify(recovery);
            // Never advertise a saved snapshot that the recovery reader would reject.
            if (encoded.length > 1000000 || recovery.drafts.length > 200 || recovery.outbox.length > 30) {
                this.storage.removeItem(recoveryKey);
                return false;
            }
            this.storage.setItem(recoveryKey, encoded);
            return true;
        }
        catch {
            return false;
        }
    }
    send(request: SendRequest): Promise<DeliveryOutcome> {
        const receipt = this.receipts.get(request.id);
        if (receipt?.state === 'accepted')
            return Promise.resolve(receipt);
        const existing = this.requests.get(request.id);
        if (existing)
            return existing;
        const promise = this.perform(request);
        this.requests.set(request.id, promise);
        void promise.then(() => this.requests.delete(request.id), () => this.requests.delete(request.id));
        return promise;
    }
    private async perform(request: SendRequest): Promise<DeliveryOutcome> {
        if (this.connection.kind !== 'ready')
            return { state: 'rejected', detail: 'The local demo did not accept this message. Reconnect, then retry.' };
        const mode = this.next;
        if (mode !== 'attachment-failed' || request.command.type === 'chat.attach')
            this.next = 'normal';
        await this.wait(mode === 'slow' ? 7000 : mode === 'normal' ? 180 : 1100);
        if (this.disposed)
            return { state: 'unknown', detail: 'The page closed before delivery was confirmed.' };
        if (mode === 'failed' || (mode === 'attachment-failed' && request.command.type === 'chat.attach'))
            return { state: 'rejected', detail: request.command.type === 'chat.attach' ? 'This file was not accepted. Your other messages were not resent.' : 'The service did not accept this message. It is safe to retry.' };
        if (mode === 'unknown') {
            // Simulate a lost receipt. The eventual acceptance becomes visible only on reconciliation.
            this.uncertain.set(request.id, request.command);
            return { state: 'unknown', detail: 'The delivery receipt did not arrive. Check status before sending again.' };
        }
        return this.accept(request);
    }
    private async accept(request: SendRequest): Promise<DeliveryOutcome> {
        const previous = new Set(this.state().messages.map(message => message.id));
        const result = await this.execute(request.command);
        if (!result.ok)
            return { state: 'rejected', detail: result.message };
        const message = this.state().messages.find(item => item.chatId === request.command.chatId && item.role === 'user' && !previous.has(item.id));
        const receipt: DeliveryOutcome = { state: 'accepted', messageId: message?.id ?? null };
        this.receipts.set(request.id, receipt);
        return receipt;
    }
    async checkDelivery(requestId: string): Promise<DeliveryOutcome> {
        const existing = this.requests.get(requestId);
        if (existing)
            return existing;
        await this.wait(650);
        if (this.disposed || this.connection.kind !== 'ready')
            return { state: 'unknown', detail: 'Still unable to check delivery. This message will not be resent automatically.' };
        const command = this.uncertain.get(requestId);
        if (command) {
            this.uncertain.delete(requestId);
            return this.accept({ id: requestId, command });
        }
        return this.receipts.get(requestId) ?? { state: 'unknown', detail: 'No authoritative receipt is available for this request. Verify the conversation before resending.' };
    }
    async retryHistory(chatId: string): Promise<void> {
        const revision = this.revision;
        const cached = this.histories[chatId]?.hasCache ?? true;
        this.histories = { ...this.histories, [chatId]: { phase: 'loading', hasCache: cached, detail: '' } };
        this.notify();
        await this.wait(1000);
        if (this.disposed || revision !== this.revision)
            return;
        this.histories = { ...this.histories, [chatId]: this.connection.kind === 'ready' ? { phase: 'ready', hasCache: true, detail: '' } : { phase: 'error', hasCache: cached, detail: 'Reconnect before trying to load the conversation again.' } };
        this.notify();
    }
    async reconnect(): Promise<void> {
        if (this.connection.kind === 'reconnecting')
            return;
        const revision = this.revision;
        this.connection = { kind: 'reconnecting', detail: 'Checking the local demo connection.' };
        this.notify();
        await this.wait(1000);
        if (this.disposed || revision !== this.revision)
            return;
        this.connection = ready;
        this.notify();
    }
    preview(scenario: ReliabilityScenario, chatId: string): void {
        const revision = ++this.revision;
        this.next = 'normal';
        this.connection = ready;
        this.histories = {};
        if (scenario.startsWith('history-')) {
            this.histories[chatId] = { phase: scenario === 'history-loading' ? 'loading' : 'error', hasCache: scenario === 'history-stale', detail: 'The latest conversation could not be loaded.' };
            if (scenario === 'history-loading')
                void this.wait(8000).then(() => {
                    if (!this.disposed && revision === this.revision) {
                        this.histories = { ...this.histories, [chatId]: { phase: 'ready', hasCache: true, detail: '' } };
                        this.notify();
                    }
                });
        }
        if (scenario === 'slow-send')
            this.next = 'slow';
        if (scenario === 'send-failed')
            this.next = 'failed';
        if (scenario === 'send-unknown')
            this.next = 'unknown';
        if (scenario === 'attachment-failed')
            this.next = 'attachment-failed';
        if (scenario === 'offline')
            this.connection = { kind: 'offline', detail: 'The demo connection is offline. Nothing leaves this tab until it reconnects.' };
        if (scenario === 'service-unavailable')
            this.connection = { kind: 'unavailable', detail: 'The workspace service is unavailable. Your visible chat remains readable.' };
        if (scenario === 'session-expired')
            this.connection = { kind: 'sign-in-required', detail: 'Your session needs attention. Your draft is kept while you reconnect.' };
        if (scenario === 'vm-restarting') {
            this.connection = { kind: 'restarting', detail: 'The workspace is restarting. Queued messages will send when it is ready.' };
            void this.wait(10000).then(() => {
                if (!this.disposed && revision === this.revision) {
                    this.connection = ready;
                    this.notify();
                }
            });
        }
        this.notify();
    }
    private wait(delay: number): Promise<void> {
        return new Promise(resolve => {
            const timer = setTimeout(() => { this.timers.delete(timer); resolve(); }, delay);
            this.timers.set(timer, resolve);
        });
    }
    private notify(): void {
        for (const listener of this.listeners)
            listener();
    }
    dispose(): void {
        this.disposed = true;
        for (const [timer, resolve] of this.timers) {
            clearTimeout(timer);
            resolve();
        }
        this.timers.clear();
        this.listeners.clear();
    }
}
