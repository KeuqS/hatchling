import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { verifyBaseline } from '../../scripts/verify-baseline.mjs';
const digest = text => createHash('sha256').update(text).digest('hex');
const baseline = { files: { 'src/main.ts': digest('bootstrap'), 'src/protocol/models.ts': digest('contract'), 'src/features/chat/chat.ts': digest('view') } };
const source = { 'src/main.ts': 'bootstrap', 'src/protocol/models.ts': 'contract', 'src/features/chat/chat.ts': 'view' };
test('a new isolated live adapter is permitted without changing the frozen frontend', () => {
    assert.deepEqual(verifyBaseline(baseline, p => source[p], { actualFiles: [...Object.keys(source), 'src/adapters/live-adapter.ts', 'src/adapters/live/transport.ts'] }), []);
});
test('a view change fails the integration boundary', () => {
    assert.deepEqual(verifyBaseline(baseline, p => p.endsWith('chat.ts') ? 'changed' : source[p]), ['Changed frozen file: src/features/chat/chat.ts']);
});
test('a missing frozen source fails rather than silently passing', () => {
    assert.deepEqual(verifyBaseline(baseline, p => {
        if (p === 'src/protocol/models.ts')
            throw new Error('missing');
        return source[p];
    }), ['Missing frozen file: src/protocol/models.ts']);
});
test('new source files outside the allowed adapter area fail', () => {
    assert.deepEqual(verifyBaseline(baseline, p => source[p], { actualFiles: ['src/features/chat/new-view.ts'] }), ['New file outside the approved adapter area: src/features/chat/new-view.ts']);
});
test('bootstrap is frozen unless explicitly excluded for a reviewed factory switch', () => {
    const read = p => p === 'src/main.ts' ? 'live bootstrap' : source[p];
    assert.equal(verifyBaseline(baseline, read).length, 1);
    assert.deepEqual(verifyBaseline(baseline, read, { allowBootstrap: true }), []);
});
test('bootstrap allowance never allows a protocol modification', () => {
    assert.equal(verifyBaseline(baseline, p => p === 'src/protocol/models.ts' ? 'other contract' : source[p], { allowBootstrap: true }).length, 1);
});
