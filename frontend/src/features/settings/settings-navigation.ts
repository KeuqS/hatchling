/** Nested settings stay in the same modal; Back restores the previous pane and scroll. */
export interface SettingsNavigation {
    detail: string;
    id: string;
    open(detail: string, id?: string): void;
    back(): void;
    refresh(): void;
}
