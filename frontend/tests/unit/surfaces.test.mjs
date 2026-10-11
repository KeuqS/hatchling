import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { createFixtures } from '../../dist/app/adapters/fixtures.js';
import { ScriptedAdapter } from '../../dist/app/adapters/scripted-adapter.js';
import { parseCommand } from '../../dist/app/protocol/codec.js';
import { selectSystemEntries, systemAncestors } from '../../dist/app/features/library/system.js';
test('reference goals are two separate tracking commitments without invented due dates', () => {
    const state = createFixtures();
    const goals = state.goals.filter((goal) => !goal.sampleOnly);
    assert.deepEqual(goals.map((goal) => goal.title), ['Sample workshop materials', 'Sample document collection']);
    assert.ok(goals.every((goal) => goal.kind === 'tracking' && goal.due === null));
    assert.match(goals[0].pulse, /No purchase is scheduled/);
    assert.equal(new Set(state.goals.filter((goal) => goal.sampleOnly).map((goal) => goal.pulseKind)).size, 7);
});
test('something else is decoded and persists as a real category, not an interests alias', async () => {
    const command = parseCommand({ type: 'goal.create', title: 'An unusual goal', description: 'A chosen result', category: 'Something else', due: '' });
    assert.ok(command);
    const app = new ScriptedAdapter({ animate: false });
    const result = await app.execute(command);
    assert.ok(result.ok);
    const goal = app.getSnapshot().goals.find((entry) => entry.id === result.id);
    assert.equal(goal.category, 'Something else');
    assert.equal(goal.sampleOnly, undefined);
});
test('new Ideas preserve reference ordering and one Productivity section', () => {
    const ideas = createFixtures().ideas.filter((idea) => !idea.sampleOnly);
    assert.equal(ideas.length, 7);
    assert.deepEqual(ideas.map((idea) => idea.section), ['', '', '', '', 'Productivity', 'Productivity', 'Productivity']);
    assert.ok(ideas.every((idea) => idea.signal.startsWith('Synthetic fact:') && idea.items.length === 1));
});
test('reference idea activation only produces sample evidence', async () => {
    const app = new ScriptedAdapter({ animate: false });
    await app.execute({ type: 'idea.activate', id: 'ref-idea-0' });
    await app.execute({ type: 'demo.advance', step: 4 });
    await app.execute({ type: 'demo.advance', step: 5 });
    const state = app.getSnapshot();
    const idea = state.ideas.find((entry) => entry.id === 'ref-idea-0');
    assert.equal(idea.state, 'completed');
    const result = state.artifacts.find((item) => item.id === idea.artifactId);
    assert.ok(result);
    assert.match(result.content, /sample|scripted|demo/i);
});
test('reference Feed retains dated sections and discloses that source claims are unverified', () => {
    const posts = createFixtures().posts.slice(0, 2);
    assert.deepEqual(posts.map((post) => post.date), ['Thursday morning', 'Wednesday evening']);
    assert.ok(posts.every((post) => post.reference && /example|fictional/i.test(post.reason)));
    assert.deepEqual(posts.map((post) => [post.media.width, post.media.height]), [[303, 170], [326, 183]]);
});
test('system root is a 16-folder metadata index with genuinely blank modified and size cells', () => {
    const entries = selectSystemEntries(createFixtures(), '', '', 'asc');
    assert.equal(entries.length, 16);
    assert.ok(entries.every((entry) => entry.kind === 'folder' && entry.modified === null && entry.bytes === null));
    assert.equal(entries[0].name, 'agents');
    assert.equal(entries.at(-1).name, 'workspace');
});
test('system selectors are case insensitive, reversible and do not mutate snapshots', () => {
    const state = createFixtures();
    const before = structuredClone(state);
    assert.deepEqual(selectSystemEntries(state, '', 'WORK', 'asc').map((entry) => entry.name), ['workspace']);
    assert.deepEqual(selectSystemEntries(state, '', '', 'desc'), [...selectSystemEntries(state, '', '', 'asc')].reverse());
    assert.deepEqual(state, before);
});
test('nested index paths have concrete ancestors and tolerate cycles defensively', () => {
    const state = createFixtures();
    assert.deepEqual(systemAncestors(state.systemEntries, 'workspace-documents').map((entry) => entry.name), ['workspace', 'documents']);
    assert.deepEqual(systemAncestors(state.systemEntries, 'missing'), []);
    const cyclic = structuredClone(state.systemEntries);
    cyclic.find((entry) => entry.id === 'dir-workspace').parentId = 'workspace-documents';
    assert.equal(systemAncestors(cyclic, 'workspace-documents').length, 2);
});
test('system documents respect recoverable artifact trash and restore', async () => {
    const app = new ScriptedAdapter({ animate: false });
    const visible = () => selectSystemEntries(app.getSnapshot(), 'dir-docs', '', 'asc').some((entry) => entry.artifactId === 'backlog');
    assert.ok(visible());
    await app.execute({ type: 'artifact.trash', id: 'backlog' });
    assert.equal(visible(), false);
    await app.execute({ type: 'artifact.restore', id: 'backlog' });
    assert.ok(visible());
});
test('deleted memory is removed from the file index selector', async () => {
    const app = new ScriptedAdapter({ animate: false });
    const entry = app.getSnapshot().memory.find((note) => note.kind === 'memory');
    assert.ok(entry);
    await app.execute({ type: 'memory.delete', id: entry.id });
    assert.equal(selectSystemEntries(app.getSnapshot(), 'dir-memory', '', 'asc').some((item) => item.memoryId === entry.id), false);
});
test('internal index contains only safe descriptions, no protected artifact or memory pointers', () => {
    const entries = createFixtures().systemEntries.filter((entry) => entry.visibility === 'internal');
    assert.ok(entries.length > 0);
    assert.ok(entries.every((entry) => entry.artifactId === null && entry.memoryId === null));
});
test('served assets contain no reference Lottie JSON or font binaries', () => {
    const walk = (dir) => readdirSync(dir, { withFileTypes: true }).flatMap((entry) => entry.isDirectory() ? walk(`${dir}/${entry.name}`) : [`${dir}/${entry.name}`]);
    assert.deepEqual(walk('public').filter((path) => /\.(woff2?|ttf|otf|json)$/i.test(path)), []);
    const built = readFileSync('dist/hatchling-standalone.html', 'utf8');
    assert.doesNotMatch(built, /HatchSystemIcon|hatchling-brand-loader-middle-out|ic-compose_agentic-medium|lottie-web/);
});
test('new documents immediately appear in both user-visible file trees', async () => {
    const app = new ScriptedAdapter({ animate: false });
    const result = await app.execute({ type: 'artifact.create', title: 'A new local note', format: 'markdown' });
    assert.ok(result.ok);
    for (const folder of ['dir-docs', 'workspace-documents'])
        assert.ok(selectSystemEntries(app.getSnapshot(), folder, '', 'asc').some((entry) => entry.artifactId === result.id));
});
test('memory title edits are reflected by the adapter-owned file index', async () => {
    const app = new ScriptedAdapter({ animate: false });
    const note = app.getSnapshot().memory.find((entry) => entry.kind === 'memory');
    await app.execute({ type: 'memory.save', id: note.id, title: 'A clearer note name', body: 'One useful preference.' });
    assert.ok(selectSystemEntries(app.getSnapshot(), 'dir-memory', 'clearer', 'asc').some((entry) => entry.memoryId === note.id));
});
