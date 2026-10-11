export type ContentResource = 'ideas' | 'feed' | 'library';
export type ContentPreview = 'loading' | 'error' | 'refresh' | 'refresh-error';
export interface ContentLoadingState {
    readonly phase: 'ready' | 'loading' | 'error';
    readonly cached: boolean;
    readonly demo: boolean;
    readonly visible: boolean;
    readonly slow: boolean;
    readonly message: string;
}
interface PendingView {
    token: number;
    state: ContentLoadingState;
    timers: ReturnType<typeof setTimeout>[];
}
const ready: ContentLoadingState = Object.freeze({ phase: 'ready', cached: false, demo: false, visible: false, slow: false, message: '' });
/** View-only pending state; never a substitute for authoritative adapter data.
 * Normal navigation has no artificial delay. Only explicit preview scenarios use timers to settle.
 */
export class ContentLoading {
    private readonly entries = new Map<ContentResource, PendingView>();
    private sequence = 0;
    private disposed = false;
    constructor(private readonly changed: () => void) { }
    read(resource: ContentResource): ContentLoadingState { return this.entries.get(resource)?.state ?? ready; }
    begin(resource: ContentResource, cached: boolean): number | null {
        if (this.disposed || (this.read(resource).phase === 'loading' && !this.read(resource).demo))
            return null;
        return this.start(resource, cached, false);
    }
    private start(resource: ContentResource, cached: boolean, demo: boolean): number {
        this.clear(resource);
        const token = ++this.sequence;
        const entry: PendingView = { token, state: { phase: 'loading', cached, demo, visible: demo, slow: false, message: '' }, timers: [] };
        this.entries.set(resource, entry);
        if (!demo)
            entry.timers.push(setTimeout(() => this.update(resource, token, { ...entry.state, visible: true }), 120));
        entry.timers.push(setTimeout(() => this.update(resource, token, { ...entry.state, visible: true, slow: true }), 10000));
        this.changed();
        return token;
    }
    finish(resource: ContentResource, token: number, ok: boolean, message = ''): void {
        const entry = this.entries.get(resource);
        if (this.disposed || entry?.token !== token)
            return;
        if (ok)
            this.clear(resource);
        else {
            entry.timers.forEach(clearTimeout);
            entry.timers = [];
            entry.state = { ...entry.state, phase: 'error', visible: true, slow: false, message };
        }
        this.changed();
    }
    preview(resource: ContentResource, mode: ContentPreview): boolean {
        if (this.disposed || (this.read(resource).phase === 'loading' && !this.read(resource).demo))
            return false;
        const cached = mode === 'refresh' || mode === 'refresh-error';
        const token = this.start(resource, cached, true);
        const entry = this.entries.get(resource);
        if (mode === 'error' || mode === 'refresh-error')
            this.finish(resource, token, false, 'Sample loading failure. No files or content were removed.');
        else
            entry?.timers.push(setTimeout(() => this.finish(resource, token, true), 6000));
        return true;
    }
    endPreview(resource: ContentResource): void {
        if (!this.read(resource).demo)
            return;
        this.clear(resource);
        this.changed();
    }
    private update(resource: ContentResource, token: number, state: ContentLoadingState): void {
        const entry = this.entries.get(resource);
        if (this.disposed || entry?.token !== token)
            return;
        entry.state = state;
        this.changed();
    }
    private clear(resource: ContentResource): void {
        this.entries.get(resource)?.timers.forEach(clearTimeout);
        this.entries.delete(resource);
    }
    dispose(): void {
        this.disposed = true;
        for (const resource of this.entries.keys())
            this.clear(resource);
    }
}
