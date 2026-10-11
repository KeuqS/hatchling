import { startBrowserWatch } from '../browser-fixtures.js';
import { startBundle } from '../live-activity-fixtures.js';
import { startCi } from '../activity-fixtures.js';
import type { AppState, Message, Purchase, Scenario } from '../../protocol/models.js';
import { row, textMessage } from '../fixtures.js';
import { BUBBLE_BREAK } from '../../protocol/rules.js';
import { say } from './shared.js';
function base(id: string): {
    id: string;
    chatId: string;
    time: string;
    source: null;
    reaction: null;
    role: 'agent';
} {
    return { id, chatId: 'main', time: '9:41 am', source: null, reaction: null, role: 'agent' };
}
export function makePurchase(state: AppState, unknown = false): Purchase {
    return { id: 'sample-purchase', taskId: 'checkout', merchant: 'Sample Supply', items: [{ name: 'Everyday Weekender', variant: 'Olive · 28 L', quantity: 1, price: 68 }], address: 'Jamie Example\n12 Sample Lane\nPortland, OR 97205\nUnited States', contact: 'jamie@example.test · (555) 010-2026', shipping: 'Standard delivery', estimate: 'Sep 28–30 · sample estimate', subtotal: 68, tax: 5.44, shippingCost: 6, fees: 0, total: 79.44, methodId: state.providers.flatMap((provider) => provider.methods)[0]?.id ?? 'visa', addOns: 'None selected · no subscription or insurance', cancellation: 'Cancel before dispatch. Return within 30 days; return shipping is not included. Sample terms only.', state: 'proposed', reviewState: 'draft', revision: 1, approvedTotal: null, approvedRevision: null, ordersSubmitted: 0, outcome: unknown ? 'unknown' : 'confirmed' };
}
export function applyScenario(state: AppState, scenario: Scenario): void {
    state.messages = state.messages.filter((message) => message.chatId !== 'main');
    state.threads = state.threads.filter((thread) => !['live-research', 'live-ci', 'live-bundle', 'live-collect'].includes(thread.id));
    state.turn = 'idle';
    switch (scenario) {
        case 'browser-watch':
            state.browserTasks = state.browserTasks.filter(task => task.state !== 'running' && task.state !== 'parked');
            state.messages.push(textMessage('watch-user', 'Can you open your browser and search for reading nooks? I want to watch you do it.', 'user'));
            startBrowserWatch(state);
            break;
        case 'bundle-files':
            startBundle(state);
            break;
        case 'fix-ci':
            startCi(state);
            break;
        case 'purchase':
        case 'price-change':
        case 'unknown-payment': {
            const quote = makePurchase(state, scenario === 'unknown-payment');
            state.purchases = [quote];
            state.browserTasks = [{ id: 'checkout', title: 'Find the weekend bag', site: 'Sample Supply', step: 'Reviewing the full bag order details', statusTitle: 'Reviewing order', state: 'parked', controller: 'agent', attemptsRemaining: 2, constraints: ['At most one order', 'No higher total than the approved quote', 'Never retry an unknown payment'], granted: [], credentialReference: 'protected-reference:sample-supply', challenge: 'none', continuedFrom: null, blocker: 'Waiting for your approval of the exact quote.', lastConfirmedState: 'Review page. No order submitted.' }, ...state.browserTasks.filter((task) => task.id === 'browser-queued')];
            state.messages.push(textMessage('demo-user', 'Find the olive weekend bag, and show me everything before you buy it.', 'user'));
            say(state, 'Here’s the full review. Nothing is submitted until you approve this exact total.');
            state.messages.push({ ...base('demo-review'), kind: 'purchase', purchaseId: quote.id });
            state.turn = 'awaiting';
            if (scenario === 'price-change')
                say(state, 'This scenario lets you change the quote after approval. A changed price always needs a fresh review.');
            if (scenario === 'unknown-payment')
                say(state, 'This scenario ends with an unknown payment outcome after you approve and submit. It will never retry the payment.');
            break;
        }
        case 'research': {
            state.turn = 'working';
            state.messages.push(textMessage('demo-user', 'Compare a few ways to make the coast trip slower and simpler.', 'user'));
            state.messages.push({ ...base('demo-progress'), kind: 'progress', text: 'I’m comparing the route and places to stop, while a second researcher checks the weekend details.', evidenceId: 'live-research' });
            state.threads.unshift({ id: 'live-research', chatId: 'main', name: 'Shape a quieter coast trip', subtitle: 'Comparing routes with time for quiet stops', statusTitle: 'Comparing routes', state: 'running', verdict: null, finishMessage: '', time: '9:41 am', evidenceArtifactId: null, lanes: [
                    { id: 'main', name: 'Main agent', parentId: null, rows: [row('research-context', 'Checked the planning preferences', 'Kept Sunday free of commitments', 'memory'), row('research-dispatch', 'Launched Harbor details research', 'Delegated opening-hour checks to a researcher', 'run', 'main', 'dispatch')] },
                    { id: 'harbor', name: 'Harbor details research', parentId: 'main', rows: [row('research-start', 'Started Harbor details research', 'Checking the sample weekend opening details', 'run', 'harbor', 'start')] },
                ] });
            break;
        }
        case 'captcha':
        case 'persistent-captcha': {
            state.turn = 'awaiting';
            state.browserTasks = [{ id: 'challenge', title: 'Check bag delivery options', site: 'Sample Supply', step: 'Checking delivery options for the weekend bag', statusTitle: 'Checking delivery', state: 'parked', controller: 'agent', attemptsRemaining: scenario === 'captcha' ? 2 : 0, constraints: ['Challenge permission required', 'No automatic challenge loop', 'No checkout before review'], granted: [], credentialReference: null, challenge: scenario === 'captcha' ? 'waiting' : 'persistent', continuedFrom: null, blocker: scenario === 'captcha' ? 'The site asks for a challenge before showing delivery options.' : 'The challenge is still unreadable after the permitted attempt. No more attempts will be made.', lastConfirmedState: 'Product page with a delivery challenge. No payment submitted.' }];
            if (state.settings.captchaStanding && scenario === 'captcha') {
                const task = state.browserTasks[0];
                if (task) {
                    task.granted.push('Standing challenge preference');
                    task.attemptsRemaining--;
                    task.challenge = 'solved';
                    task.state = 'running';
                    task.blocker = '';
                    task.lastConfirmedState = 'Standing preference permitted one sample challenge; delivery options are visible.';
                    state.turn = 'idle';
                }
                say(state, 'Your standing challenge preference allowed this sample challenge. Delivery options are visible; no purchase was authorized.');
                break;
            }
            state.messages.push(textMessage('demo-user', 'Check when the bag could arrive.', 'user'));
            say(state, scenario === 'captcha' ? 'Sample Supply needs a challenge completed before it shows delivery options. You can allow this challenge once or take over and solve it yourself.' : 'I reached the delivery step on Sample Supply, but the permitted challenge attempt did not complete it. Please take over the browser, finish the challenge, and hand control back.');
            break;
        }
        case 'channels': {
            state.channels = [
                { id: 'whatsapp', name: 'WhatsApp', state: 'linked', polling: true, interval: '~1 second', greetingSent: true, chatId: 'chat-whatsapp', lastEvent: 'Listening for your messages' },
                { id: 'messenger', name: 'Messenger', state: 'checking', polling: false, interval: '~3 seconds', greetingSent: true, chatId: 'chat-messenger', lastEvent: 'Checking your connection. No connect action is available.' },
                { id: 'telegram', name: 'Telegram', state: 'unavailable', polling: false, interval: 'Not active', greetingSent: false, chatId: 'chat-telegram', lastEvent: 'Temporarily unavailable. Try again later.' },
                { id: 'sample-pending', name: 'Sample pending channel', state: 'link_pending', polling: false, interval: 'Not active', greetingSent: false, chatId: 'chat-pending', lastEvent: 'Waiting for the secure handoff already opened.' },
                { id: 'sample-unlinked', name: 'Sample unlinked channel', state: 'unlinked', polling: false, interval: 'Not active', greetingSent: false, chatId: 'chat-unlinked', lastEvent: 'Not connected.' },
            ];
            say(state, 'All five channel states are ready to inspect in Messaging channels. WhatsApp has its own conversation; Messenger never offers an open-chat deep link.');
            const purchase = makePurchase(state);
            state.purchases = [purchase];
            state.messages.push({ ...base('channel-review'), id: `channel-review-${state.revision}`, chatId: 'chat-messenger', source: 'Messenger', kind: 'purchase', purchaseId: purchase.id });
            break;
        }
        case 'secure': {
            say(state, 'Your credentials go straight to secure storage, where I cannot read them.');
            state.messages.push({ ...base('demo-secure'), kind: 'secure', service: 'Sample Supply', completed: false });
            state.messages.push({ ...base('demo-code'), kind: 'code-request', text: 'The site needs a one-time code. Enter it directly in the browser; do not paste it here.', completed: false });
            state.turn = 'awaiting';
            break;
        }
        case 'choices': {
            say(state, `I can make the plan lighter.${BUBBLE_BREAK}A new line stays inside this bubble.\nOnly the explicit break token starts another bubble.`);
            state.messages.push({ ...base('demo-choices'), kind: 'choice', text: 'Which part would you like to keep?', options: ['The coast walk', 'The bookshop stop', 'Both, with time between'], selected: null });
            say(state, 'What would make this feel like a good weekend for you?');
            break;
        }
        case 'mixed-stream': {
            state.messages.push(textMessage('mixed-user', 'Keep the weekend plan together and send me the recap later.', 'user'));
            const events: Message[] = [
                { ...base('mixed-scheduled'), kind: 'scheduled', text: 'Your arranged weekly recap is ready, with the changes you asked to receive.', evidenceId: 't-handoff' },
                { ...base('mixed-device'), kind: 'device', text: 'Your sample phone is reachable again. No synced content was posted to this chat.', evidenceId: null },
                { ...base('mixed-notice'), kind: 'notice', text: 'The background researcher handed back the opening-hour notes.', evidenceId: 't-weekend' },
                { ...base('mixed-file'), kind: 'attachment', text: 'Here is the complete plan to keep offline.', artifactId: 'weekend' },
                { ...base('mixed-image'), kind: 'attachment', text: 'Here is the reference image for the sample cover.', artifactId: 'portrait' },
                { ...base('mixed-error'), kind: 'error', text: 'One source was not available, so I left that detail out rather than guessing.', evidenceId: 't-weekend' },
            ];
            state.messages.push(...events);
            break;
        }
        case 'interrupted':
            state.turn = 'interrupted';
            say(state, 'The work is paused where you stopped it. You can steer the next step or resume from the saved context.');
            break;
        case 'stalled':
            state.turn = 'stalled';
            state.messages.push({ ...base('demo-stalled'), kind: 'error', text: 'I’m waiting for a source that is not responding. You can change the plan or stop here; nothing has been submitted.', evidenceId: null });
            break;
    }
}
export function advanceResearch(state: AppState, step: number): void {
    if (state.turn !== 'working')
        return;
    const thread = state.threads.find((item) => item.id === 'live-research');
    if (!thread || thread.state !== 'running')
        return;
    const main = thread.lanes.find((lane) => lane.id === 'main');
    const child = thread.lanes.find((lane) => lane.id === 'harbor');
    if (!main || !child)
        return;
    if (step === 1 && !child.rows.some((item) => item.id === 'research-source')) {
        thread.subtitle = 'Checking the bookshop and harbor opening times';
        thread.statusTitle = 'Checking stops';
        child.rows.push(row('research-source', 'Could not verify the bookshop hours', 'The sample source did not return a result', 'globe', 'harbor', 'action', true));
        child.rows.push(row('research-source2', 'Checked the harbor market details', 'Verified the alternate sample reference', 'globe', 'harbor'));
    }
    if (step === 2) {
        child.rows.push(row('research-end', 'Finished Harbor details research', 'Returned the verified stops and one uncertainty', 'run', 'harbor', 'finish'));
        main.rows.push(row('research-save', 'Saved a flexible coast itinerary', 'Marked unverified opening hours for your review', 'file'));
        thread.state = 'completed';
        thread.verdict = 'success';
        thread.statusTitle = 'Saved itinerary';
        thread.subtitle = 'Saved the itinerary with unverified details marked';
        thread.finishMessage = 'The plan is ready, with the unverified opening hours clearly marked. Nothing has been booked.';
        thread.evidenceArtifactId = 'weekend';
        state.turn = 'idle';
        say(state, 'The plan is ready. I left the unverified bookshop hours clearly marked rather than treating them as confirmed.');
        state.messages.push({ ...base(`research-artifact-${state.revision}`), kind: 'attachment', text: 'Here is the itinerary to keep or edit.', artifactId: 'weekend' });
    }
}
