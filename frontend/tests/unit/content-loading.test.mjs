import test from 'node:test';
import assert from 'node:assert/strict';
import { ContentLoading } from '../../dist/app/app/content-loading.js';
function setup(t) {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    let changes = 0;
    const loading = new ContentLoading(() => changes++);
    t.after(() => loading.dispose());
    return { loading, changes: () => changes };
}
test('ready synchronous collections never get automatic placeholders', t => {
    const { loading } = setup(t);
    for (const resource of ['ideas', 'feed', 'library'])
        assert.equal(loading.read(resource).phase, 'ready');
    t.mock.timers.tick(30000);
    assert.equal(loading.read('ideas').phase, 'ready');
});
test('fast operations settle without an artificial minimum loading time', t => {
    const { loading, changes } = setup(t);
    const token = loading.begin('feed', true);
    assert.equal(loading.read('feed').visible, false);
    loading.finish('feed', token, true);
    assert.equal(loading.read('feed').phase, 'ready');
    const before = changes();
    t.mock.timers.tick(30000);
    assert.equal(changes(), before);
});
test('slow operations reveal their notice then honest slow copy', t => {
    const { loading } = setup(t);
    loading.begin('feed', true);
    t.mock.timers.tick(120);
    assert.equal(loading.read('feed').visible, true);
    assert.equal(loading.read('feed').cached, true);
    assert.equal(loading.read('feed').slow, false);
    t.mock.timers.tick(10000);
    assert.equal(loading.read('feed').slow, true);
    assert.equal(loading.read('feed').phase, 'loading');
});
test('failed operations retain cached state and allow an explicit new attempt', t => {
    const { loading } = setup(t);
    const first = loading.begin('feed', true);
    loading.finish('feed', first, false, 'The edition did not finish.');
    assert.equal(loading.read('feed').cached, true);
    assert.equal(loading.read('feed').phase, 'error');
    const retry = loading.begin('feed', true);
    assert.notEqual(retry, null);
    loading.finish('feed', first, true);
    assert.equal(loading.read('feed').phase, 'loading');
    loading.finish('feed', retry, true);
    assert.equal(loading.read('feed').phase, 'ready');
});
test('duplicate real operations and demos cannot override a pending real command', t => {
    const { loading } = setup(t);
    loading.begin('feed', true);
    assert.equal(loading.begin('feed', true), null);
    assert.equal(loading.preview('feed', 'loading'), false);
    loading.endPreview('feed');
    assert.equal(loading.read('feed').phase, 'loading');
    assert.equal(loading.read('feed').demo, false);
});
test('preview tokens prevent late completions replacing a newer scenario', t => {
    const { loading } = setup(t);
    loading.preview('ideas', 'loading');
    t.mock.timers.tick(4000);
    loading.preview('ideas', 'error');
    t.mock.timers.tick(30000);
    assert.equal(loading.read('ideas').phase, 'error');
    assert.equal(loading.read('ideas').demo, true);
});
test('independent resources do not cancel each other on navigation', t => {
    const { loading } = setup(t);
    loading.preview('ideas', 'loading');
    loading.preview('library', 'error');
    t.mock.timers.tick(6000);
    assert.equal(loading.read('ideas').phase, 'ready');
    assert.equal(loading.read('library').phase, 'error');
});
test('preview error and refresh paths retain explicit source and cache state', t => {
    const { loading } = setup(t);
    loading.preview('feed', 'refresh-error');
    assert.deepEqual([loading.read('feed').cached, loading.read('feed').demo, loading.read('feed').phase], [true, true, 'error']);
    loading.preview('feed', 'refresh');
    assert.equal(loading.read('feed').cached, true);
    t.mock.timers.tick(6000);
    assert.equal(loading.read('feed').phase, 'ready');
});
test('ending a preview clears scheduled callbacks without affecting a real operation', t => {
    const { loading, changes } = setup(t);
    loading.preview('ideas', 'loading');
    const real = loading.begin('feed', true);
    loading.endPreview('ideas');
    const before = changes();
    t.mock.timers.tick(6000);
    assert.equal(loading.read('ideas').phase, 'ready');
    assert.equal(loading.read('feed').phase, 'loading');
    assert.equal(changes(), before + 1);
    loading.finish('feed', real, true);
});
test('dispose releases timers and ignores stale results and new requests', t => {
    const { loading, changes } = setup(t);
    const token = loading.begin('feed', false);
    loading.preview('ideas', 'loading');
    loading.dispose();
    const before = changes();
    t.mock.timers.tick(30000);
    loading.finish('feed', token, true);
    assert.equal(loading.begin('library', false), null);
    assert.equal(loading.preview('ideas', 'loading'), false);
    assert.equal(changes(), before);
});
