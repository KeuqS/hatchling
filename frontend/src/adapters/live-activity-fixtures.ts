import type { ActivityRow, ActivityThread, AppState, RunningActivityStep } from '../protocol/models.js';
import { normalizeRow } from '../protocol/rules.js';
const at = (ms: number): string => new Date(Date.parse('2026-09-25T01:38:00Z') + ms).toISOString();
const pending: RunningActivityStep = { id: 'bundle-write', workId: 'bundle-write', laneId: 'bundle-main', title: 'Writing the check script', subtitle: 'Writing the requested bundle verification script', startedAt: at(3000), report: 'Running `cd /demo/workspace && cat > /tmp/check_sample_bundle.py …` is in progress.\n\nThe command is shown as reference text only. No shell is executed by this frontend.' };
function row(id: string, title: string, subtitle: string, report: string, offset: number, failed = false): ActivityRow {
    return normalizeRow({ id, workId: id, laneId: 'bundle-main', title, subtitle, report, phase: 'action', icon: 'check', outputFailed: failed, status: failed ? 'failed' : 'success', startedAt: at(offset), endedAt: at(offset + 20), durationMs: 20 });
}
export function startBundle(state: AppState): void {
    state.turn = 'working';
    const thread: ActivityThread = { id: 'live-bundle', chatId: 'reference', name: 'Bundle sample files', subtitle: 'Writing the requested bundle verification script', statusTitle: 'Writing script', state: 'running', verdict: null, finishMessage: '', time: '1:38 am', startedAt: at(0), endedAt: null, evidenceArtifactId: null, pendingSteps: [{ ...pending }], lanes: [{ id: 'bundle-main', name: 'Main agent', parentId: null, rows: [
                    row('bundle-reaction', 'Reacted with 👍 emoji', 'Acknowledged the requested file bundle without expanding scope', 'Acknowledged the request. No external action was authorized by the reaction.', 200),
                    row('bundle-sha-failed', 'Sample source lookup failed', 'The first source lookup returned no result', 'The initial lookup failed. The failure remains in the evidence even when a later lookup succeeds.\n\n```text\nSample response: sample source unavailable\nNo repository was contacted\n```', 800, true),
                    row('bundle-sha', 'Retrieved the sample source identifier', 'Retrieved the reference identifier through the alternate lookup', 'The alternate lookup returned the scripted reference identifier. The earlier failed row is retained.\n\nNo live repository or private account was accessed.', 1800),
                ] }] };
    const childId = 'bundle-verifier';
    thread.lanes.push({ id: childId, parentId: 'bundle-main', name: 'Verify bundle manifest', rows: [normalizeRow({ ...row('verify-start', 'Started Verify bundle manifest', 'Started the delegated local file manifest review', 'Began the delegated sample verification. No result is claimed.', 2200), laneId: childId, phase: 'start', icon: 'run' })] });
    thread.lanes[0]?.rows.push(normalizeRow({ ...row('verify-dispatch', 'Launched Verify bundle manifest', 'Delegated the local file manifest review to a subagent', 'Launched a separate verifier lane. The main agent does not claim the child’s work.', 2100), phase: 'dispatch', icon: 'run' }));
    thread.pendingSteps?.push({ id: 'verify-scope', workId: 'verify-scope', laneId: childId, title: 'Reviewing the manifest entries', subtitle: 'Checking the listed local sample file entries', startedAt: at(2500), report: 'The child verifier is reading the local manifest entries. Work appears only in this lane.' });
    const second: ActivityThread = { id: 'live-collect', chatId: 'reference', name: 'Collect sample notes', subtitle: 'Querying the requested sample note summary for comparison', statusTitle: 'Reading notes', state: 'running', verdict: null, finishMessage: '', time: '1:36 am', startedAt: at(-120000), endedAt: null, evidenceArtifactId: null, pendingSteps: [{ id: 'collect-query', workId: 'collect-query', laneId: 'collect-main', title: 'Reading the sample note summary', subtitle: 'Reading the recorded sample note summary for comparison', startedAt: at(500), report: 'Reading the supplied sample-note fixture. No binary, process, or remote service is accessed.' }], lanes: [{ id: 'collect-main', name: 'Main agent', parentId: null, rows: [] }] };
    state.threads.unshift(thread, second);
}
export function advanceBundle(state: AppState, phase: number): void {
    const thread = state.threads.find(item => item.id === 'live-bundle');
    if (!thread || thread.state !== 'running')
        return;
    const lane = thread.lanes[0];
    if (!lane)
        return;
    if (phase === 8) {
        if (!lane.rows.some(item => item.id === pending.id))
            lane.rows.push(row(pending.id, 'Wrote the check script', 'Saved the local verification script as an attachment', 'Saved an illustrative local script. Nothing was executed. The file preview is available with the completed summary.', 3000));
        thread.pendingSteps = [{ id: 'bundle-check', workId: 'bundle-check', laneId: lane.id, title: 'Checking the bundle contents', subtitle: 'Reviewing the saved bundle and its manifest', startedAt: at(7000), report: 'Reviewing the local fixture manifest before completing the bundle.\n\n```text\ncheck_sample_bundle.py\nbundle-manifest.md\n```' }, { id: 'verify-current', workId: 'verify-current', laneId: 'bundle-verifier', title: 'Comparing the final manifest', subtitle: 'Comparing the last recorded sample manifest entries', startedAt: at(8000), report: 'The child verifier is comparing its final local manifest entries. No repository is contacted.' }];
        const child = thread.lanes.find(item => item.id === 'bundle-verifier');
        if (child && !child.rows.some(item => item.id === 'verify-scope')) {
            const labels = ['Read the manifest scope', 'Checked the script entry', 'Checked the document entry', 'Compared the file names', 'Reviewed the sample boundaries', 'Checked the stored byte lengths', 'Verified the preview references', 'Recorded the manifest comparison'];
            for (const [index, title] of labels.entries())
                child.rows.push(normalizeRow({ ...row(index === 0 ? 'verify-scope' : `verify-${index}`, title, 'Checked one recorded detail in the sample bundle', `## ${title}\n\nInspected the deterministic local manifest. This is illustrative evidence, not a live repository check.\n\n| Field | Result |\n| --- | --- |\n| Fixture entry | Present |\n| External execution | None |`, 3000 + index * 400), laneId: child.id, icon: 'run' }));
        }
        thread.subtitle = 'Reviewing the saved bundle and its manifest';
        thread.statusTitle = 'Checking bundle';
    }
    else if (phase === 9) {
        if (!lane.rows.some(item => item.id === pending.id))
            advanceBundle(state, 8);
        lane.rows.push(row('bundle-check', 'Checked the bundle contents', 'Verified the two illustrative local file previews', 'The two local previews were saved. This does not verify or recreate a real source repository.', 9000));
        const child = thread.lanes.find(item => item.id === 'bundle-verifier');
        if (child && !child.rows.some(item => item.id === 'verify-current'))
            child.rows.push(normalizeRow({ ...row('verify-current', 'Compared the final manifest', 'Compared the final recorded sample manifest entries', 'The local fixture comparison finished. No external verification is claimed.', 9600), laneId: child.id, icon: 'run' }));
        if (child && !child.rows.some(item => item.id === 'verify-finish'))
            child.rows.push(normalizeRow({ ...row('verify-finish', 'Completed Verify bundle manifest', 'Completed the delegated review of local fixture entries', 'The separate verifier finished its local manifest review. No external verification is claimed.', 10000), laneId: child.id, phase: 'finish', icon: 'run' }));
        const files = [
            { id: 'bundle-script', title: 'check_sample_bundle.py', format: 'code' as const, content: '# Local UI demonstration — not the synthetic verification example.\n# No network, shell, or repository access.\n\ndef bundle_names() -> tuple[str, ...]:\n    return ("check_sample_bundle.py", "bundle-manifest.md")\n' },
            { id: 'bundle-manifest', title: 'bundle-manifest.md', format: 'markdown' as const, content: '# Local bundle manifest\n\nThese two illustrative files demonstrate the activity-to-artifact workflow. They are not the original research files.\n\n- check_sample_bundle.py\n- bundle-manifest.md\n' },
        ];
        for (const file of files)
            if (!state.artifacts.some(item => item.id === file.id))
                state.artifacts.push({ ...file, kind: 'document', description: 'Illustrative evidence from the local active-work demonstration.', asset: null, created: at(10000), opened: at(10000), pinned: false, trashUntil: null, version: 1, publishedVersion: null, slides: [] });
        if (!state.artifacts.some(item => item.id === 'bundle-archive'))
            state.artifacts.push({ id: 'bundle-archive', title: 'Sample handoff tarball', kind: 'document', format: 'archive', description: 'Local illustrative archive — not the original external project bundle.', content: '# Contents\n\n- check_sample_bundle.py\n- bundle-manifest.md\n\nThese are original inert example files. No external commands were run.', asset: './media/hatchling-sample-handoff.tar.gz', created: at(10000), opened: at(10000), pinned: false, trashUntil: null, version: 1, publishedVersion: null, slides: [] });
        thread.pendingSteps = [];
        thread.state = 'completed';
        thread.verdict = 'success';
        thread.endedAt = at(11000);
        thread.statusTitle = 'Saved bundle';
        thread.subtitle = 'Saved the illustrative script and bundle manifest';
        thread.summary = 'Saved the two local preview files and checked the manifest. The first source lookup failed; the successful alternate lookup and saved outputs are recorded separately.';
        thread.finishMessage = 'The sample bundle is ready, with its verification notes. No external repository was changed.';
        thread.evidenceArtifactId = 'bundle-script';
        thread.evidenceArtifactIds = ['bundle-script', 'bundle-manifest'];
        const second = state.threads.find(item => item.id === 'live-collect');
        if (second) {
            const child = second.lanes[0];
            if (child && !child.rows.some(item => item.id === 'collect-query'))
                child.rows.push(normalizeRow({ id: 'collect-query', workId: 'collect-query', laneId: child.id, title: 'Recorded the sample note summary', subtitle: 'Saved the supplied sample note summary for comparison', report: 'Read and recorded the local reference fixture. No binary or remote service was accessed.', phase: 'action', icon: 'check', outputFailed: false, status: 'success', startedAt: at(500), endedAt: at(11000), durationMs: 10500 }));
            second.state = 'completed';
            second.pendingSteps = [];
            second.verdict = 'success';
            second.statusTitle = 'Recorded summary';
            second.subtitle = 'Recorded the supplied sample note summary for comparison';
            second.finishMessage = 'The local reference summary is ready.';
            second.endedAt = at(11000);
        }
        state.turn = 'idle';
        state.messages.push({ id: `bundle-result-${state.revision}`, chatId: 'reference', time: '1:38 am', source: null, reaction: null, role: 'agent', kind: 'attachment', artifactId: 'bundle-archive', text: 'The local sample bundle is ready, with both files included.' });
    }
}
