import type { ActivityRow, AppState, BrowserTask, Command, CommandResult } from '../protocol/models.js';
import { containsSensitiveValue, normalizeRow } from '../protocol/rules.js';
import { fail, ok, say } from './reducers/shared.js';
const timestamp = (seconds: number): string => new Date(Date.parse('2026-09-25T02:51:00Z') + seconds * 1000).toISOString();
const completed = (id: string, title: string, subtitle: string, report: string, seconds: number): ActivityRow => normalizeRow({ id, workId: id, laneId: 'watch-main', title, subtitle, report, phase: 'action', icon: 'globe', outputFailed: false, status: 'success', startedAt: timestamp(seconds), endedAt: timestamp(seconds + 1), durationMs: 1000 });
/** Deliberately local: these are recorded/synthetic frames, not remote browser control. */
export function startBrowserWatch(state: AppState, chatId = 'main', query = 'reading nooks'): void {
    const id = `browser-watch-${state.revision}`;
    const busy = state.browserTasks.some(task => ['running', 'parked'].includes(task.state));
    const task: BrowserTask = { id, chatId, title: 'Search the web', site: 'search.example', step: 'Opening the requested search page in browser', statusTitle: 'Opening search', state: busy ? 'queued' : 'running', controller: 'agent', attemptsRemaining: 1, constraints: ['Read-only search', 'Pause for every unapproved challenge', 'No automatic retry', 'Frames and takeover controls are local demonstrations'], granted: [], credentialReference: null, challenge: 'none', continuedFrom: null, blocker: '', lastConfirmedState: 'Local browser opened. No external site contacted.', frame: { sequence: 0, page: 'home', title: 'Sample search', query, capturedAt: timestamp(0), image: null } };
    state.browserTasks.push(task);
    if (!busy)
        state.turn = 'working';
    state.messages.push({ id: `browser-card-${state.revision}`, kind: 'browser', role: 'agent', chatId, time: '2:51 am', source: null, reaction: null, taskId: id });
    say(state, busy ? 'The browser is busy with another task. This search is queued.' : 'Opening the browser preview. You can watch the sample search here, or take control at any point.', chatId);
    state.threads.unshift({ id: `watch-${id}`, chatId, name: 'Search the web', subtitle: task.step, statusTitle: 'Opening search', state: busy ? 'interrupted' : 'running', verdict: null, finishMessage: '', time: '2:51 am', startedAt: timestamp(0), pendingSteps: busy ? [] : [{ id: 'watch-open', workId: 'watch-open', laneId: 'watch-main', title: 'Opening search', subtitle: task.step, report: 'Opening a local search-page fixture. This frontend does not connect to any search service or launch a browser process.', startedAt: timestamp(0) }], lanes: [{ id: 'watch-main', parentId: null, name: 'Main agent', rows: [] }], evidenceArtifactId: null });
}
function updateThread(state: AppState, task: BrowserTask): void {
    const thread = state.threads.find(item => item.id === `watch-${task.id}`);
    if (!thread)
        return;
    thread.subtitle = task.step;
    thread.statusTitle = task.statusTitle;
    if (task.state === 'cancelled' || task.state === 'completed') {
        thread.pendingSteps = [];
        thread.state = task.state === 'completed' ? 'completed' : 'interrupted';
        thread.endedAt = task.frame?.capturedAt ?? null;
        thread.verdict = 'success'; // A retained snapshot and honest handoff exist; stop is not a failed result.
        thread.finishMessage = task.state === 'completed' ? 'The local sample result is ready. No external search ran.' : 'The browser stopped. The last recorded preview is still available.';
        thread.summary = thread.finishMessage;
    }
    else if (task.state === 'parked' || task.controller === 'user') {
        thread.state = 'interrupted';
    }
    else {
        thread.state = 'running';
    }
}
function showResults(state: AppState, task: BrowserTask): void {
    if (!task.frame)
        return;
    task.frame = { ...task.frame, sequence: 3, page: 'results', title: 'Sample search results', capturedAt: timestamp(9), image: null };
    task.state = 'completed';
    task.step = 'Displayed the local example search result page';
    task.statusTitle = 'Displayed results';
    task.blocker = '';
    task.lastConfirmedState = 'Local results fixture rendered. Not a verified web search.';
    state.turn = 'idle';
    say(state, 'The sample browser flow is complete. These are local example results, not a live web search.', task.chatId ?? 'main');
}
export function reduceBrowserSession(state: AppState, command: Command): CommandResult | null {
    if (command.type === 'turn.interrupt') {
        for (const task of state.browserTasks)
            if (task.frame && task.state === 'running') {
                task.controller = 'user';
                task.state = 'parked';
                updateThread(state, task);
            }
        return null;
    }
    if (command.type === 'chat.send' && /(?:open|use|show|watch).*(?:browser)|(?:browser).*(?:google|search)/i.test(command.text)) {
        const chat = state.chats.find(item => item.id === command.chatId);
        if (!chat || chat.channelId || chat.archived)
            return fail('Open an active product conversation first.');
        if (containsSensitiveValue(command.text))
            return fail('Do not include credentials or payment details in a search.');
        state.messages.push({ id: `watch-user-${state.revision}`, kind: 'text', role: 'user', chatId: chat.id, text: command.text, time: '2:51 am', source: null, reaction: '👍' });
        startBrowserWatch(state, chat.id);
        return ok('');
    }
    if (!['browser.advance', 'browser.input', 'browser.cancel', 'browser.takeover', 'browser.resume', 'browser.solve', 'browser.grant', 'browser.start'].includes(command.type) || !('id' in command))
        return null;
    const task = state.browserTasks.find(item => item.id === command.id);
    if (!task?.frame)
        return null;
    const frame = task.frame;
    const terminal = ['cancelled', 'completed', 'failed'].includes(task.state);
    if (command.type === 'browser.advance') {
        if (terminal || task.state !== 'running' || task.controller !== 'agent' || command.sequence !== frame.sequence + 1)
            return ok('');
        const thread = state.threads.find(item => item.id === `watch-${task.id}`);
        const lane = thread?.lanes[0];
        if (command.sequence === 1) {
            task.frame = { ...frame, sequence: 1, page: 'search', capturedAt: timestamp(3) };
            task.step = 'Entering the requested phrase in the search';
            task.statusTitle = 'Entering search';
            lane?.rows.push(completed('watch-open', 'Opened the search page', 'Displayed the local search-page preview without external access', 'The local search fixture is visible. No external request was made.', 0));
            if (thread)
                thread.pendingSteps = [{ id: 'watch-search', workId: 'watch-search', laneId: 'watch-main', title: 'Entering the search', subtitle: task.step, report: 'The local browser is showing the requested search text. No real search is submitted.', startedAt: timestamp(3) }];
        }
        else if (command.sequence === 2) {
            task.frame = { ...frame, sequence: 2, page: 'challenge', capturedAt: timestamp(6), image: './media/browser-sample-challenge.png' };
            task.state = 'parked';
            task.challenge = 'waiting';
            task.statusTitle = 'Opening results';
            task.step = 'Opening the results for the requested search';
            task.blocker = 'A site challenge blocks the results. Grant permission for this challenge or complete it in takeover.';
            task.lastConfirmedState = 'Synthetic local challenge screen. No real site or account is represented.';
            state.turn = 'awaiting';
            lane?.rows.push(completed('watch-search', 'Entered the search', 'The local preview reached the sample challenge page', 'Reached the synthetic site-challenge screen. Permission has not been granted and no checkbox was clicked. No automatic retries.', 3));
            if (thread)
                thread.pendingSteps = [{ id: 'watch-challenge', workId: 'watch-challenge', laneId: 'watch-main', title: 'Opening search results', subtitle: task.step, report: 'The visible site challenge is waiting for your decision. A takeover or a one-time grant is required; no result has been observed.', startedAt: timestamp(6) }];
            say(state, 'The preview reached a robot check before the search results. You can take over and complete the sample check yourself, or allow this one challenge from the browser controls. Nothing continues automatically.', task.chatId ?? 'main');
        }
        else if (command.sequence === 3 && task.challenge === 'solved')
            showResults(state, task);
        updateThread(state, task);
        return ok('');
    }
    if (terminal)
        return fail('This browser session has ended. The last preview is read-only.');
    if (command.type === 'browser.cancel') {
        task.state = 'cancelled';
        task.statusTitle = 'Stopped browsing';
        task.controller = 'user';
        state.turn = 'idle';
        updateThread(state, task);
        return ok('Browser stopped');
    }
    if (['queued', 'assigned'].includes(task.state) && command.type !== 'browser.start')
        return fail('This task must acquire the browser slot first.');
    if (command.type === 'browser.takeover') {
        task.controller = 'user';
        state.turn = 'awaiting';
        updateThread(state, task);
        return ok('');
    }
    if (command.type === 'browser.input') {
        if (task.controller !== 'user')
            return fail('Take control before interacting with the sample page.');
        if (containsSensitiveValue(command.text) || command.text.length > 240)
            return fail('Use a short sample search without private information.');
        task.frame = { ...frame, page: frame.page === 'home' ? 'search' : frame.page, query: command.text, capturedAt: timestamp(7) };
        return ok('');
    }
    if (command.type === 'browser.solve' || command.type === 'browser.grant') {
        if (command.type === 'browser.solve' && task.controller !== 'user')
            return fail('Take over before completing the sample site step.');
        if (task.challenge !== 'waiting' && task.challenge !== 'persistent')
            return fail('No pending challenge is available.');
        if (command.type === 'browser.grant') {
            if (task.attemptsRemaining < 1 || task.challenge === 'persistent')
                return fail('No automated attempts remain. Take over to continue.');
            task.granted.push('One-time permission for this challenge');
            task.attemptsRemaining--;
        }
        task.challenge = 'solved';
        task.blocker = '';
        task.lastConfirmedState = 'The explicit sample challenge interaction completed.';
        if (command.type === 'browser.grant') {
            task.state = 'running';
            task.controller = 'agent';
            state.turn = 'working';
        }
        updateThread(state, task);
        return ok('');
    }
    if (command.type === 'browser.resume') {
        if (task.challenge === 'waiting' || task.challenge === 'persistent')
            return fail('Complete the visible challenge before returning control.');
        task.controller = 'agent';
        task.state = 'running';
        task.continuedFrom = task.id;
        state.turn = 'working';
        updateThread(state, task);
        return ok('');
    }
    if (command.type === 'browser.start') {
        if (!['queued', 'assigned'].includes(task.state))
            return fail('Only queued or assigned tasks may be started.');
        if (state.browserTasks.some(other => other.id !== task.id && ['running', 'parked'].includes(other.state)))
            return fail('Another task holds the browser slot.');
        task.state = 'running';
        state.turn = 'working';
        updateThread(state, task);
        return ok('');
    }
    return null;
}
