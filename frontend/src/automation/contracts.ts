import type { Command } from '../protocol/models.js';
export type Exposure = 'review' | 'human-only' | 'internal';
export interface InputField {
    readonly type: 'string' | 'number' | 'boolean';
    readonly enum?: readonly (string | number | boolean)[];
}
export interface ActionSchema {
    readonly type: 'object';
    readonly properties: Readonly<Record<string, InputField>>;
    readonly required: readonly string[];
    readonly additionalProperties: false;
}
export interface ActionDescription {
    readonly name: string;
    readonly title: string;
    readonly policy: Exposure | 'view';
    readonly inputSchema: ActionSchema;
}
export type ViewAction = {
    type: 'ui.navigate';
    route: string;
    detail?: string;
} | {
    type: 'ui.settings';
    section?: string;
} | {
    type: 'ui.context';
    tab: 'activity' | 'approvals' | 'schedule' | 'identity';
    open: boolean;
} | {
    type: 'ui.side-chats';
    open: boolean;
} | {
    type: 'ui.prefill';
    text: string;
} | {
    type: 'ui.artifact' | 'ui.document' | 'ui.activity' | 'ui.browser';
    id: string;
} | {
    type: 'ui.close-workspace' | 'ui.search';
};
export type AgentAction = Command | ViewAction;
export type AgentResult = {
    status: 'done' | 'pending' | 'rejected' | 'cancelled' | 'stale' | 'blocked';
    id: string;
    message: string;
    revision: number;
};
export interface AgentEnvelope {
    key: string;
    action: unknown;
    expectedRevision?: number;
}
export interface AgentPort {
    readonly product: 'hatchling-frontend';
    readonly version: '1.0';
    status(): {
        enabled: boolean;
        product: string;
        version: string;
        pending: string | null;
    };
    list(): readonly ActionDescription[];
    inspect(): unknown;
    read(resource: string, id: string): unknown;
    execute(input: unknown): Promise<AgentResult>;
    result(id: string): AgentResult | null;
}
