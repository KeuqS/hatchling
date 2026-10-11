import type { ActivityRow, ActivityThread, AppState, Artifact } from '../protocol/models.js';
import { normalizeRow } from '../protocol/rules.js';
const start = Date.parse('2026-09-25T09:41:00.000Z');
const timeAt = (offset: number): string => new Date(start + offset).toISOString();
function step(id: string, title: string, subtitle: string, report: string, offset: number, duration: number, laneId = 'dynamic-g1', phase: ActivityRow['phase'] = 'action', artifactIds: string[] = []): ActivityRow {
    return normalizeRow({ id, workId: id, laneId, title, subtitle, icon: artifactIds.length ? 'file' : phase === 'action' ? 'check' : 'run', report, startedAt: timeAt(offset), endedAt: timeAt(offset + duration), durationMs: duration, artifactIds, phase, outputFailed: false, status: 'success' });
}
const loader = '#!/usr/bin/env python3\n"""Read-only illustration for the local activity viewer.\n\nThis is a frontend fixture, not a live document loader.\nIt performs no process, filesystem, network or account access.\n"""\n\nfrom dataclasses import dataclass\n\n@dataclass(frozen=True)\nclass SampleEntry:\n    tag: int\n    phase: str\n\ndef sample_entries() -> tuple[SampleEntry, ...]:\n    """Return inert sample records for the generated preview."""\n    return tuple(SampleEntry(tag, "sample") for tag in range(64))\n\nif __name__ == "__main__":\n    print(f"{len(sample_entries())} sample records")\n';
function file(id: string, title: string, format: Artifact['format'], content: string): Artifact {
    return { id, title, kind: 'document', format, description: 'Scripted evidence attachment. Not a live task output.', content, asset: null, created: timeAt(80000), opened: timeAt(80000), pinned: false, trashUntil: null, version: 1, publishedVersion: null, slides: [] };
}
export function dynamicActivity(): ActivityThread {
    return {
        id: 'dynamic04', chatId: 'reference', name: 'Review sample document collection', state: 'completed', verdict: 'success', statusTitle: 'Completed review', subtitle: 'Saved the example review and document artifacts', time: '9:42 am', startedAt: timeAt(0), endedAt: timeAt(88000),
        summary: 'Reviewed 64 synthetic document entries, grouped them under two example headings, and saved the local review notes and file previews. This is a scripted walkthrough, not a live run.',
        finishMessage: 'The sample review and its notes are ready. No real project was inspected.', evidenceArtifactId: 'dynamic-loader', evidenceArtifactIds: ['dynamic-loader', 'dynamic-notes', 'dynamic-sites'],
        lanes: [
            { id: 'main', name: 'Main agent', parentId: null, rows: [
                    step('dynamic-reaction', 'Reacted to user message with 👍', 'Acknowledged the requested continuation without expanding scope', 'Acknowledged the request to continue the existing task. No new network or execution permission was requested or inferred.', 500, 28, 'main'),
                    step('dynamic-dispatch', 'Launched Document Structure Review Subagent', 'Delegated the document review to a subagent', '## Delegation\n\nLaunched **Document Structure Review** with the existing read-only context and the bounded review task.\n\nThe child’s work is recorded in its own lane below. The main agent did not perform these child steps.\n\n### Scope\n\n- Reuse the supplied snapshot.\n- Keep unresolved observations explicit.\n- Return a saved result and the verification evidence.', 800, 140, 'main', 'dispatch'),
                ] },
            { id: 'dynamic-g1', name: 'Document Structure Review', parentId: 'main', rows: [
                    step('dynamic-start', 'Started Document Structure Review', 'Began the delegated review with the existing scope', 'Started the delegated task with the saved context. This is a start record, not a claim that any review was complete.', 1100, 20, 'dynamic-g1', 'start'),
                    step('dynamic-batch', 'Read sample-review.md', 'Loaded the previous findings and unresolved questions', 'Read the saved batch note and retained its explicit uncertainty.\n\n```text\nRead-only source: sample-review.md\nNo file modifications\n```', 1500, 18),
                    step('dynamic-previous', 'Read sample-collection/review.md', 'Loaded the prior document review scope and stopping point', 'Read the previous sample note. The next pass stays within the existing scope.\n\n> Unverified observations remain marked as unverified; a saved note does not convert them into evidence.', 2100, 22),
                    step('dynamic-inspection', 'Inspected sample collection contents', 'Completed a read-only example directory inspection', 'Read the synthetic collection manifest. The fixture records a successful inspection lasting 6 ms. No command ran and no host directory was accessed.\n\nThe example lists `README.md`, `loader.py`, `sample-entries.json`, and `verification-notes.md`.\n\n```text\nSample directory: /demo/collection/\nMode: read-only fixture\nExternal execution: none\n```\n\nNo files were modified by this inspection.', 2600, 6),
                    step('dynamic-read-loader', 'Read sample-collection/loader.py', 'Inspected the loader without executing it', 'Read the sample loader as text. No script was executed in this frontend.\n\n```python\n# Read-only preview of the saved loader\n# Execution stays outside the frontend adapter boundary.\n```\n\nThe supplied file preview is available in the completed summary.', 3300, 14),
                    step('dynamic-list-tools', 'Listed sample review tools', 'Located the supplied analysis notes and index', 'Listed the referenced tool directory to locate its README and index.\n\n```text\nREADME.md\nindex.json\nnotes.md\n```\n\nThis listing is synthetic; the app never reads the host filesystem.', 4100, 9),
                    step('dynamic-tool-readme', 'Inspected sample tools README.md', 'Checked the documented inputs and output scope', 'Read the tool documentation before interpreting the saved index. The report keeps commands and paths here, outside the live profile subtitle.', 5000, 11),
                    step('dynamic-verify-index', 'Checked sample index and version', 'Matched the reference build identifier before reviewing', '## Snapshot check\n\nThe synthetic summary uses version `sample-v1`. The scripted trace keeps that identifier with the review; it does not claim to have hashed an actual project.\n\n| Check | Recorded result |\n| --- | --- |\n| Reference build | `sample-v1` |\n| Scope | Saved snapshot |\n| Writes during inspection | None |', 6100, 31),
                    step('dynamic-map', 'Reviewed the sample document sections', 'Recorded the sample entries and two headings', 'Recorded **64 synthetic document entries** and **two example headings** in this fictional review. This is illustrative data, not a claim about a real project.\n\n### Verification notes\n\n- Keep documented observations separate from hypotheses.\n- Preserve the agreed sample scope.\n- Stop when the bounded output has been saved.', 9000, 62400),
                    step('dynamic-files', 'Saved the review and sample artifacts', 'Saved three local previews with the completed report', 'Saved the local evidence previews for this sample trace.\n\n- [Loader preview](artifact:dynamic-loader)\n- [Verification notes](artifact:dynamic-notes)\n- [Sample entries](artifact:dynamic-sites)\n\nNo files were written to a user repository. These attachments live in the scripted adapter.', 76000, 84, 'dynamic-g1', 'action', ['dynamic-loader', 'dynamic-notes', 'dynamic-sites']),
                    step('dynamic-finish', 'Completed Document Structure Review', 'Returned the review and saved evidence to the parent', 'Returned the saved artifacts and the uncertainty notes to the main agent. The bounded task is complete; no additional run is implied.', 86500, 20, 'dynamic-g1', 'finish'),
                ] },
        ],
    };
}
export function applyActivityFixtures(state: AppState): AppState {
    state.threads.unshift(dynamicActivity());
    state.artifacts.push(file('dynamic-loader', 'loader.py', 'code', loader), file('dynamic-notes', 'verification-notes.md', 'markdown', '# Sample document review\n\nSynthetic evidence created for this local walkthrough, not a live run.\n\n## Recorded outcome\n\nThe example contains 64 entries and two headings.\n\n## Limits\n\nThe frontend does not execute the sample loader. Timestamps in this trace are deterministic sample values.\n\n## Files\n\nThe loader and JSON preview illustrate the file-viewer workflow.\n'), file('dynamic-sites', 'sample-entries.json', 'code', JSON.stringify({ source: 'scripted frontend fixture', sampleVersion: 'sample-v1', sampleCount: 64, sampleGroups: 2, independentlyVerified: false }, null, 2)));
    return state;
}
/** A short stream for exercising real status transitions and side-chat completion. No CI is contacted. */
export function startCi(state: AppState): void {
    state.threads = state.threads.filter((thread) => thread.id !== 'live-ci');
    state.turn = 'working';
    state.threads.unshift({ id: 'live-ci', chatId: 'chat-0', name: 'Fix the frontend CI checks', statusTitle: 'Fixing CI', subtitle: 'Checking the failing type and lint checks', state: 'running', verdict: null, finishMessage: '', time: '9:41 am', startedAt: timeAt(0), endedAt: null, evidenceArtifactId: null, lanes: [{ id: 'main', name: 'Main agent', parentId: null, rows: [step('ci-inspect', 'Inspected the failing CI report', 'Identified the type-check and lint checks', 'Read the deterministic CI fixture. No repository or remote workflow was accessed.', 0, 48, 'main')] }] });
}
export function advanceCi(state: AppState, stage: number): void {
    const thread = state.threads.find((item) => item.id === 'live-ci');
    const lane = thread?.lanes[0];
    if (!thread || !lane || thread.state !== 'running')
        return;
    if (stage === 6 && !lane.rows.some((item) => item.id === 'ci-verify')) {
        lane.rows.push(step('ci-verify', 'Verified the scripted correction', 'Checked the corrected fixture against the sample rules', '```diff\n- // unchecked sample result\n+ // handle the missing result explicitly\n```\n\nThis is an illustrative diff, not a change to a real repository.', 1600, 92, 'main'));
        thread.statusTitle = 'Checking tests';
        thread.subtitle = 'Verifying the corrected type and lint results';
    }
    if (stage === 7) {
        thread.state = 'completed';
        thread.verdict = 'success';
        thread.statusTitle = 'Completed checks';
        thread.subtitle = 'Saved the sample verification and completion notes';
        thread.endedAt = timeAt(4200);
        thread.finishMessage = 'The sample checks are complete and the verification notes are saved. No remote workflow was run.';
        thread.summary = thread.finishMessage;
        state.turn = 'idle';
        state.messages.push({ id: `ci-result-${state.revision}`, chatId: thread.chatId ?? 'main', time: '9:41 am', source: null, reaction: null, role: 'agent', kind: 'text', text: thread.finishMessage });
    }
}
