import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { lintSource } from '../../scripts/lint.mjs';
import { presentationSnapshot } from '../../dist/app/adapters/presentation-snapshot.js';
import { createFixtures, textMessage } from '../../dist/app/adapters/fixtures.js';
import { ScriptedAdapter } from '../../dist/app/adapters/scripted-adapter.js';
import * as display from '../../dist/app/presentation/format.js';
import * as rules from '../../dist/app/protocol/rules.js';
import { verifyBaseline } from '../../scripts/verify-baseline.mjs';
const view = 'src/features/example/view.ts';
test('type-only contract imports are allowed in presentation', () => {
    assert.deepEqual(lintSource(view, "import type { ProtocolAdapter } from '../../protocol/models.js';"), []);
});
test('runtime protocol imports are rejected even when disguised as renamed symbols', () => {
    assert.match(lintSource(view, "import { decode as display } from '../../protocol/codec.js';").join(), /only contract types/);
});
test('side-effect protocol imports cannot bypass the boundary', () => {
    assert.match(lintSource(view, "import '../../protocol/codec.js';").join(), /only contract types/);
});
test('re-export and dynamic-import boundary escapes are rejected', () => {
    assert.match(lintSource(view, "export * from '../../adapters/scripted-adapter.js';").join(), /cannot re-export/);
    assert.match(lintSource(view, "const load = () => import('../../protocol/codec.js');").join(), /Dynamic imports/);
});
test('concrete adapters and transport I/O stay out of views', () => {
    assert.match(lintSource(view, "import { ScriptedAdapter } from '../../adapters/scripted-adapter.js';").join(), /composition root/);
    for (const name of ['fetch', 'WebSocket', 'XMLHttpRequest', 'localStorage', 'sessionStorage'])
        assert.match(lintSource(view, `const value = ${name};`).join(), /I\/O belongs/);
});
test('protocol cannot depend on view code', () => {
    assert.match(lintSource('src/protocol/example.ts', "import { draw } from '../features/example.js';").join(), /Protocol must not depend/);
});
test('four frozen protocol modules still match the original contract', () => {
    const baseline = JSON.parse(readFileSync(new URL('../../docs/PROTOCOL-LOCK.json', import.meta.url)));
    assert.deepEqual(verifyBaseline(baseline, path => readFileSync(new URL(`../../${path}`, import.meta.url))), []);
});
test('scripted bubble convention is normalized below the seam with stable ids', () => {
    const state = createFixtures();
    state.messages = [textMessage('a', 'First.\n\nKeep this paragraph.[[break]]Second.'), textMessage('b', '[[break]]literal user text', 'user')];
    const snapshot = presentationSnapshot(state);
    assert.deepEqual(snapshot.messages.map(item => item.id), ['a', 'a:bubble:1', 'b']);
    assert.equal(snapshot.messages[0].text, 'First.\n\nKeep this paragraph.');
    assert.equal(snapshot.messages[1].text, 'Second.');
    assert.equal(snapshot.messages[2].text, '[[break]]literal user text');
    assert.deepEqual(presentationSnapshot(state), snapshot);
    assert.equal(state.messages.length, 2);
});
test('non-text message kinds cross the adapter without fake text rows', () => {
    const state = createFixtures();
    const original = state.messages.filter(item => item.kind !== 'text');
    const normalized = presentationSnapshot(state).messages.filter(item => item.kind !== 'text');
    assert.deepEqual(normalized, original);
});
test('all scripted agent texts returned by the adapter are presentation records', async () => {
    const adapter = new ScriptedAdapter({ animate: false });
    await adapter.execute({ type: 'chat.send', chatId: 'main', text: 'Plan a sample weekend' });
    assert.ok(adapter.getSnapshot().messages.filter(item => item.kind === 'text' && item.role === 'agent').every(item => !item.text.includes(rules.BUBBLE_BREAK)));
    adapter.dispose();
});
test('later bubble reaction is mapped to the canonical stored message', async () => {
    const adapter = new ScriptedAdapter({ animate: false });
    await adapter.execute({ type: 'demo.scenario', scenario: 'choices' });
    const bubble = adapter.getSnapshot().messages.find(item => item.id.includes(':bubble:'));
    assert.ok(bubble, 'The fixture includes a real normalized second bubble');
    const originalId = bubble.id.replace(/:bubble:\d+$/, '');
    const result = await adapter.execute({ type: 'message.react', id: bubble.id, reaction: '👍' });
    assert.equal(result.ok, true);
    assert.equal(adapter.getSnapshot().messages.find(item => item.id === originalId).reaction, '👍');
    adapter.dispose();
});
test('valid card display remains consistent without importing rules into presentation', () => {
    const state = createFixtures();
    for (const provider of state.providers)
        for (const method of provider.methods)
            assert.equal(display.maskCard(method), rules.maskCard(method));
    assert.equal(display.maskCard({ brand: 'Sample', last4: '1234567890' }), 'Sample ....');
});
test('money display and publish visibility remain presentation-only', () => {
    for (const amount of [0, 1, 68, 109.99, 200])
        assert.equal(display.currency(amount), rules.currency(amount));
    assert.equal(display.canPublish('app'), false);
    assert.equal(display.canPublish('document'), true);
    assert.equal(display.canPublish('unknown'), false);
});
test('channel labels preserve five-state behavior at the existing domain seam', () => {
    const channel = createFixtures().channels[0];
    for (const state of ['linked', 'unlinked', 'link_pending', 'checking', 'unavailable'])
        assert.deepEqual(display.channelAction({ ...channel, state }), rules.channelAction({ ...channel, state }));
});
test('synthetic editorial and permission fixtures do not imply connected accounts', () => {
    const state = createFixtures();
    assert.equal(state.mode, 'scripted');
    assert.ok(state.websitePermissions.every(item => item.host.endsWith('.example')));
    assert.ok(state.posts.slice(0, 2).every(item => item.kicker === 'Synthetic editorial'));
    assert.ok(state.posts.slice(0, 2).every(item => item.media?.src.endsWith('.svg')));
    assert.match(state.messages.find(item => item.id === 'ref-50').text, /synthetic workspace/);
});
