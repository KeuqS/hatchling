import type { ActivityLane, ActivityRow, ActivityThread, DeepReadonly, IconName, Verdict } from '../../protocol/models.js';
export type ReadRow = DeepReadonly<ActivityRow>;
export type ReadThread = DeepReadonly<ActivityThread>;
export interface TimelineGroup {
    lane: DeepReadonly<ActivityLane>;
    rows: ReadRow[];
    ordinal: number;
}
/** Keep lane ownership, stable work identity, and chronological order. Missing times never become invented times. */
export function timelineGroups(thread: ReadThread): TimelineGroup[] {
    const seen = new Set<string>();
    const ordered: DeepReadonly<ActivityLane>[] = [];
    const visit = (lane: DeepReadonly<ActivityLane>): void => {
        if (seen.has(lane.id))
            return;
        seen.add(lane.id);
        ordered.push(lane);
        thread.lanes.filter((item) => item.parentId === lane.id).forEach(visit);
    };
    thread.lanes.filter((lane) => !lane.parentId).forEach(visit);
    thread.lanes.forEach(visit);
    let ordinal = 0;
    return ordered.map((lane) => {
        const unique = new Map<string, ReadRow>();
        for (const row of lane.rows)
            if (row.laneId === lane.id)
                unique.set(row.workId, row);
        const rows = [...unique.values()].toSorted((a, b) => a.startedAt && b.startedAt ? a.startedAt.localeCompare(b.startedAt) : 0);
        return { lane, rows, ordinal: lane.parentId ? ++ordinal : 0 };
    });
}
export function durationLabel(milliseconds: number | null | undefined): string {
    if (milliseconds === null || milliseconds === undefined || !Number.isFinite(milliseconds) || milliseconds < 0)
        return 'Not recorded';
    if (milliseconds < 1000)
        return `${milliseconds} ms`;
    const seconds = milliseconds / 1000;
    if (seconds < 60)
        return `${Number(seconds.toFixed(2))} s`;
    return `${Math.floor(seconds / 60)} min ${Number((seconds % 60).toFixed(1))} s`;
}
export function timeLabel(value: string | null | undefined): string {
    if (!value || !Number.isFinite(Date.parse(value)))
        return 'Time not recorded';
    return new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit', second: '2-digit', fractionalSecondDigits: 3, timeZone: 'UTC' }).format(new Date(value));
}
export function reportTitleAndBody(row: ReadRow): {
    title: string;
    body: string;
} {
    const lines = row.report.split('\n');
    // Avoid repeating the selected step as a second oversized heading.
    if ((lines[0] ?? '').replace(/^#{1,6} /u, '').trim().toLowerCase() === row.title.trim().toLowerCase())
        lines.shift();
    return { title: row.title, body: lines.join('\n').trim() };
}
export interface StepOutcome {
    readonly mark: Verdict;
    readonly icon: IconName;
    readonly reportIcon: IconName;
    readonly sentence: string | null;
}
/** How each step status reads . A stopped step is not a failed one, and a partial result is not a success. */
const outcomes: Readonly<Record<Verdict, StepOutcome>> = {
    success: { mark: 'success', icon: 'check', reportIcon: 'check', sentence: null },
    failed: { mark: 'failed', icon: 'close', reportIcon: 'warning', sentence: 'This step failed. The overall task has a separate verdict.' },
    interrupted: { mark: 'interrupted', icon: 'stop', reportIcon: 'stop', sentence: 'This step was stopped before it finished. Being stopped is not a failure; the overall task has a separate verdict.' },
    partial: { mark: 'partial', icon: 'warning', reportIcon: 'warning', sentence: 'This step finished only part of its work. The overall task has a separate verdict.' },
};
export function stepOutcome(status: Verdict): StepOutcome {
    return outcomes[status];
}
