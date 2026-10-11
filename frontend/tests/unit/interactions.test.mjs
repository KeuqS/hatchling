import test from 'node:test';
import assert from 'node:assert/strict';
import { createFixtures } from '../../dist/app/adapters/fixtures.js';
import { ScriptedAdapter } from '../../dist/app/adapters/scripted-adapter.js';
import { startBundle, advanceBundle } from '../../dist/app/adapters/live-activity-fixtures.js';
import { parseCommand } from '../../dist/app/protocol/codec.js';
import { agentStatus } from '../../dist/app/features/context/agent-status.js';
const make = () => new ScriptedAdapter({ animate: false });
const storage = () => {
    const map = new Map();
    return { getItem: key => map.get(key) ?? null, setItem: (key, value) => map.set(key, value), removeItem: key => map.delete(key), text: () => [...map.values()].join('') };
};
const bundle = state => state.threads.find(thread => thread.id === 'live-bundle');
test('new settings fixture follows the connected/available reference without losing sample wallets', () => {
    const state = createFixtures();
    assert.deepEqual(state.connectors.filter(item => item.state === 'connected').map(item => item.name), ['Browser', 'Github', 'Instagram']);
    assert.equal(state.connectors.find(item => item.id === 'github').custom, true);
    assert.deepEqual(state.providers.filter(item => !item.sampleOnly).map(item => item.name), ['Link by Stripe', 'Shop Pay']);
    assert.ok(state.providers.some(item => item.sampleOnly && item.methods.length));
    assert.deepEqual(state.secureItems, []);
});
test('website and direct-protocol fixtures preserve reference states and independent object ownership', () => {
    const a = createFixtures(), b = createFixtures();
    assert.equal(a.websitePermissions.length, 6);
    assert.ok(a.websitePermissions.every(item => item.policy === 'allow'));
    assert.equal(a.networkPermissions.length, 8);
    assert.deepEqual(a.networkPermissions.map(item => item.enabled), [true, true, true, true, true, true, false, false]);
    a.networkPermissions[0].enabled = false;
    assert.equal(b.networkPermissions[0].enabled, true);
});
test('new commands reject malformed values at the persisted-event boundary', () => {
    for (const command of [{ type: 'permission.website', host: 'projects.example', policy: 'permit' }, { type: 'permission.network', id: 'tcp', enabled: 'true' }, { type: 'identity.avatar', value: {} }, { type: 'support.download', title: 'file', body: null }])
        assert.equal(parseCommand(command), null);
    assert.deepEqual(parseCommand({ type: 'demo.scenario', scenario: 'bundle-files' }), { type: 'demo.scenario', scenario: 'bundle-files' });
});
test('pending activity has no fabricated status and terminal rows retain explicit failures', () => {
    const state = createFixtures();
    startBundle(state);
    const thread = bundle(state);
    assert.equal(thread.state, 'running');
    assert.equal(thread.verdict, null);
    assert.equal(thread.pendingSteps.length, 2);
    assert.equal(thread.pendingSteps[1].laneId, thread.lanes[1].id);
    assert.equal(thread.pendingSteps[0].status, undefined);
    assert.equal(thread.pendingSteps[0].endedAt, undefined);
    assert.equal(thread.lanes[0].rows.find(item => item.id === 'bundle-sha-failed').status, 'failed');
    assert.equal(thread.pendingSteps[0].laneId, thread.lanes[0].id);
    assert.ok(!thread.lanes[0].rows.some(item => item.id === thread.pendingSteps[0].id));
});
test('pending-to-terminal conversion preserves identity and never duplicates the same step', () => {
    const state = createFixtures();
    startBundle(state);
    const id = bundle(state).pendingSteps[0].id;
    advanceBundle(state, 8);
    advanceBundle(state, 8);
    const thread = bundle(state);
    assert.equal(thread.lanes[0].rows.filter(item => item.id === id).length, 1);
    assert.equal(thread.pendingSteps[0].id, 'bundle-check');
    assert.ok(thread.lanes[0].rows.find(item => item.id === id).endedAt);
});
test('completed bundle keeps failed attempts, succeeds on outputs and delivers concrete files once', () => {
    const state = createFixtures();
    startBundle(state);
    advanceBundle(state, 8);
    advanceBundle(state, 9);
    const thread = bundle(state);
    assert.equal(thread.verdict, 'success');
    assert.deepEqual(thread.pendingSteps, []);
    assert.equal(thread.lanes[0].rows.find(item => item.id === 'bundle-sha-failed').status, 'failed');
    assert.equal(thread.evidenceArtifactIds.length, 2);
    for (const id of thread.evidenceArtifactIds)
        assert.ok(state.artifacts.some(item => item.id === id && item.content));
    const before = structuredClone(state);
    advanceBundle(state, 9);
    assert.deepEqual(state, before);
});
test('activity status reflects work and returns to Connected only when the tasks finish', () => {
    const state = createFixtures();
    startBundle(state);
    assert.equal(agentStatus(state).kind, 'working');
    assert.notEqual(agentStatus(state).label, 'Connected');
    advanceBundle(state, 8);
    advanceBundle(state, 9);
    assert.equal(agentStatus(state).label, 'Connected');
});
test('adapter delivers the completed bundle to its own chat and marks it unread', async () => {
    const adapter = make();
    await adapter.execute({ type: 'demo.scenario', scenario: 'bundle-files' });
    await adapter.execute({ type: 'chat.read', id: 'reference' });
    await adapter.execute({ type: 'demo.advance', step: 8 });
    await adapter.execute({ type: 'demo.advance', step: 9 });
    const state = adapter.getSnapshot();
    assert.equal(state.chats.find(item => item.id === 'reference').unread, true);
    assert.equal(state.messages.at(-1).chatId, 'reference');
    assert.equal(state.messages.at(-1).artifactId, 'bundle-archive');
    assert.ok(state.artifacts.some(item => item.id === 'bundle-manifest'));
    assert.ok(state.systemEntries.some(item => item.artifactId === 'bundle-script'));
    adapter.dispose();
});
test('interrupted and restored activity retains its pending evidence instead of claiming success', async () => {
    const port = storage(), first = new ScriptedAdapter({ animate: false, storage: port });
    await first.execute({ type: 'demo.scenario', scenario: 'bundle-files' });
    await first.execute({ type: 'turn.interrupt' });
    await first.execute({ type: 'demo.advance', step: 8 });
    assert.equal(bundle(first.getSnapshot()).state, 'interrupted');
    assert.equal(bundle(first.getSnapshot()).pendingSteps[0].id, 'bundle-write');
    const restored = new ScriptedAdapter({ animate: false, storage: port });
    assert.equal(bundle(restored.getSnapshot()).verdict, null);
    await restored.execute({ type: 'turn.resume' });
    await restored.execute({ type: 'demo.advance', step: 8 });
    await restored.execute({ type: 'demo.advance', step: 9 });
    assert.equal(bundle(restored.getSnapshot()).state, 'completed');
    first.dispose();
    restored.dispose();
});
test('website grants and protocol changes replay but do not disable the purchase approval gate', async () => {
    const port = storage(), adapter = new ScriptedAdapter({ animate: false, storage: port });
    await adapter.execute({ type: 'permission.website', host: 'projects.example', policy: 'block' });
    const networkId = adapter.getSnapshot().networkPermissions[0].id;
    await adapter.execute({ type: 'permission.network', id: networkId, enabled: false });
    const restored = new ScriptedAdapter({ animate: false, storage: port });
    assert.equal(restored.getSnapshot().websitePermissions.find(item => item.host === 'projects.example').policy, 'block');
    assert.equal(restored.getSnapshot().networkPermissions[0].enabled, false);
    assert.equal((await restored.execute({ type: 'permission.set', id: 'payments', mode: 'ask-for-some' })).ok, false);
    adapter.dispose();
    restored.dispose();
});
test('unknown website and protocol ids cannot grow an unreviewed grant list', async () => {
    const adapter = make();
    const before = structuredClone(adapter.getSnapshot());
    assert.equal((await adapter.execute({ type: 'permission.website', host: 'injected.example', policy: 'allow' })).ok, false);
    assert.equal((await adapter.execute({ type: 'permission.network', id: 'all-ports', enabled: true })).ok, false);
    assert.deepEqual(adapter.getSnapshot(), before);
    adapter.dispose();
});
test('reset removes all standing protocol/site grants while retaining sensitive-action rails', async () => {
    const adapter = make();
    await adapter.execute({ type: 'settings.captcha', value: true });
    await adapter.execute({ type: 'permission.reset' });
    const state = adapter.getSnapshot();
    assert.ok(state.websitePermissions.every(item => item.policy === 'ask'));
    assert.ok(state.networkPermissions.every(item => !item.enabled));
    assert.equal(state.settings.captchaStanding, false);
    assert.equal(state.permissions.find(item => item.id === 'payments').mode, 'always-ask');
    adapter.dispose();
});
test('Link goes through connected-empty to masked methods; coming-soon provider cannot connect', async () => {
    const adapter = make();
    assert.equal((await adapter.execute({ type: 'wallet.connect', id: 'shop-reference' })).ok, false);
    await adapter.execute({ type: 'wallet.connect', id: 'link' });
    assert.equal(adapter.getSnapshot().providers.find(item => item.id === 'link').methods.length, 0);
    await adapter.execute({ type: 'wallet.add-method', id: 'link' });
    const method = adapter.getSnapshot().providers.find(item => item.id === 'link').methods[0];
    assert.equal(method.last4.length, 4);
    assert.equal(method.number, undefined);
    adapter.dispose();
});
test('avatar command accepts only bounded raster data URLs and supports restoring the original', async () => {
    const adapter = make();
    for (const value of ['https://example.com/avatar.png', 'data:image/svg+xml;base64,PHN2Zz4=', 'data:image/png;base64,' + 'a'.repeat(700001)])
        assert.equal((await adapter.execute({ type: 'identity.avatar', value })).ok, false);
    assert.equal((await adapter.execute({ type: 'identity.avatar', value: 'data:image/png;base64,YWJj' })).ok, true);
    assert.equal((await adapter.execute({ type: 'identity.avatar', value: '' })).ok, true);
    assert.equal(adapter.getSnapshot().settings.avatar, '');
    adapter.dispose();
});
test('name and avatar updates leave the white default and other profile fields intact', async () => {
    const adapter = make(), vibe = adapter.getSnapshot().settings.vibe;
    await adapter.execute({ type: 'identity.update', name: 'Pip', vibe });
    await adapter.execute({ type: 'identity.avatar', value: 'data:image/png;base64,YWJj' });
    const settings = adapter.getSnapshot().settings;
    assert.equal(settings.name, 'Pip');
    assert.equal(settings.vibe, vibe);
    assert.equal(settings.accent, 'white');
    adapter.dispose();
});
test('local support download is an actual attachment result and is not persisted to the event log', async () => {
    const port = storage(), adapter = new ScriptedAdapter({ animate: false, storage: port });
    const body = '# A local-only issue note\n\nNothing was sent.';
    const result = await adapter.execute({ type: 'support.download', title: '../Test note', body });
    assert.equal(result.ok, true);
    assert.equal(result.download.mime, 'text/markdown');
    assert.equal(result.download.text, body);
    assert.ok(!result.download.filename.includes('/'));
    assert.ok(!port.text().includes('local-only issue note'));
    assert.equal((await adapter.execute({ type: 'support.download', title: 'note', body: 'x'.repeat(50001) })).ok, false);
    adapter.dispose();
});
test('secure reference commands preserve site metadata only in the deterministic fixture', async () => {
    const adapter = make();
    await adapter.execute({ type: 'secure.add', service: 'example.com' });
    assert.deepEqual(adapter.getSnapshot().secureItems, ['example.com · protected credential reference']);
    assert.equal(adapter.getSnapshot().secureItems.some(item => item.includes('sample-password')), false);
    adapter.dispose();
});
test('avatar accent sampling ignores white surrounds and transparent pixels', async () => {
    const { avatarAccentFromPixels } = await import('../../dist/app/features/context/avatar-color.js');
    const pixels = new Uint8ClampedArray([255, 255, 255, 255, 20, 160, 220, 255, 20, 160, 220, 255, 250, 0, 0, 0]);
    assert.equal(avatarAccentFromPixels(pixels), '#14a0dc');
    assert.equal(avatarAccentFromPixels(new Uint8ClampedArray()), '#a78c57');
    assert.equal(avatarAccentFromPixels(new Uint8ClampedArray([100, 100, 100, 255])), '#646464');
});
test('avatar-derived accent is validated, replayed, and applied only on opt-in', async () => {
    const port = storage(), adapter = new ScriptedAdapter({ animate: false, storage: port });
    assert.equal((await adapter.execute({ type: 'identity.avatar', value: 'data:image/png;base64,YWJj', accent: 'red;display:none' })).ok, false);
    await adapter.execute({ type: 'identity.avatar', value: 'data:image/png;base64,YWJj', accent: '#14a0dc' });
    assert.equal(adapter.getSnapshot().settings.accent, 'white');
    const restored = new ScriptedAdapter({ animate: false, storage: port });
    assert.equal(restored.getSnapshot().settings.avatarAccent, '#14a0dc');
    await restored.execute({ type: 'identity.avatar', value: '' });
    assert.equal(restored.getSnapshot().settings.avatarAccent, '#a78c57');
    restored.dispose();
    adapter.dispose();
});
