import test from 'node:test';
import assert from 'node:assert/strict';
import { ScriptedAdapter } from '../../dist/app/adapters/scripted-adapter.js';
import { createFixtures, row } from '../../dist/app/adapters/fixtures.js';
import { parseCommand } from '../../dist/app/protocol/codec.js';
import { splitAgentBubbles, isSecureEmbed, normalizeRow, deduplicateRows, finishVerdict, validThreadCopy, sanitizeBrowserNarration, maskCard, channelAction, containsSensitiveValue, canApprove } from '../../dist/app/protocol/rules.js';
const adapter = (options = {}) => new ScriptedAdapter({ animate: false, ...options });
const scenario = async (name) => { const app = adapter(); await app.execute({ type: 'demo.scenario', scenario: name }); return app; };
const quote = (app) => { const value = app.getSnapshot().purchases[0]; assert.ok(value); return value; };
const approve = (app) => { const purchase = quote(app); return app.execute({ type: 'purchase.approve', id: purchase.id, revision: purchase.revision, total: purchase.total }); };
const commandOK = async (app, command) => { const result = await app.execute(command); assert.equal(result.ok, true, result.message); return result; };
test('01 activity rows require complete evidence and force the underlying failure verdict', () => {
    const record = row('check', 'Checked the opening hours', 'Read the sample reference', 'globe');
    assert.equal(normalizeRow({ ...record, outputFailed: true, status: 'success' }).status, 'failed');
    for (const field of ['title', 'subtitle', 'icon', 'report', 'workId', 'laneId'])
        assert.throws(() => normalizeRow({ ...record, [field]: ' ' }));
});
test('01 deduplication replaces only the same work identity inside its lane', () => {
    const record = row('id', 'Checked one reference', 'Read the sample evidence');
    assert.equal(deduplicateRows([record, { ...record, title: 'Verified one reference' }]).length, 1);
    assert.equal(deduplicateRows([record, { ...record, laneId: 'child' }]).length, 2);
});
test('02 delegated child records stay in their lane with launch/start/terminal wording', async () => {
    const app = await scenario('research');
    await commandOK(app, { type: 'demo.advance', step: 1 });
    await commandOK(app, { type: 'demo.advance', step: 2 });
    const thread = app.getSnapshot().threads.find((entry) => entry.id === 'live-research');
    assert.ok(thread);
    assert.match(thread.lanes[0].rows.find((entry) => entry.phase === 'dispatch').title, /^Launched /);
    const child = thread.lanes.find((lane) => lane.parentId !== null);
    assert.ok(child);
    assert.match(child.rows[0].title, /^Started /);
    assert.match(child.rows.at(-1).title, /^Finished /);
    for (const lane of thread.lanes)
        for (const record of lane.rows)
            assert.equal(record.laneId, lane.id);
    for (const record of child.rows.filter((entry) => entry.phase !== 'action'))
        assert.equal(record.icon, 'run');
});
test('03 all fixture and live thread copy fits the length contract', async () => {
    const app = await scenario('research');
    for (const step of [0, 1, 2]) {
        if (step)
            await app.execute({ type: 'demo.advance', step });
        for (const thread of app.getSnapshot().threads)
            assert.ok(validThreadCopy(thread.name, thread.subtitle, thread.statusTitle), JSON.stringify(thread));
    }
});
test('03 unchanged progress does not rewrite the concrete subtitle', async () => {
    const app = await scenario('research');
    await app.execute({ type: 'demo.advance', step: 1 });
    const before = app.getSnapshot().threads[0];
    await app.execute({ type: 'demo.advance', step: 1 });
    assert.deepEqual(app.getSnapshot().threads[0], before);
});
test('04 browser narration strips failure, retry, and troubleshooting language', () => {
    for (const text of ['Retrying the checkout', 'Recovering from an error', 'Failed to check shipping', 'Troubleshooting the page'])
        assert.equal(sanitizeBrowserNarration(text, 'Checking the bag delivery options'), 'Checking the bag delivery options');
    assert.equal(sanitizeBrowserNarration('Checking the bag delivery options', 'Fallback'), 'Checking the bag delivery options');
});
test('05 finish success includes an honest handoff or any produced outcome', () => {
    const empty = { produced: false, launched: false, scheduled: false, saved: false, delivered: false, handoff: false };
    assert.equal(finishVerdict(empty), 'failed');
    for (const field of Object.keys(empty))
        assert.equal(finishVerdict({ ...empty, [field]: true }), 'success');
});
test('05 completed research succeeds even when one source row failed', async () => {
    const app = await scenario('research');
    await app.execute({ type: 'demo.advance', step: 1 });
    await app.execute({ type: 'demo.advance', step: 2 });
    const thread = app.getSnapshot().threads[0];
    assert.equal(thread.verdict, 'success');
    assert.ok(thread.lanes.flatMap((lane) => lane.rows).some((record) => record.status === 'failed'));
    assert.doesNotMatch(thread.finishMessage, /[#*`]|https?:|\.md|\/mnt\//);
});
test('06 explicit break tokens split bubbles; ordinary newlines do not', () => {
    assert.deepEqual(splitAgentBubbles('First line\nSecond line[[break]]Another bubble'), ['First line\nSecond line', 'Another bubble']);
    assert.equal(isSecureEmbed('A sentence.\n[[secure-entry]]\n'), true);
    assert.equal(isSecureEmbed('Inline [[secure-entry]] text'), false);
});
test('06 bounded choices accept one advertised option exactly once', async () => {
    const app = await scenario('choices');
    const message = app.getSnapshot().messages.find((entry) => entry.kind === 'choice');
    assert.equal((await app.execute({ type: 'message.choose', id: message.id, option: 'Unlisted' })).ok, false);
    await commandOK(app, { type: 'message.choose', id: message.id, option: message.options[0] });
    assert.equal((await app.execute({ type: 'message.choose', id: message.id, option: message.options[1] })).ok, false);
});
test('07 purchase requires approval and all material terms are present', async () => {
    const app = await scenario('purchase');
    const purchase = quote(app);
    for (const field of ['merchant', 'address', 'contact', 'shipping', 'estimate', 'methodId', 'addOns', 'cancellation'])
        assert.ok(purchase[field]);
    assert.equal(purchase.total, Math.round((purchase.subtotal + purchase.tax + purchase.fees + purchase.shippingCost) * 100) / 100);
    assert.equal((await app.execute({ type: 'purchase.submit', id: purchase.id })).ok, false);
    assert.equal(quote(app).ordersSubmitted, 0);
});
test('07 exact quote and card changes invalidate previous approval', async () => {
    const app = await scenario('purchase');
    const old = quote(app);
    await commandOK(app, { type: 'purchase.approve', id: old.id, revision: old.revision, total: old.total });
    await commandOK(app, { type: 'purchase.requote', id: old.id, total: old.total + 5 });
    assert.equal(quote(app).state, 'proposed');
    assert.equal((await app.execute({ type: 'purchase.approve', id: old.id, revision: old.revision, total: old.total })).ok, false);
    assert.equal((await app.execute({ type: 'purchase.submit', id: old.id })).ok, false);
    assert.equal((await approve(app)).ok, true);
    await commandOK(app, { type: 'purchase.method', id: old.id, methodId: 'mastercard' });
    assert.equal(quote(app).state, 'proposed');
});
test('07 plain yes approves the pending main-chat quote without magic words', async () => {
    const app = await scenario('purchase');
    await commandOK(app, { type: 'chat.send', chatId: 'main', text: 'yes' });
    assert.equal(quote(app).state, 'approved');
    assert.equal(quote(app).ordersSubmitted, 0);
});
test('07 each browser task can submit at most one order', async () => {
    const app = await scenario('purchase');
    await approve(app);
    await commandOK(app, { type: 'purchase.submit', id: quote(app).id });
    assert.equal((await app.execute({ type: 'purchase.submit', id: quote(app).id })).ok, false);
    await app.execute({ type: 'demo.advance', step: 3 });
    assert.equal(quote(app).state, 'confirmed');
    assert.equal(quote(app).ordersSubmitted, 1);
});
test('07 unknown payment never retries and browser handback is denied', async () => {
    const app = await scenario('unknown-payment');
    await approve(app);
    await app.execute({ type: 'purchase.submit', id: quote(app).id });
    await app.execute({ type: 'demo.advance', step: 3 });
    assert.equal(quote(app).state, 'unknown');
    const retry = await app.execute({ type: 'purchase.submit', id: quote(app).id });
    assert.equal(retry.ok, false);
    assert.match(retry.message, /twice|one allowed order|unknown/);
    await commandOK(app, { type: 'browser.takeover', id: quote(app).taskId });
    assert.equal((await app.execute({ type: 'browser.resume', id: quote(app).taskId })).ok, false);
});
test('07 insufficient fresh allowance shows the requested amount and remaining limit', async () => {
    const app = await scenario('purchase');
    const purchase = quote(app);
    const reason = canApprove(purchase, purchase.revision, purchase.total, 10);
    assert.match(reason, /79\.44/);
    assert.match(reason, /10\.00/);
});
test('08 card display is brand, four periods, and last four only', () => {
    assert.equal(maskCard({ id: 'one', brand: 'Visa', last4: '4242', provider: 'Sample', fee: 0 }), 'Visa .... 4242');
    assert.throws(() => maskCard({ brand: 'Visa', last4: '4111111111111111' }));
});
test('09 CAPTCHA pauses without permission; one grant consumes exactly one attempt', async () => {
    const app = await scenario('captcha');
    const task = app.getSnapshot().browserTasks[0];
    assert.equal(task.state, 'parked');
    assert.equal(task.challenge, 'waiting');
    assert.equal((await app.execute({ type: 'browser.resume', id: task.id })).ok, false);
    await commandOK(app, { type: 'browser.grant', id: task.id });
    assert.equal(app.getSnapshot().browserTasks[0].attemptsRemaining, task.attemptsRemaining - 1);
    assert.equal(app.getSnapshot().browserTasks[0].challenge, 'solved');
    assert.equal((await app.execute({ type: 'browser.grant', id: task.id })).ok, false);
});
test('09 persistent challenge hands off with no additional attempts', async () => {
    const app = await scenario('persistent-captcha');
    await commandOK(app, { type: 'browser.grant', id: 'challenge' });
    assert.equal(app.getSnapshot().browserTasks[0].attemptsRemaining, 0);
    assert.equal(app.getSnapshot().browserTasks[0].controller, 'user');
    assert.equal((await app.execute({ type: 'browser.resume', id: 'challenge' })).ok, false);
    await commandOK(app, { type: 'browser.solve', id: 'challenge' });
    await commandOK(app, { type: 'browser.resume', id: 'challenge' });
});
test('09 standing challenge preference authorizes only the challenge, not a purchase', async () => {
    const app = adapter();
    await app.execute({ type: 'settings.captcha', value: true });
    await app.execute({ type: 'demo.scenario', scenario: 'captcha' });
    const task = app.getSnapshot().browserTasks[0];
    assert.equal(task.challenge, 'solved');
    assert.equal(task.attemptsRemaining, 1);
    assert.deepEqual(task.granted, ['Standing challenge preference']);
    assert.equal(app.getSnapshot().purchases.length, 0);
});
test('10 each channel state exposes the correct action, including Messenger restrictions', async () => {
    const app = await scenario('channels');
    assert.deepEqual(app.getSnapshot().channels.map((channel) => channelAction(channel).action), ['open', 'check', 'retry-later', 'pending', 'begin']);
    await app.execute({ type: 'channel.check', id: 'messenger' });
    assert.equal(channelAction(app.getSnapshot().channels.find((channel) => channel.id === 'messenger')).action, 'view');
});
test('10 connect is blocked while checking and unavailable; secure handoff is required', async () => {
    const app = await scenario('channels');
    assert.equal((await app.execute({ type: 'channel.begin', id: 'messenger' })).ok, false);
    assert.equal((await app.execute({ type: 'channel.begin', id: 'telegram' })).ok, false);
    assert.equal((await app.execute({ type: 'channel.finish', id: 'sample-unlinked' })).ok, false);
    await commandOK(app, { type: 'channel.begin', id: 'sample-unlinked' });
    await commandOK(app, { type: 'channel.finish', id: 'sample-unlinked' });
    assert.equal(app.getSnapshot().messages.filter((message) => message.chatId === 'chat-unlinked').length, 1);
});
test('10 disconnect stops polling, adds goodbye, and reconnect does not repeat greeting', async () => {
    const app = adapter();
    await commandOK(app, { type: 'channel.disconnect', id: 'whatsapp' });
    assert.equal(app.getSnapshot().channels[0].polling, false);
    assert.match(app.getSnapshot().messages.at(-1).text, /Goodbye/);
    const count = app.getSnapshot().messages.length;
    await app.execute({ type: 'channel.begin', id: 'whatsapp' });
    await app.execute({ type: 'channel.finish', id: 'whatsapp' });
    assert.equal(app.getSnapshot().messages.length, count);
});
test('channel conversations are durable and read-only in the product composer', async () => {
    const app = adapter();
    assert.equal((await app.execute({ type: 'chat.send', chatId: 'chat-whatsapp', text: 'hi' })).ok, false);
    assert.equal((await app.execute({ type: 'chat.delete', id: 'chat-whatsapp' })).ok, false);
});
test('browser queue has one slot, site allowlist, and no restart after cancellation', async () => {
    const app = adapter();
    await commandOK(app, { type: 'browser.start', id: 'browser-idle' });
    assert.equal((await app.execute({ type: 'browser.start', id: 'browser-queued' })).ok, false);
    await app.execute({ type: 'browser.cancel', id: 'browser-idle' });
    assert.equal((await app.execute({ type: 'browser.start', id: 'browser-idle' })).ok, false);
    await app.execute({ type: 'settings.allowlist', value: 'Sample Supply' });
    assert.equal((await app.execute({ type: 'browser.start', id: 'browser-queued' })).ok, false);
});
test('small talk creates no activity thread; steering does not duplicate the current thread', async () => {
    const app = adapter();
    const count = app.getSnapshot().threads.length;
    await app.execute({ type: 'chat.send', chatId: 'main', text: 'hi' });
    assert.equal(app.getSnapshot().threads.length, count);
    await app.execute({ type: 'demo.scenario', scenario: 'research' });
    await app.execute({ type: 'chat.send', chatId: 'main', text: 'Focus on walking.' });
    assert.equal(app.getSnapshot().threads.length, count + 1);
});
test('interruption preserves saved state, while standalone resume returns to ready', async () => {
    const app = await scenario('research');
    await app.execute({ type: 'turn.interrupt' });
    await app.execute({ type: 'demo.advance', step: 2 });
    assert.equal(app.getSnapshot().threads[0].state, 'interrupted');
    await app.execute({ type: 'turn.resume' });
    await app.execute({ type: 'demo.advance', step: 2 });
    assert.equal(app.getSnapshot().threads[0].state, 'completed');
    const other = await scenario('interrupted');
    await other.execute({ type: 'turn.resume' });
    assert.equal(other.getSnapshot().turn, 'idle');
});
test('agent snapshots are immutable by replacement and subscribers are disposable', async () => {
    const app = adapter();
    const before = app.getSnapshot();
    let calls = 0;
    const unsubscribe = app.subscribe(() => calls++);
    await app.execute({ type: 'settings.name', value: 'Companion' });
    assert.equal(before.settings.name, 'hatchling');
    assert.equal(calls, 1);
    unsubscribe();
    await app.execute({ type: 'settings.name', value: 'hatchling' });
    assert.equal(calls, 1);
});
test('runtime command decoder rejects unsafe, missing, non-finite, and unknown inputs', () => {
    for (const value of [null, 'text', [], {}, { type: 'remote.exec' }, { type: 'chat.send', text: 'hello' }, { type: 'demo.advance', step: Infinity }, { type: 'settings.theme', value: 'ultraviolet' }])
        assert.equal(parseCommand(value), null);
    assert.deepEqual(parseCommand({ type: 'settings.theme', value: 'dark' }), { type: 'settings.theme', value: 'dark' });
});
test('scripted fixtures are deterministic, and reset clears state', async () => {
    assert.deepEqual(createFixtures(), createFixtures());
    const app = adapter();
    await app.execute({ type: 'chat.send', chatId: 'main', text: 'Hello' });
    await app.execute({ type: 'demo.reset' });
    assert.deepEqual(app.getSnapshot(), createFixtures());
});
test('storage event log replays edits without trusting arbitrary persisted state', async () => {
    const memory = new Map();
    const legacy = JSON.stringify([{ type: 'settings.name', value: 'Untouched legacy name' }]);
    memory.set('hatchling.scripted.events.v1', legacy);
    const storage = { getItem: (key) => memory.get(key) ?? null, setItem: (key, value) => memory.set(key, value), removeItem: (key) => memory.delete(key) };
    const app = adapter({ storage });
    assert.equal(app.getSnapshot().settings.name, 'hatchling');
    assert.equal(memory.get('hatchling.scripted.events.v1'), legacy);
    await app.execute({ type: 'settings.name', value: 'Aster' });
    await app.execute({ type: 'artifact.save', id: 'backlog', content: '# Saved locally' });
    const restored = adapter({ storage });
    assert.equal(restored.getSnapshot().settings.name, 'Aster');
    assert.equal(restored.getSnapshot().artifacts.find((item) => item.id === 'backlog').content, '# Saved locally');
    memory.set('hatchling.scripted.events.v10', '{not valid json');
    assert.equal(adapter({ storage }).getSnapshot().settings.name, 'hatchling');
});
test('denied storage never prevents in-memory operation', async () => {
    const storage = { getItem() { throw Error('denied'); }, setItem() { throw Error('denied'); }, removeItem() { throw Error('denied'); } };
    const app = adapter({ storage });
    await commandOK(app, { type: 'settings.name', value: 'Local only' });
    assert.equal(app.getSnapshot().settings.name, 'Local only');
    await commandOK(app, { type: 'demo.reset' });
});
test('obvious credentials and full card-like numbers are excluded from chat and memory', async () => {
    assert.equal(containsSensitiveValue('password: hunter2'), true);
    assert.equal(containsSensitiveValue('4111 1111 1111 1111'), true);
    const app = adapter();
    assert.equal((await app.execute({ type: 'chat.send', chatId: 'main', text: 'password: hunter2' })).ok, false);
    const entry = app.getSnapshot().memory.find((item) => item.kind === 'profile');
    assert.equal((await app.execute({ type: 'memory.save', id: entry.id, title: entry.title, body: 'password: hunter2' })).ok, false);
    assert.equal((await app.execute({ type: 'secure.add', service: 'password: hunter2' })).ok, false);
});
